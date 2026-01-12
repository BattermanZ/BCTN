use std::collections::BTreeMap;

use anyhow::{Context, Result};
use axum::extract::{Path, State};
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
pub(crate) struct RankPoint {
    year: i64,
    rank: i64,
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

async fn all_spellings_for_concept(
    state: &AppState,
    concept_key: &str,
    limit: i64,
) -> Result<Vec<String>> {
    let run_id = state.names_window.run_id;
    let rows: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT ns.display
        FROM name_concept nc
        JOIN name_concept_member ncm ON ncm.concept_id = nc.id
        JOIN name_string ns ON ns.id = ncm.name_string_id
        LEFT JOIN (
          SELECT name_string_id, SUM(count) AS total
          FROM name_observation
          WHERE run_id = ?2
          GROUP BY name_string_id
        ) obs ON obs.name_string_id = ns.id
        WHERE nc.concept_key = ?1
        ORDER BY COALESCE(obs.total, 0) DESC, ns.display ASC
        LIMIT ?3
        "#,
    )
    .bind(concept_key)
    .bind(run_id)
    .bind(limit)
    .fetch_all(&state.names_db)
    .await
    .context("load all spellings for concept")?;
    Ok(rows)
}

async fn all_spellings_for_group(
    state: &AppState,
    group_key: &str,
    limit: i64,
) -> Result<Vec<String>> {
    let run_id = state.names_window.run_id;
    let rows: Vec<String> = sqlx::query_scalar(
        r#"
        WITH members AS (
          SELECT nc.id AS concept_id
          FROM variant_group_member vgm
          JOIN name_concept nc ON nc.concept_key = vgm.concept_key
          WHERE vgm.variant_group_key = ?1
        ),
        strings AS (
          SELECT DISTINCT ncm.name_string_id AS name_string_id
          FROM members m
          JOIN name_concept_member ncm ON ncm.concept_id = m.concept_id
        ),
        totals AS (
          SELECT no.name_string_id AS name_string_id, SUM(no.count) AS total
          FROM name_observation no
          JOIN strings s ON s.name_string_id = no.name_string_id
          WHERE no.run_id = ?2
          GROUP BY no.name_string_id
        )
        SELECT ns.display
        FROM strings s
        JOIN name_string ns ON ns.id = s.name_string_id
        LEFT JOIN totals t ON t.name_string_id = s.name_string_id
        ORDER BY COALESCE(t.total, 0) DESC, ns.display ASC
        LIMIT ?3
        "#,
    )
    .bind(group_key)
    .bind(run_id)
    .bind(limit)
    .fetch_all(&state.names_db)
    .await
    .context("load all spellings for group")?;
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

async fn last_years(state: &AppState, country: &str, n: i64) -> Result<Vec<i64>> {
    let run_id = state.names_window.run_id;
    let rows: Vec<i64> = sqlx::query_scalar(
        "SELECT DISTINCT year FROM name_observation WHERE run_id=?1 AND country=?2 ORDER BY year DESC LIMIT ?3",
    )
    .bind(run_id)
    .bind(country)
    .bind(n)
    .fetch_all(&state.names_db)
    .await
    .context("load last years")?;
    let mut years = rows;
    years.sort();
    Ok(years)
}

async fn ranks_for_concept_years(
    state: &AppState,
    concept_id: i64,
    country: &str,
    years: &[i64],
) -> Result<Vec<RankPoint>> {
    if years.is_empty() {
        return Ok(Vec::new());
    }
    let run_id = state.names_window.run_id;

    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        r#"
        WITH concept_year AS (
          SELECT
            ncm.concept_id AS concept_id,
            no.year AS year,
            SUM(no.count) AS total
          FROM name_observation no
          JOIN name_concept_member ncm ON ncm.name_string_id = no.name_string_id
          WHERE no.run_id =
        "#,
    );
    qb.push_bind(run_id);
    qb.push(" AND no.country = ");
    qb.push_bind(country);
    qb.push(" AND no.year IN (");
    let mut separated = qb.separated(", ");
    for y in years {
        separated.push_bind(y);
    }
    separated.push_unseparated(")");
    qb.push(
        r#"
          GROUP BY ncm.concept_id, no.year
        ),
        ranked AS (
          SELECT
            concept_id,
            year,
            total,
            DENSE_RANK() OVER (PARTITION BY year ORDER BY total DESC) AS rnk
          FROM concept_year
        )
        SELECT year, rnk
        FROM ranked
        WHERE concept_id =
        "#,
    );
    qb.push_bind(concept_id);
    qb.push(" ORDER BY year ASC");

    let rows = qb.build().fetch_all(&state.names_db).await?;
    Ok(rows
        .into_iter()
        .map(|r| RankPoint {
            year: r.try_get::<i64, _>("year").unwrap_or_default(),
            rank: r.try_get::<i64, _>("rnk").unwrap_or_default(),
        })
        .collect())
}

