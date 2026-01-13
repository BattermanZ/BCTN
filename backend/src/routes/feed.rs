use anyhow::{Context, Result, bail};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, debug_handler};
use axum_extra::extract::CookieJar;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::collections::HashSet;

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

#[derive(Debug, Deserialize)]
pub(crate) struct FeedBatchQuery {
    /// Comma-separated list of evidence countries (subset of US,FR,CL).
    countries: Option<String>,

    /// rare|common|top
    pop: Option<String>,

    /// Include extra-rare (<75 ppm) names.
    include_extra_rare: Option<bool>,

    /// unisex|mostly_male|mostly_female|male|female|any
    gender: Option<String>,

    /// Number of cards to return.
    limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub(crate) struct FeedCardResponse {
    card_key: String,
    card_kind: String,
    display_name: String,
    spellings_preview: Vec<String>,
    score: Option<i64>,
    per_country: Vec<CountrySummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last5_year_ranks: Option<BTreeMap<String, Vec<RankPoint>>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CountrySummary {
    country: String,
    ppm: f64,
    top100_any: bool,
    gender: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct RankPoint {
    year: i64,
    rank: i64,
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

#[debug_handler]
pub(crate) async fn batch(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<FeedBatchQuery>,
) -> impl IntoResponse {
    match batch_impl(&state, &jar, q).await {
        Ok(cards) => Json(cards).into_response(),
        Err(err) => {
            tracing::warn!(error=%err, "feed_batch_failed");
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

    let base_candidate_limit = if best_pick_mode != 0 { 20000 } else { 3000 };

    let mut candidates = Vec::<CandidateCard>::new();
    let mut candidate_limit = base_candidate_limit;
    for _attempt in 0..4 {
        candidates = load_candidates(state, user_id, candidate_limit).await?;
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

        if !candidates.is_empty() {
            break;
        }
        // We may have filtered out most of the top `candidate_limit` rows (due to swipes and/or
        // narrow filters). Increase the scan window so we can still find remaining candidates.
        candidate_limit = (candidate_limit * 3).min(200_000);
    }

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

    build_response(state, &chosen, &selected_countries, best_pick_mode != 0).await
}

async fn batch_impl(
    state: &AppState,
    jar: &CookieJar,
    q: FeedBatchQuery,
) -> Result<Vec<FeedCardResponse>> {
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

    let limit = q.limit.unwrap_or(10).clamp(1, 25);
    let base_candidate_limit = if best_pick_mode != 0 { 20000 } else { 3000 };

    let mut candidates = Vec::<CandidateCard>::new();
    let mut candidate_limit = base_candidate_limit;
    for _attempt in 0..4 {
        candidates = load_candidates(state, user_id, candidate_limit).await?;
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

        if !candidates.is_empty() {
            break;
        }
        candidate_limit = (candidate_limit * 3).min(200_000);
    }

    if candidates.is_empty() {
        bail!("no matching names left (try broader filters)");
    }

    let chosen: Vec<CandidateCard> = if best_pick_mode != 0 {
        let mut v = candidates;
        v.sort_by_key(|c| {
            Reverse(score_best_picks(
                &state.country_total_births,
                c.us,
                c.fr,
                c.cl,
            ))
        });
        v.into_iter().take(limit as usize).collect()
    } else {
        let mut rng = rand::rngs::OsRng;
        let mut v = candidates;
        v.shuffle(&mut rng);
        v.into_iter().take(limit as usize).collect()
    };

    let mut out = Vec::with_capacity(chosen.len());
    for c in chosen {
        out.push(build_response(state, &c, &selected_countries, best_pick_mode != 0).await?);
    }
    Ok(out)
}

async fn build_response(
    state: &AppState,
    chosen: &CandidateCard,
    selected_countries: &[&'static str],
    best_pick_mode: bool,
) -> Result<FeedCardResponse> {
    let preview = spellings_preview_materialized(
        state,
        chosen.card_kind,
        &chosen.card_key,
        selected_countries,
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

    let score = if best_pick_mode {
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
        card_key: chosen.card_key.clone(),
        card_kind: chosen.card_kind.as_str().to_string(),
        display_name: chosen.display_name.clone(),
        spellings_preview: preview,
        score,
        per_country,
        last5_year_ranks: Some(
            last5_year_ranks_materialized(state, chosen.card_kind, &chosen.card_key).await?,
        ),
    })
}

async fn load_candidates(state: &AppState, user_id: i64, limit: i64) -> Result<Vec<CandidateCard>> {
    // `swipe` lives in the app DB (state.db), not the names DB (state.names_db). We need to
    // filter out swiped cards after reading candidates from the names DB.
    //
    // Important: because we can't filter in SQL on the names DB connection, we must query with
    // enough headroom. Otherwise, if the user swipes a lot of popular names, we could end up
    // with "no matching names left" even though there are plenty lower in the global ordering.
    let swiped: Vec<String> = sqlx::query_scalar("SELECT card_key FROM swipe WHERE user_id = ?1")
        .bind(user_id)
        .fetch_all(&state.db)
        .await
        .context("load swiped card keys")?;
    let swiped_set: HashSet<String> = swiped.into_iter().collect();
    let query_limit = limit
        .saturating_add(swiped_set.len().try_into().unwrap_or(0))
        .saturating_add(200);

    let rows = sqlx::query(
        r#"
        WITH cards AS (
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
            COALESCE(clt.top100_any, 0) AS cl_top,
            COALESCE(us.total, 0) + COALESCE(fr.total, 0) + COALESCE(cl.total, 0) AS global_total
          FROM variant_group vg
          JOIN name_concept nc ON nc.concept_key = vg.variant_group_key
          JOIN name_string ns ON ns.id = nc.preferred_name_string_id
          LEFT JOIN variant_group_country_window_total us ON us.variant_group_key = vg.variant_group_key AND us.country='US'
          LEFT JOIN variant_group_country_top100_any ust ON ust.variant_group_key = vg.variant_group_key AND ust.country='US'
          LEFT JOIN variant_group_country_window_total fr ON fr.variant_group_key = vg.variant_group_key AND fr.country='FR'
          LEFT JOIN variant_group_country_top100_any frt ON frt.variant_group_key = vg.variant_group_key AND frt.country='FR'
          LEFT JOIN variant_group_country_window_total cl ON cl.variant_group_key = vg.variant_group_key AND cl.country='CL'
          LEFT JOIN variant_group_country_top100_any clt ON clt.variant_group_key = vg.variant_group_key AND clt.country='CL'

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
            COALESCE(clt.top100_any, 0) AS cl_top,
            COALESCE(us.total, 0) + COALESCE(fr.total, 0) + COALESCE(cl.total, 0) AS global_total
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
        )
        SELECT * FROM cards
        ORDER BY global_total DESC, card_key ASC
        LIMIT ?1
        "#,
    )
    .bind(query_limit)
    .fetch_all(&state.names_db)
    .await
    .context("load candidates from names db (run `bctn-ingest variants materialize` if missing)")?;

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

    if !swiped_set.is_empty() {
        out.retain(|c| !swiped_set.contains(&c.card_key));
    }

    Ok(out)
}

fn subset_key_for(countries: &[&str]) -> String {
    let mut has_us = false;
    let mut has_fr = false;
    let mut has_cl = false;
    for &c in countries {
        match c {
            "US" => has_us = true,
            "FR" => has_fr = true,
            "CL" => has_cl = true,
            _ => {}
        }
    }
    let mut parts = Vec::new();
    if has_us {
        parts.push("US");
    }
    if has_fr {
        parts.push("FR");
    }
    if has_cl {
        parts.push("CL");
    }
    parts.join(",")
}

async fn spellings_preview_materialized(
    state: &AppState,
    kind: CardKind,
    card_key: &str,
    selected_countries: &[&'static str],
) -> Result<Vec<String>> {
    let subset_key = subset_key_for(selected_countries);
    let mut subset_keys = Vec::<String>::new();
    subset_keys.push(subset_key.clone());
    for &c in selected_countries {
        subset_keys.push(c.to_string());
    }
    subset_keys.sort();
    subset_keys.dedup();

    let mut qb: sqlx::QueryBuilder<sqlx::Sqlite> = sqlx::QueryBuilder::new(
        "SELECT subset_key, name_raw, rank FROM card_spelling_subset_top WHERE card_kind = ",
    );
    qb.push_bind(kind.as_str());
    qb.push(" AND card_key = ");
    qb.push_bind(card_key);
    qb.push(" AND subset_key IN (");
    let mut sep = qb.separated(", ");
    for k in &subset_keys {
        sep.push_bind(k);
    }
    sep.push_unseparated(")");
    qb.push(" ORDER BY subset_key ASC, rank ASC");

    let rows = qb
        .build()
        .fetch_all(&state.names_db)
        .await
        .context("load spellings subset top")?;

    let mut by_subset: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for r in rows {
        let sk: String = r.try_get("subset_key")?;
        let name_raw: String = r.try_get("name_raw")?;
        by_subset.entry(sk).or_default().push(name_raw);
    }

    // Country top-1 spellings.
    let mut country_tops = Vec::<String>::new();
    for &country in selected_countries {
        if let Some(first) = by_subset.get(country).and_then(|v| v.first())
            && !country_tops.contains(first)
        {
            country_tops.push(first.clone());
        }
    }

    let subset_list = by_subset.get(&subset_key).cloned().unwrap_or_default();
    if selected_countries.len() > 1 && country_tops.len() == 1 {
        return Ok(subset_list.into_iter().take(3).collect());
    }
    if country_tops.len() >= 3 {
        country_tops.truncate(3);
        return Ok(country_tops);
    }

    let mut out = country_tops;
    for g in subset_list {
        if out.len() >= 3 {
            break;
        }
        if !out.contains(&g) {
            out.push(g);
        }
    }
    Ok(out)
}

async fn last5_year_ranks_materialized(
    state: &AppState,
    kind: CardKind,
    card_key: &str,
) -> Result<BTreeMap<String, Vec<RankPoint>>> {
    let rows = sqlx::query(
        r#"
        SELECT country, year, rank
        FROM card_rank_last5
        WHERE card_kind = ?1 AND card_key = ?2
        ORDER BY country ASC, year ASC
        "#,
    )
    .bind(kind.as_str())
    .bind(card_key)
    .fetch_all(&state.names_db)
    .await
    .context("load card ranks")?;

    let mut out = BTreeMap::<String, Vec<RankPoint>>::new();
    for r in rows {
        let country: String = r.try_get("country")?;
        let year: i64 = r.try_get("year")?;
        let rank: i64 = r.try_get("rank")?;
        out.entry(country)
            .or_default()
            .push(RankPoint { year, rank });
    }
    Ok(out)
}
