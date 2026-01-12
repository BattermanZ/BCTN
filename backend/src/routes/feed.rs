use anyhow::{Context, Result, bail};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, debug_handler};
use axum_extra::extract::CookieJar;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::state::{AppState, CountryTotals};

#[derive(Debug, Deserialize)]
pub(crate) struct FeedQuery {
    /// Comma-separated list of evidence countries (subset of US,FR,CL).
    countries: Option<String>,

    /// rare|common|top
    pop: Option<String>,

    /// Include extra-rare (<75 ppm) names.
    include_extra_rare: Option<bool>,

    /// unisex|mostly_male|mostly_female|male|female|any
    gender: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct FeedCardResponse {
    card_key: String,
    card_kind: String,
    display_name: String,
    spellings_preview: Vec<String>,
    score: Option<i64>,
    per_country: Vec<CountrySummary>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CountrySummary {
    country: String,
    ppm: f64,
    top100_any: bool,
    gender: String,
}

#[derive(Debug, Clone, Copy)]
struct CountryStats {
    total: i64,
    male: i64,
    female: i64,
    top100_any: bool,
}

#[derive(Debug, Clone)]
struct CandidateCard {
    card_kind: CardKind,
    card_key: String,
    display_name: String,
    us: CountryStats,
    fr: CountryStats,
    cl: CountryStats,
}

#[derive(Debug, Clone, Copy)]
enum CardKind {
    Group,
    Concept,
}

impl CardKind {
    fn as_str(self) -> &'static str {
        match self {
            CardKind::Group => "group",
            CardKind::Concept => "concept",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum GenderFilter {
    Any,
    Unisex,
    MostlyMale,
    MostlyFemale,
    Male,
    Female,
}

#[derive(Debug, Clone, Copy)]
enum PopFilter {
    Rare,
    Common,
    Top,
}

fn parse_countries(s: Option<String>) -> Result<Vec<&'static str>> {
    let raw = s.unwrap_or_else(|| "US,FR,CL".to_string());
    let mut out = Vec::new();
    for part in raw.split(',').map(|x| x.trim()).filter(|x| !x.is_empty()) {
        match part {
            "US" => out.push("US"),
            "FR" => out.push("FR"),
            "CL" => out.push("CL"),
            other => bail!("invalid country {other:?}; expected US,FR,CL"),
        }
    }
    if out.is_empty() {
        bail!("countries must not be empty");
    }
    Ok(out)
}

fn parse_pop(s: Option<String>) -> Result<PopFilter> {
    match s.as_deref().unwrap_or("common") {
        "rare" => Ok(PopFilter::Rare),
        "common" => Ok(PopFilter::Common),
        "top" => Ok(PopFilter::Top),
        other => bail!("invalid pop {other:?}; expected rare|common|top"),
    }
}

fn parse_gender(s: Option<String>) -> Result<GenderFilter> {
    match s.as_deref().unwrap_or("any") {
        "any" => Ok(GenderFilter::Any),
        "unisex" => Ok(GenderFilter::Unisex),
        "mostly_male" => Ok(GenderFilter::MostlyMale),
        "mostly_female" => Ok(GenderFilter::MostlyFemale),
        "male" => Ok(GenderFilter::Male),
        "female" => Ok(GenderFilter::Female),
        other => bail!("invalid gender {other:?}"),
    }
}

fn ppm(total: i64, country_total_births: i64) -> f64 {
    if total <= 0 || country_total_births <= 0 {
        return 0.0;
    }
    (total as f64) * 1_000_000.0 / (country_total_births as f64)
}

fn tier_from_ppm(ppm: f64, top100_any: bool) -> Tier {
    if top100_any {
        return Tier::Top;
    }
    if ppm < 75.0 {
        Tier::ExtraRare
    } else if ppm < 300.0 {
        Tier::Rare
    } else {
        Tier::Common
    }
}

#[derive(Debug, Clone, Copy)]
enum Tier {
    ExtraRare,
    Rare,
    Common,
    Top,
}

fn tier_score(tier: Tier) -> i64 {
    match tier {
        Tier::ExtraRare => 0,
        Tier::Rare => 1,
        Tier::Common => 2,
        Tier::Top => 3,
    }
}

fn sweet_spot(ppm: f64, top100_any: bool) -> i64 {
    if top100_any {
        return 0;
    }
    if (300.0..800.0).contains(&ppm) { 1 } else { 0 }
}

fn gender_label(male: i64, female: i64) -> String {
    let total = male + female;
    if total <= 0 {
        return "unknown".to_string();
    }
    let mr = male as f64 / total as f64;
    let fr = female as f64 / total as f64;
    if mr <= 0.70 && fr <= 0.70 {
        "unisex".to_string()
    } else if mr >= 0.90 {
        "male".to_string()
    } else if fr >= 0.90 {
        "female".to_string()
    } else if mr >= 0.70 {
        "mostly_male".to_string()
    } else {
        "mostly_female".to_string()
    }
}

fn matches_gender(filter: GenderFilter, label: &str) -> bool {
    match filter {
        GenderFilter::Any => true,
        GenderFilter::Unisex => label == "unisex",
        GenderFilter::MostlyMale => label == "mostly_male",
        GenderFilter::MostlyFemale => label == "mostly_female",
        GenderFilter::Male => label == "male",
        GenderFilter::Female => label == "female",
    }
}

fn matches_pop(filter: PopFilter, ppm: f64, top100_any: bool) -> bool {
    match filter {
        PopFilter::Top => top100_any,
        PopFilter::Rare => !top100_any && (75.0..300.0).contains(&ppm),
        PopFilter::Common => !top100_any && ppm >= 300.0,
    }
}

fn score_best_picks(
    totals: &CountryTotals,
    us: CountryStats,
    fr: CountryStats,
    cl: CountryStats,
) -> i64 {
    let ppm_us = ppm(us.total, totals.us_total);
    let ppm_fr = ppm(fr.total, totals.fr_total);
    let ppm_cl = ppm(cl.total, totals.cl_total);

    let tier_us = tier_from_ppm(ppm_us, us.top100_any);
    let tier_fr = tier_from_ppm(ppm_fr, fr.top100_any);
    let tier_cl = tier_from_ppm(ppm_cl, cl.top100_any);

    let base = tier_score(tier_fr) + tier_score(tier_us) + tier_score(tier_cl);

    let present_count = [ppm_fr, ppm_us, ppm_cl]
        .into_iter()
        .filter(|&v| v >= 75.0)
        .count() as i64;
    let bonus_present = match present_count {
        3 => 6,
        2 => 3,
        _ => 0,
    };

    let top_count = [us.top100_any, fr.top100_any, cl.top100_any]
        .into_iter()
        .filter(|&v| v)
        .count() as i64;
    let penalty_top = 5 * top_count;

    let sweet = sweet_spot(ppm_fr, fr.top100_any)
        + sweet_spot(ppm_us, us.top100_any)
        + sweet_spot(ppm_cl, cl.top100_any);

    base + bonus_present + sweet - penalty_top
}

#[debug_handler]
pub(crate) async fn next(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<FeedQuery>,
) -> impl IntoResponse {
    match next_impl(&state, &jar, q).await {
        Ok(card) => Json(card).into_response(),
        Err(err) => {
            tracing::warn!(error=%err, "feed_next_failed");
            (StatusCode::BAD_REQUEST, err.to_string()).into_response()
        }
    }
}

async fn next_impl(state: &AppState, jar: &CookieJar, q: FeedQuery) -> Result<FeedCardResponse> {
    let (user_id, _) = super::auth::require_session_user(state, jar).await?;
    let selected_countries = parse_countries(q.countries)?;
    let pop = parse_pop(q.pop)?;
    let gender = parse_gender(q.gender)?;
    let include_extra_rare = q.include_extra_rare.unwrap_or(false);

    let best_pick_mode: i64 =
        sqlx::query_scalar("SELECT best_pick_mode FROM user_settings WHERE user_id = ?1")
            .bind(user_id)
            .fetch_optional(&state.db)
            .await
            .context("load best_pick_mode")?
            .unwrap_or(0);

    let mut candidates = load_candidates(state, user_id, 3000).await?;

    candidates.retain(|c| {
        let totals = state.country_total_births;

        let by_country = |country: &str| -> (f64, bool, &CountryStats) {
            match country {
                "US" => (ppm(c.us.total, totals.us_total), c.us.top100_any, &c.us),
                "FR" => (ppm(c.fr.total, totals.fr_total), c.fr.top100_any, &c.fr),
                "CL" => (ppm(c.cl.total, totals.cl_total), c.cl.top100_any, &c.cl),
                _ => (0.0, false, &c.us),
            }
        };

        let mut has_any = false;
        let mut passes_any_pop = false;
        let mut passes_any_gender = false;
        let mut passes_any_non_extra = false;

        for &country in &selected_countries {
            let (p, top, stats) = by_country(country);
            if stats.total > 0 {
                has_any = true;
            }
            if matches_pop(pop, p, top) {
                passes_any_pop = true;
            }
            if p >= 75.0 {
                passes_any_non_extra = true;
            }
            let gl = gender_label(stats.male, stats.female);
            if matches_gender(gender, &gl) {
                passes_any_gender = true;
            }
        }

        if !has_any {
            return false;
        }
        if !include_extra_rare && !passes_any_non_extra {
            return false;
        }
        if !passes_any_pop {
            return false;
        }
        if !passes_any_gender {
            return false;
        }
        true
    });

    if candidates.is_empty() {
        bail!("no matching names left (try broader filters)");
    }

    let chosen = if best_pick_mode != 0 {
        candidates
            .into_iter()
            .max_by_key(|c| score_best_picks(&state.country_total_births, c.us, c.fr, c.cl))
            .unwrap()
    } else {
        let mut rng = rand::rngs::OsRng;
        candidates.choose(&mut rng).cloned().expect("non-empty")
    };

    let preview = spellings_preview(
        state,
        chosen.card_kind,
        &chosen.card_key,
        &selected_countries,
    )
    .await?;

    let totals = state.country_total_births;
    let mut per_country = Vec::new();
    for country in ["US", "FR", "CL"] {
        let (stats, country_total) = match country {
            "US" => (chosen.us, totals.us_total),
            "FR" => (chosen.fr, totals.fr_total),
            "CL" => (chosen.cl, totals.cl_total),
            _ => continue,
        };
        let p = ppm(stats.total, country_total);
        per_country.push(CountrySummary {
            country: country.to_string(),
            ppm: p,
            top100_any: stats.top100_any,
            gender: gender_label(stats.male, stats.female),
        });
    }

    let score = if best_pick_mode != 0 {
        Some(score_best_picks(
            &state.country_total_births,
            chosen.us,
            chosen.fr,
            chosen.cl,
        ))
    } else {
        None
    };

    Ok(FeedCardResponse {
        card_key: chosen.card_key,
        card_kind: chosen.card_kind.as_str().to_string(),
        display_name: chosen.display_name,
        spellings_preview: preview,
        score,
        per_country,
    })
}

async fn load_candidates(state: &AppState, user_id: i64, limit: i64) -> Result<Vec<CandidateCard>> {
    let rows = sqlx::query(
        r#"
        SELECT
          'group' AS card_kind,
          vg.variant_group_key AS card_key,
          ns.display AS display_name,
          COALESCE(us.total, 0) AS us_total,
          COALESCE(us.male, 0) AS us_male,
          COALESCE(us.female, 0) AS us_female,
          COALESCE(ust.top100_any, 0) AS us_top,
          COALESCE(fr.total, 0) AS fr_total,
          COALESCE(fr.male, 0) AS fr_male,
          COALESCE(fr.female, 0) AS fr_female,
          COALESCE(frt.top100_any, 0) AS fr_top,
          COALESCE(cl.total, 0) AS cl_total,
          COALESCE(cl.male, 0) AS cl_male,
          COALESCE(cl.female, 0) AS cl_female,
          COALESCE(clt.top100_any, 0) AS cl_top
        FROM variant_group vg
        JOIN name_concept nc ON nc.concept_key = vg.variant_group_key
        JOIN name_string ns ON ns.id = nc.preferred_name_string_id
        LEFT JOIN variant_group_country_window_total us ON us.variant_group_key = vg.variant_group_key AND us.country='US'
        LEFT JOIN variant_group_country_top100_any ust ON ust.variant_group_key = vg.variant_group_key AND ust.country='US'
        LEFT JOIN variant_group_country_window_total fr ON fr.variant_group_key = vg.variant_group_key AND fr.country='FR'
        LEFT JOIN variant_group_country_top100_any frt ON frt.variant_group_key = vg.variant_group_key AND frt.country='FR'
        LEFT JOIN variant_group_country_window_total cl ON cl.variant_group_key = vg.variant_group_key AND cl.country='CL'
        LEFT JOIN variant_group_country_top100_any clt ON clt.variant_group_key = vg.variant_group_key AND clt.country='CL'
        WHERE NOT EXISTS (SELECT 1 FROM swipe s WHERE s.user_id = ?1 AND s.card_key = vg.variant_group_key)

        UNION ALL

        SELECT
          'concept' AS card_kind,
          nc.concept_key AS card_key,
          ns.display AS display_name,
          COALESCE(us.total, 0) AS us_total,
          COALESCE(us.male, 0) AS us_male,
          COALESCE(us.female, 0) AS us_female,
          COALESCE(ust.top100_any, 0) AS us_top,
          COALESCE(fr.total, 0) AS fr_total,
          COALESCE(fr.male, 0) AS fr_male,
          COALESCE(fr.female, 0) AS fr_female,
          COALESCE(frt.top100_any, 0) AS fr_top,
          COALESCE(cl.total, 0) AS cl_total,
          COALESCE(cl.male, 0) AS cl_male,
          COALESCE(cl.female, 0) AS cl_female,
          COALESCE(clt.top100_any, 0) AS cl_top
        FROM name_concept nc
        JOIN name_string ns ON ns.id = nc.preferred_name_string_id
        LEFT JOIN variant_group_member vgm ON vgm.concept_key = nc.concept_key
        LEFT JOIN concept_country_window_total us ON us.concept_id = nc.id AND us.country='US'
        LEFT JOIN concept_country_top100_any ust ON ust.concept_id = nc.id AND ust.country='US'
        LEFT JOIN concept_country_window_total fr ON fr.concept_id = nc.id AND fr.country='FR'
        LEFT JOIN concept_country_top100_any frt ON frt.concept_id = nc.id AND frt.country='FR'
        LEFT JOIN concept_country_window_total cl ON cl.concept_id = nc.id AND cl.country='CL'
        LEFT JOIN concept_country_top100_any clt ON clt.concept_id = nc.id AND clt.country='CL'
        WHERE vgm.concept_key IS NULL
          AND NOT EXISTS (SELECT 1 FROM swipe s WHERE s.user_id = ?1 AND s.card_key = nc.concept_key)

        LIMIT ?2
        "#,
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(&state.names_db)
    .await
    .context("load candidates (run `bctn-ingest variants materialize`)")?;

    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let card_kind_s: String = r.try_get("card_kind")?;
        let card_kind = match card_kind_s.as_str() {
            "group" => CardKind::Group,
            "concept" => CardKind::Concept,
            _ => bail!("unexpected card_kind in sql"),
        };
        out.push(CandidateCard {
            card_kind,
            card_key: r.try_get("card_key")?,
            display_name: r.try_get("display_name")?,
            us: CountryStats {
                total: r.try_get("us_total")?,
                male: r.try_get("us_male")?,
                female: r.try_get("us_female")?,
                top100_any: r.try_get::<i64, _>("us_top")? != 0,
            },
            fr: CountryStats {
                total: r.try_get("fr_total")?,
                male: r.try_get("fr_male")?,
                female: r.try_get("fr_female")?,
                top100_any: r.try_get::<i64, _>("fr_top")? != 0,
            },
            cl: CountryStats {
                total: r.try_get("cl_total")?,
                male: r.try_get("cl_male")?,
                female: r.try_get("cl_female")?,
                top100_any: r.try_get::<i64, _>("cl_top")? != 0,
            },
        });
    }
    Ok(out)
}

async fn spellings_preview(
    state: &AppState,
    kind: CardKind,
    card_key: &str,
    selected_countries: &[&'static str],
) -> Result<Vec<String>> {
    async fn top1(
        state: &AppState,
        kind: CardKind,
        card_key: &str,
        run_id: i64,
        country: &str,
    ) -> Result<Option<String>> {
        let sql = match kind {
            CardKind::Concept => {
                r#"
                SELECT no.name_raw AS name_raw
                FROM name_concept nc
                JOIN name_concept_member ncm ON ncm.concept_id = nc.id
                JOIN name_observation no ON no.name_string_id = ncm.name_string_id
                WHERE nc.concept_key = ?1 AND no.run_id = ?2 AND no.country = ?3
                GROUP BY no.name_raw
                ORDER BY SUM(no.count) DESC
                LIMIT 1
                "#
            }
            CardKind::Group => {
                r#"
                WITH concept_ids AS (
                  SELECT nc.id AS id
                  FROM variant_group_member vgm
                  JOIN name_concept nc ON nc.concept_key = vgm.concept_key
                  WHERE vgm.variant_group_key = ?1
                )
                SELECT no.name_raw AS name_raw
                FROM name_concept_member ncm
                JOIN name_observation no ON no.name_string_id = ncm.name_string_id
                WHERE ncm.concept_id IN (SELECT id FROM concept_ids)
                  AND no.run_id = ?2
                  AND no.country = ?3
                GROUP BY no.name_raw
                ORDER BY SUM(no.count) DESC
                LIMIT 1
                "#
            }
        };

        let v = sqlx::query_scalar::<_, String>(sql)
            .bind(card_key)
            .bind(run_id)
            .bind(country)
            .fetch_optional(&state.names_db)
            .await
            .with_context(|| format!("top spelling for {card_key} {country}"))?;
        Ok(v)
    }

    async fn global_top_n(
        state: &AppState,
        kind: CardKind,
        card_key: &str,
        run_id: i64,
        countries: &[&str],
        n: i64,
    ) -> Result<Vec<String>> {
        let placeholders = countries.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = match kind {
            CardKind::Concept => format!(
                r#"
                SELECT no.name_raw AS name_raw
                FROM name_concept nc
                JOIN name_concept_member ncm ON ncm.concept_id = nc.id
                JOIN name_observation no ON no.name_string_id = ncm.name_string_id
                WHERE nc.concept_key = ?1
                  AND no.run_id = ?2
                  AND no.country IN ({placeholders})
                GROUP BY no.name_raw
                ORDER BY SUM(no.count) DESC
                LIMIT ?3
                "#
            ),
            CardKind::Group => format!(
                r#"
                WITH concept_ids AS (
                  SELECT nc.id AS id
                  FROM variant_group_member vgm
                  JOIN name_concept nc ON nc.concept_key = vgm.concept_key
                  WHERE vgm.variant_group_key = ?1
                )
                SELECT no.name_raw AS name_raw
                FROM name_concept_member ncm
                JOIN name_observation no ON no.name_string_id = ncm.name_string_id
                WHERE ncm.concept_id IN (SELECT id FROM concept_ids)
                  AND no.run_id = ?2
                  AND no.country IN ({placeholders})
                GROUP BY no.name_raw
                ORDER BY SUM(no.count) DESC
                LIMIT ?3
                "#
            ),
        };

        let mut q = sqlx::query_scalar::<_, String>(&sql)
            .bind(card_key)
            .bind(run_id);
        for c in countries {
            q = q.bind(*c);
        }
        let out = q.bind(n).fetch_all(&state.names_db).await?;
        Ok(out)
    }

    let run_id = state.names_window.run_id;

    let mut country_tops = Vec::new();
    for &country in selected_countries {
        if let Some(v) = top1(state, kind, card_key, run_id, country).await?
            && !country_tops.contains(&v)
        {
            country_tops.push(v);
        }
    }

    if selected_countries.len() > 1 && country_tops.len() == 1 {
        return global_top_n(state, kind, card_key, run_id, selected_countries, 3).await;
    }

    if country_tops.len() >= 3 {
        country_tops.truncate(3);
        return Ok(country_tops);
    }

    let mut out = country_tops;
    let global = global_top_n(state, kind, card_key, run_id, selected_countries, 10).await?;
    for g in global {
        if out.len() >= 3 {
            break;
        }
        if !out.contains(&g) {
            out.push(g);
        }
    }
    Ok(out)
}