async fn ranks_for_group_years(
    state: &AppState,
    group_key: &str,
    country: &str,
    years: &[i64],
) -> Result<Vec<RankPoint>> {
    if years.is_empty() {
        return Ok(Vec::new());
    }
    let run_id = state.names_window.run_id;

    let mut qb: QueryBuilder<Sqlite> = QueryBuilder::new(
        r#"
        WITH concept_year AS (
          SELECT
            ncm.concept_id AS concept_id,
            no.year AS year,
            SUM(no.count) AS total
          FROM name_observation no
          JOIN name_concept_member ncm ON ncm.name_string_id = no.name_string_id
          WHERE no.run_id =
        "#,
    );
    qb.push_bind(run_id);
    qb.push(" AND no.country = ");
    qb.push_bind(country);
    qb.push(" AND no.year IN (");
    let mut separated = qb.separated(", ");
    for y in years {
        separated.push_bind(y);
    }
    separated.push_unseparated(")");
    qb.push(
        r#"
          GROUP BY ncm.concept_id, no.year
        ),
        group_year AS (
          SELECT
            vgm.variant_group_key AS group_key,
            cy.year AS year,
            SUM(cy.total) AS total
          FROM concept_year cy
          JOIN name_concept nc ON nc.id = cy.concept_id
          JOIN variant_group_member vgm ON vgm.concept_key = nc.concept_key
          GROUP BY vgm.variant_group_key, cy.year
        ),
        ranked AS (
          SELECT
            group_key,
            year,
            total,
            DENSE_RANK() OVER (PARTITION BY year ORDER BY total DESC) AS rnk
          FROM group_year
        )
        SELECT year, rnk
        FROM ranked
        WHERE group_key =
        "#,
    );
    qb.push_bind(group_key);
    qb.push(" ORDER BY year ASC");

    let rows = qb.build().fetch_all(&state.names_db).await?;
    Ok(rows
        .into_iter()
        .map(|r| RankPoint {
            year: r.try_get::<i64, _>("year").unwrap_or_default(),
            rank: r.try_get::<i64, _>("rnk").unwrap_or_default(),
        })
        .collect())
}

fn preview_from_all(all: &[String]) -> Vec<String> {
    all.iter().take(3).cloned().collect()
}

#[debug_handler]
pub(crate) async fn get(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(card_key): Path<String>,
) -> impl IntoResponse {
    match get_impl(&state, &jar, &card_key).await {
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
) -> Result<CardDetailsResponse> {
    let (user_id, _) = super::auth::require_session_user(state, jar).await?;
    let kind = detect_kind(state, card_key).await?;

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

    let all_spellings = match kind {
        CardKind::Group => all_spellings_for_group(state, card_key, 200).await?,
        CardKind::Concept => all_spellings_for_concept(state, card_key, 200).await?,
    };
    let spellings_preview = preview_from_all(&all_spellings);

    let within_concept_variants = within_concept_variants(state, card_key)
        .await
        .unwrap_or_default();

    let mut last5_year_ranks = BTreeMap::<String, Vec<RankPoint>>::new();
    for country in ["US", "FR", "CL"] {
        let years = last_years(state, country, 5).await?;
        let ranks = match kind {
            CardKind::Group => ranks_for_group_years(state, card_key, country, &years).await?,
            CardKind::Concept => {
                let concept_id = concept_id_for_key(state, card_key).await?;
                ranks_for_concept_years(state, concept_id, country, &years).await?
            }
        };
        if !ranks.is_empty() {
            last5_year_ranks.insert(country.to_string(), ranks);
        }
    }

    Ok(CardDetailsResponse {
        card_key: card_key.to_string(),
        card_kind: kind.as_str().to_string(),
        display_name,
        spellings_preview,
        score,
        per_country,
        all_spellings,
        within_concept_variants,
        last5_year_ranks,
    })
}
