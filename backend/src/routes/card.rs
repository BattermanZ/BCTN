use std::collections::BTreeMap;

use anyhow::{Context, Result};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, debug_handler};
use axum_extra::extract::CookieJar;
use serde::Serialize;
use sqlx::{QueryBuilder, Row, Sqlite};

use crate::state::{AppState, CountryTotals};

#[derive(Debug, Serialize)]
pub(crate) struct CardDetailsResponse {
    card_key: String,
    card_kind: String,
    display_name: String,
    spellings_preview: Vec<String>,
    score: Option<i64>,
    per_country: Vec<CountrySummary>,
    sex_split: BTreeMap<String, SexSplit>,
    all_spellings: Vec<String>,
    within_concept_variants: Vec<String>,
    last5_year_ranks: BTreeMap<String, Vec<RankPoint>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CountrySummary {
    country: String,
    ppm: f64,
    top100_any: bool,
    gender: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct SexSplit {
    male: i64,
    female: i64,
    male_pct: f64,
    female_pct: f64,
}

#[derive(Debug, Serialize)]
pub(crate) struct RankPoint {
    year: i64,
    rank: i64,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct CardQuery {
    /// Comma-separated list of evidence countries (subset of US,FR,CL).
    countries: Option<String>,

    /// Page of spellings to return (0-based).
    page: Option<i64>,

    /// Page size for spellings (clamped to 1..=500).
    page_size: Option<i64>,
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

fn ppm(total: i64, country_total_births: i64) -> f64 {
    if total <= 0 || country_total_births <= 0 {
        return 0.0;
    }
    (total as f64) * 1_000_000.0 / (country_total_births as f64)
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

#[derive(Debug, Clone, Copy)]
enum Tier {
    ExtraRare,
    Rare,
    Common,
    Top,
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

fn score_best_picks(
    totals: &CountryTotals,
    us_total: i64,
    us_top: bool,
    fr_total: i64,
    fr_top: bool,
    cl_total: i64,
    cl_top: bool,
) -> i64 {
    let ppm_us = ppm(us_total, totals.us_total);
    let ppm_fr = ppm(fr_total, totals.fr_total);
    let ppm_cl = ppm(cl_total, totals.cl_total);

    let tier_us = tier_from_ppm(ppm_us, us_top);
    let tier_fr = tier_from_ppm(ppm_fr, fr_top);
    let tier_cl = tier_from_ppm(ppm_cl, cl_top);

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

    let top_count = [us_top, fr_top, cl_top].into_iter().filter(|&v| v).count() as i64;
    let penalty_top = 5 * top_count;

    let sweet =
        sweet_spot(ppm_fr, fr_top) + sweet_spot(ppm_us, us_top) + sweet_spot(ppm_cl, cl_top);

    base + bonus_present + sweet - penalty_top
}

async fn detect_kind(state: &AppState, card_key: &str) -> Result<CardKind> {
    let exists: i64 = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM variant_group WHERE variant_group_key = ?1)",
    )
    .bind(card_key)
    .fetch_one(&state.names_db)
    .await
    .context("detect card kind")?;
    Ok(if exists != 0 {
        CardKind::Group
    } else {
        CardKind::Concept
    })
}

async fn concept_id_for_key(state: &AppState, concept_key: &str) -> Result<i64> {
    let id: i64 = sqlx::query_scalar("SELECT id FROM name_concept WHERE concept_key = ?1")
        .bind(concept_key)
        .fetch_one(&state.names_db)
        .await
        .context("load concept id")?;
    Ok(id)
}

async fn display_name_for_concept(state: &AppState, concept_key: &str) -> Result<String> {
    let name: String = sqlx::query_scalar(
        r#"
        SELECT ns.display
        FROM name_concept nc
        JOIN name_string ns ON ns.id = nc.preferred_name_string_id
        WHERE nc.concept_key = ?1
        "#,
    )
    .bind(concept_key)
    .fetch_one(&state.names_db)
    .await
    .context("load display name")?;
    Ok(name)
}

async fn within_concept_variants(state: &AppState, concept_key: &str) -> Result<Vec<String>> {
    let rows: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT ns.display
        FROM name_concept nc
        JOIN name_concept_member ncm ON ncm.concept_id = nc.id
        JOIN name_string ns ON ns.id = ncm.name_string_id
        WHERE nc.concept_key = ?1
        ORDER BY ns.display
        "#,
    )
    .bind(concept_key)
    .fetch_all(&state.names_db)
    .await
    .context("load within concept variants")?;

    let preferred = display_name_for_concept(state, concept_key).await?;
    Ok(rows.into_iter().filter(|s| s != &preferred).collect())
}

async fn all_spellings_materialized(
    state: &AppState,
    kind: CardKind,
    card_key: &str,
    page: i64,
    page_size: i64,
) -> Result<Vec<String>> {
    let limit = page_size.clamp(1, 500);
    let offset = page.saturating_mul(limit).max(0);
    let rows: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT name_raw
        FROM card_spelling_total_all
        WHERE card_kind = ?1 AND card_key = ?2
        ORDER BY total DESC, name_raw ASC
        LIMIT ?3 OFFSET ?4
        "#,
    )
    .bind(kind.as_str())
    .bind(card_key)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.names_db)
    .await
    .context("load all spellings (materialized)")?;
    Ok(rows)
}

async fn per_country_for_concept(state: &AppState, concept_id: i64) -> Result<Vec<CountrySummary>> {
    async fn load_for(
        state: &AppState,
        concept_id: i64,
        country: &str,
    ) -> Result<(i64, i64, i64, bool)> {
        let row = sqlx::query(
            r#"
            SELECT
              COALESCE(ct.total, 0) AS total,
              COALESCE(ct.male, 0) AS male,
              COALESCE(ct.female, 0) AS female,
              COALESCE(t100.top100_any, 0) AS top100_any
            FROM (SELECT 1) one
            LEFT JOIN concept_country_window_total ct ON ct.concept_id = ?1 AND ct.country = ?2
            LEFT JOIN concept_country_top100_any t100 ON t100.concept_id = ?1 AND t100.country = ?2
            "#,
        )
        .bind(concept_id)
        .bind(country)
        .fetch_one(&state.names_db)
        .await?;
        Ok((
            row.try_get("total")?,
            row.try_get("male")?,
            row.try_get("female")?,
            row.try_get::<i64, _>("top100_any")? != 0,
        ))
    }

    let totals = state.country_total_births;
    let mut out = Vec::new();
    for country in ["US", "FR", "CL"] {
        let (total, male, female, top100_any) = load_for(state, concept_id, country).await?;
        let country_total = match country {
            "US" => totals.us_total,
            "FR" => totals.fr_total,
            "CL" => totals.cl_total,
            _ => 0,
        };
        out.push(CountrySummary {
            country: country.to_string(),
            ppm: ppm(total, country_total),
            top100_any,
            gender: gender_label(male, female),
        });
    }
    Ok(out)
}

async fn sex_split_for_concept(
    state: &AppState,
    concept_id: i64,
) -> Result<BTreeMap<String, SexSplit>> {
    let rows = sqlx::query(
        r#"
        SELECT
          ct.country AS country,
          COALESCE(ct.male, 0) AS male,
          COALESCE(ct.female, 0) AS female
        FROM concept_country_window_total ct
        WHERE ct.concept_id = ?1 AND ct.country IN ('US','FR','CL')
        "#,
    )
    .bind(concept_id)
    .fetch_all(&state.names_db)
    .await
    .context("load sex split (concept)")?;

    let mut out = BTreeMap::<String, SexSplit>::new();
    for country in ["US", "FR", "CL"] {
        let mut male = 0i64;
        let mut female = 0i64;
        for r in &rows {
            let c: String = r.try_get("country")?;
            if c == country {
                male = r.try_get("male")?;
                female = r.try_get("female")?;
                break;
            }
        }
        let total = male + female;
        let (male_pct, female_pct) = if total <= 0 {
            (0.0, 0.0)
        } else {
            (
                male as f64 * 100.0 / total as f64,
                female as f64 * 100.0 / total as f64,
            )
        };
        out.insert(
            country.to_string(),
            SexSplit {
                male,
                female,
                male_pct,
                female_pct,
            },
        );
    }
    Ok(out)
}

async fn per_country_for_group(state: &AppState, group_key: &str) -> Result<Vec<CountrySummary>> {
    async fn load_for(
        state: &AppState,
        group_key: &str,
        country: &str,
    ) -> Result<(i64, i64, i64, bool)> {
        let row = sqlx::query(
            r#"
            SELECT
              COALESCE(gt.total, 0) AS total,
              COALESCE(gt.male, 0) AS male,
              COALESCE(gt.female, 0) AS female,
              COALESCE(t100.top100_any, 0) AS top100_any
            FROM (SELECT 1) one
            LEFT JOIN variant_group_country_window_total gt ON gt.variant_group_key = ?1 AND gt.country = ?2
            LEFT JOIN variant_group_country_top100_any t100 ON t100.variant_group_key = ?1 AND t100.country = ?2
            "#,
        )
        .bind(group_key)
        .bind(country)
        .fetch_one(&state.names_db)
        .await?;
        Ok((
            row.try_get("total")?,
            row.try_get("male")?,
            row.try_get("female")?,
            row.try_get::<i64, _>("top100_any")? != 0,
        ))
    }

    let totals = state.country_total_births;
    let mut out = Vec::new();
    for country in ["US", "FR", "CL"] {
        let (total, male, female, top100_any) = load_for(state, group_key, country).await?;
        let country_total = match country {
            "US" => totals.us_total,
            "FR" => totals.fr_total,
            "CL" => totals.cl_total,
            _ => 0,
        };
        out.push(CountrySummary {
            country: country.to_string(),
            ppm: ppm(total, country_total),
            top100_any,
            gender: gender_label(male, female),
        });
    }
    Ok(out)
}

async fn sex_split_for_group(
    state: &AppState,
    group_key: &str,
) -> Result<BTreeMap<String, SexSplit>> {
    let rows = sqlx::query(
        r#"
        SELECT
          gt.country AS country,
          COALESCE(gt.male, 0) AS male,
          COALESCE(gt.female, 0) AS female
        FROM variant_group_country_window_total gt
        WHERE gt.variant_group_key = ?1 AND gt.country IN ('US','FR','CL')
        "#,
    )
    .bind(group_key)
    .fetch_all(&state.names_db)
    .await
    .context("load sex split (group)")?;

    let mut out = BTreeMap::<String, SexSplit>::new();
    for country in ["US", "FR", "CL"] {
        let mut male = 0i64;
        let mut female = 0i64;
        for r in &rows {
            let c: String = r.try_get("country")?;
            if c == country {
                male = r.try_get("male")?;
                female = r.try_get("female")?;
                break;
            }
        }
        let total = male + female;
        let (male_pct, female_pct) = if total <= 0 {
            (0.0, 0.0)
        } else {
            (
                male as f64 * 100.0 / total as f64,
                female as f64 * 100.0 / total as f64,
            )
        };
        out.insert(
            country.to_string(),
            SexSplit {
                male,
                female,
                male_pct,
                female_pct,
            },
        );
    }
    Ok(out)
}

async fn best_pick_mode(state: &AppState, user_id: i64) -> Result<bool> {
    let best_pick_mode: i64 =
        sqlx::query_scalar("SELECT best_pick_mode FROM user_settings WHERE user_id = ?1")
            .bind(user_id)
            .fetch_optional(&state.db)
            .await
            .context("load best_pick_mode")?
            .unwrap_or(0);
    Ok(best_pick_mode != 0)
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

fn parse_countries(s: Option<String>) -> Result<Vec<&'static str>> {
    let raw = s.unwrap_or_else(|| "US,FR,CL".to_string());
    let mut out = Vec::new();
    for part in raw.split(',').map(|x| x.trim()).filter(|x| !x.is_empty()) {
        match part {
            "US" => out.push("US"),
            "FR" => out.push("FR"),
            "CL" => out.push("CL"),
            other => anyhow::bail!("invalid country {other:?}; expected US,FR,CL"),
        }
    }
    if out.is_empty() {
        anyhow::bail!("countries must not be empty");
    }
    Ok(out)
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

    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
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

    let rows = qb.build().fetch_all(&state.names_db).await?;
    let mut by_subset: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in rows {
        let sk: String = r.try_get("subset_key")?;
        let name_raw: String = r.try_get("name_raw")?;
        by_subset.entry(sk).or_default().push(name_raw);
    }

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
        let mut out = country_tops;
        out.truncate(3);
        return Ok(out);
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
    .context("load card ranks (materialized)")?;

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

#[debug_handler]
pub(crate) async fn get(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<CardQuery>,
    Path(card_key): Path<String>,
) -> impl IntoResponse {
    match get_impl(&state, &jar, &card_key, q).await {
        Ok(v) => Json(v).into_response(),
        Err(err) => {
            tracing::warn!(error=%err, "card_get_failed");
            (StatusCode::BAD_REQUEST, err.to_string()).into_response()
        }
    }
}

async fn get_impl(
    state: &AppState,
    jar: &CookieJar,
    card_key: &str,
    q: CardQuery,
) -> Result<CardDetailsResponse> {
    let (user_id, _) = super::auth::require_session_user(state, jar).await?;
    let kind = detect_kind(state, card_key).await?;
    let selected_countries = parse_countries(q.countries)?;

    let display_name = match kind {
        CardKind::Group => display_name_for_concept(state, card_key).await?,
        CardKind::Concept => display_name_for_concept(state, card_key).await?,
    };

    let per_country = match kind {
        CardKind::Group => per_country_for_group(state, card_key).await?,
        CardKind::Concept => {
            let concept_id = concept_id_for_key(state, card_key).await?;
            per_country_for_concept(state, concept_id).await?
        }
    };
    let sex_split = match kind {
        CardKind::Group => sex_split_for_group(state, card_key).await?,
        CardKind::Concept => {
            let concept_id = concept_id_for_key(state, card_key).await?;
            sex_split_for_concept(state, concept_id).await?
        }
    };

    let best_pick = best_pick_mode(state, user_id).await?;
    let score = if best_pick {
        let by = |c: &str| {
            per_country
                .iter()
                .find(|x| x.country == c)
                .map(|x| (x.ppm, x.top100_any))
                .unwrap_or((0.0, false))
        };
        let (ppm_us, top_us) = by("US");
        let (ppm_fr, top_fr) = by("FR");
        let (ppm_cl, top_cl) = by("CL");

        let totals = state.country_total_births;
        Some(score_best_picks(
            &totals,
            ((ppm_us * totals.us_total as f64) / 1_000_000.0) as i64,
            top_us,
            ((ppm_fr * totals.fr_total as f64) / 1_000_000.0) as i64,
            top_fr,
            ((ppm_cl * totals.cl_total as f64) / 1_000_000.0) as i64,
            top_cl,
        ))
    } else {
        None
    };

    let page = q.page.unwrap_or(0).max(0);
    let page_size = q.page_size.unwrap_or(200).clamp(1, 500);
    let all_spellings = all_spellings_materialized(state, kind, card_key, page, page_size).await?;
    let spellings_preview =
        spellings_preview_materialized(state, kind, card_key, &selected_countries).await?;

    let within_concept_variants = match kind {
        CardKind::Concept => within_concept_variants(state, card_key)
            .await
            .unwrap_or_default(),
        CardKind::Group => within_concept_variants(state, card_key)
            .await
            .unwrap_or_default(),
    };
    let last5_year_ranks = last5_year_ranks_materialized(state, kind, card_key).await?;

    Ok(CardDetailsResponse {
        card_key: card_key.to_string(),
        card_kind: kind.as_str().to_string(),
        display_name,
        spellings_preview,
        score,
        per_country,
        sex_split,
        all_spellings,
        within_concept_variants,
        last5_year_ranks,
    })
}
