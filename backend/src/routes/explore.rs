use anyhow::{Context, Result, bail};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, debug_handler};
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::HashSet;

use crate::display::clean_spelling_display;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub(crate) struct ExploreSearchQuery {
    q: Option<String>,
    countries: Option<String>,
    gender: Option<String>, // any|male|female
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ExploreTopQuery {
    countries: Option<String>,
    gender: Option<String>, // any|male|female
    page: Option<i64>,
    page_size: Option<i64>,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct ExploreCard {
    card_key: String,
    card_kind: String,
    display_name: String,
    per_country: Vec<CountrySummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_ppm: Option<f64>,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct CountrySummary {
    country: String,
    ppm: f64,
    top100_any: bool,
    gender: String,
}

#[derive(Debug, Clone, Copy)]
enum GenderFilter {
    Any,
    Male,
    Female,
}

fn parse_gender(s: Option<String>) -> Result<GenderFilter> {
    match s.as_deref().unwrap_or("any") {
        "any" => Ok(GenderFilter::Any),
        "male" => Ok(GenderFilter::Male),
        "female" => Ok(GenderFilter::Female),
        other => bail!("invalid gender {other:?}; expected any|male|female"),
    }
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

fn matches_gender_filter(filter: GenderFilter, male: i64, female: i64) -> bool {
    match filter {
        GenderFilter::Any => true,
        GenderFilter::Male => {
            let total = male + female;
            total > 0 && (male as f64 / total as f64) >= 0.70
        }
        GenderFilter::Female => {
            let total = male + female;
            total > 0 && (female as f64 / total as f64) >= 0.70
        }
    }
}

fn ppm(total: i64, country_total_births: i64) -> f64 {
    if total <= 0 || country_total_births <= 0 {
        return 0.0;
    }
    (total as f64) * 1_000_000.0 / (country_total_births as f64)
}

fn fold_query_key(input: &str) -> String {
    use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

    let norm = input.nfkc().collect::<String>().trim().to_string();
    let strict = norm.to_lowercase();
    strict
        .nfkd()
        .filter(|&ch| !is_combining_mark(ch))
        .filter(|ch| ch.is_alphanumeric() || *ch == '-')
        .collect()
}

fn levenshtein_bounded(a: &str, b: &str, max_dist: usize) -> Option<usize> {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let lb = b_chars.len();
    let la = a_chars.len();
    if la.abs_diff(lb) > max_dist {
        return None;
    }

    let mut prev: Vec<usize> = (0..=lb).collect();
    let mut cur = vec![0usize; lb + 1];

    for (i, &a_ch) in a_chars.iter().enumerate() {
        cur[0] = i + 1;
        let mut row_min = cur[0];
        for j in 0..lb {
            let cost = if a_ch == b_chars[j] { 0 } else { 1 };
            let v = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
            cur[j + 1] = v;
            row_min = row_min.min(v);
        }
        if row_min > max_dist {
            return None;
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    if prev[lb] <= max_dist {
        Some(prev[lb])
    } else {
        None
    }
}

#[derive(Debug, Clone)]
struct CardStatsRow {
    card_key: String,
    card_kind: String,
    display_name: String,
    folded: String,
    us_total: i64,
    us_male: i64,
    us_female: i64,
    us_top: i64,
    fr_total: i64,
    fr_male: i64,
    fr_female: i64,
    fr_top: i64,
    cl_total: i64,
    cl_male: i64,
    cl_female: i64,
    cl_top: i64,
}

fn build_country_summaries(state: &AppState, r: &CardStatsRow) -> Vec<CountrySummary> {
    let totals = state.country_total_births;
    vec![
        CountrySummary {
            country: "US".to_string(),
            ppm: ppm(r.us_total, totals.us_total),
            top100_any: r.us_top != 0,
            gender: gender_label(r.us_male, r.us_female),
        },
        CountrySummary {
            country: "FR".to_string(),
            ppm: ppm(r.fr_total, totals.fr_total),
            top100_any: r.fr_top != 0,
            gender: gender_label(r.fr_male, r.fr_female),
        },
        CountrySummary {
            country: "CL".to_string(),
            ppm: ppm(r.cl_total, totals.cl_total),
            top100_any: r.cl_top != 0,
            gender: gender_label(r.cl_male, r.cl_female),
        },
    ]
}

fn max_ppm_for_selected(state: &AppState, r: &CardStatsRow, selected: &[&str]) -> f64 {
    let totals = state.country_total_births;
    let mut best: f64 = 0.0;
    for &c in selected {
        let p = match c {
            "US" => ppm(r.us_total, totals.us_total),
            "FR" => ppm(r.fr_total, totals.fr_total),
            "CL" => ppm(r.cl_total, totals.cl_total),
            _ => 0.0,
        };
        best = best.max(p);
    }
    best
}

fn matches_gender_any_selected(filter: GenderFilter, r: &CardStatsRow, selected: &[&str]) -> bool {
    if matches!(filter, GenderFilter::Any) {
        return true;
    }
    for &c in selected {
        let (m, f) = match c {
            "US" => (r.us_male, r.us_female),
            "FR" => (r.fr_male, r.fr_female),
            "CL" => (r.cl_male, r.cl_female),
            _ => (0, 0),
        };
        if matches_gender_filter(filter, m, f) {
            return true;
        }
    }
    false
}

async fn load_swiped_keys_for_candidates(
    state: &AppState,
    user_id: i64,
    keys: &[String],
) -> Result<HashSet<String>> {
    if keys.is_empty() {
        return Ok(HashSet::new());
    }

    let mut q = String::from("SELECT card_key FROM swipe WHERE user_id = ?1 AND card_key IN (");
    for i in 0..keys.len() {
        if i > 0 {
            q.push(',');
        }
        q.push('?');
        q.push_str(&(i + 2).to_string());
    }
    q.push(')');

    let mut query = sqlx::query_scalar(&q).bind(user_id);
    for k in keys {
        query = query.bind(k);
    }

    let rows: Vec<String> = query
        .fetch_all(&state.db)
        .await
        .context("load swiped keys for candidates")?;
    Ok(rows.into_iter().collect())
}

fn status_code_for_explore_error(err: &anyhow::Error) -> StatusCode {
    match err.to_string().as_str() {
        "missing session" | "invalid session" => StatusCode::UNAUTHORIZED,
        "invalid origin" | "invalid host" | "cross-origin request blocked" => StatusCode::FORBIDDEN,
        _ => StatusCode::BAD_REQUEST,
    }
}

async fn load_search_candidates(
    state: &AppState,
    query_folded: &str,
    limit: i64,
) -> Result<Vec<CardStatsRow>> {
    let prefix = if query_folded.len() >= 3 {
        &query_folded[..3]
    } else {
        query_folded
    };

    let rows = sqlx::query(
        r#"
        WITH matched_strings AS (
          SELECT id
          FROM name_string
          WHERE key_folded LIKE ?1 || '%'
             OR key_folded LIKE '%' || ?1 || '%'
          LIMIT 3000
        ),
        matched_concepts AS (
          SELECT DISTINCT nc.concept_key AS concept_key
          FROM matched_strings ms
          JOIN name_concept_member ncm ON ncm.name_string_id = ms.id
          JOIN name_concept nc ON nc.id = ncm.concept_id
        ),
        resolved AS (
          SELECT DISTINCT
            COALESCE(vgm.variant_group_key, mc.concept_key) AS card_key,
            EXISTS(SELECT 1 FROM variant_group vg WHERE vg.variant_group_key = COALESCE(vgm.variant_group_key, mc.concept_key)) AS is_group
          FROM matched_concepts mc
          LEFT JOIN variant_group_member vgm ON vgm.concept_key = mc.concept_key
        )
        SELECT
          r.card_key AS card_key,
          CASE WHEN r.is_group != 0 THEN 'group' ELSE 'concept' END AS card_kind,
          pref.display AS display_name,
          pref.key_folded AS folded,

          CASE WHEN r.is_group != 0 THEN COALESCE(vgus.total, 0) ELSE COALESCE(cus.total, 0) END AS us_total,
          CASE WHEN r.is_group != 0 THEN COALESCE(vgus.male, 0) ELSE COALESCE(cus.male, 0) END AS us_male,
          CASE WHEN r.is_group != 0 THEN COALESCE(vgus.female, 0) ELSE COALESCE(cus.female, 0) END AS us_female,
          CASE WHEN r.is_group != 0 THEN COALESCE(vgust.top100_any, 0) ELSE COALESCE(cust.top100_any, 0) END AS us_top,

          CASE WHEN r.is_group != 0 THEN COALESCE(vgfr.total, 0) ELSE COALESCE(cfr.total, 0) END AS fr_total,
          CASE WHEN r.is_group != 0 THEN COALESCE(vgfr.male, 0) ELSE COALESCE(cfr.male, 0) END AS fr_male,
          CASE WHEN r.is_group != 0 THEN COALESCE(vgfr.female, 0) ELSE COALESCE(cfr.female, 0) END AS fr_female,
          CASE WHEN r.is_group != 0 THEN COALESCE(vgfrt.top100_any, 0) ELSE COALESCE(cfrt.top100_any, 0) END AS fr_top,

          CASE WHEN r.is_group != 0 THEN COALESCE(vgcl.total, 0) ELSE COALESCE(ccl.total, 0) END AS cl_total,
          CASE WHEN r.is_group != 0 THEN COALESCE(vgcl.male, 0) ELSE COALESCE(ccl.male, 0) END AS cl_male,
          CASE WHEN r.is_group != 0 THEN COALESCE(vgcl.female, 0) ELSE COALESCE(ccl.female, 0) END AS cl_female,
          CASE WHEN r.is_group != 0 THEN COALESCE(vgclt.top100_any, 0) ELSE COALESCE(cclt.top100_any, 0) END AS cl_top
        FROM resolved r
        JOIN name_concept canon ON canon.concept_key = r.card_key
        JOIN name_string pref ON pref.id = canon.preferred_name_string_id
        LEFT JOIN concept_country_window_total cus ON cus.concept_id = canon.id AND cus.country='US'
        LEFT JOIN concept_country_top100_any cust ON cust.concept_id = canon.id AND cust.country='US'
        LEFT JOIN concept_country_window_total cfr ON cfr.concept_id = canon.id AND cfr.country='FR'
        LEFT JOIN concept_country_top100_any cfrt ON cfrt.concept_id = canon.id AND cfrt.country='FR'
        LEFT JOIN concept_country_window_total ccl ON ccl.concept_id = canon.id AND ccl.country='CL'
        LEFT JOIN concept_country_top100_any cclt ON cclt.concept_id = canon.id AND cclt.country='CL'

        LEFT JOIN variant_group_country_window_total vgus ON vgus.variant_group_key = r.card_key AND vgus.country='US'
        LEFT JOIN variant_group_country_top100_any vgust ON vgust.variant_group_key = r.card_key AND vgust.country='US'
        LEFT JOIN variant_group_country_window_total vgfr ON vgfr.variant_group_key = r.card_key AND vgfr.country='FR'
        LEFT JOIN variant_group_country_top100_any vgfrt ON vgfrt.variant_group_key = r.card_key AND vgfrt.country='FR'
        LEFT JOIN variant_group_country_window_total vgcl ON vgcl.variant_group_key = r.card_key AND vgcl.country='CL'
        LEFT JOIN variant_group_country_top100_any vgclt ON vgclt.variant_group_key = r.card_key AND vgclt.country='CL'
        WHERE pref.key_folded LIKE ?1 || '%'
           OR pref.key_folded LIKE '%' || ?1 || '%'
        LIMIT ?2
        "#,
    )
    .bind(prefix)
    .bind(limit)
    .fetch_all(&state.names_db)
    .await
    .context("load search candidates")?;

    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(CardStatsRow {
            card_key: r.try_get("card_key")?,
            card_kind: r.try_get("card_kind")?,
            display_name: r.try_get("display_name")?,
            folded: r.try_get("folded")?,
            us_total: r.try_get("us_total")?,
            us_male: r.try_get("us_male")?,
            us_female: r.try_get("us_female")?,
            us_top: r.try_get("us_top")?,
            fr_total: r.try_get("fr_total")?,
            fr_male: r.try_get("fr_male")?,
            fr_female: r.try_get("fr_female")?,
            fr_top: r.try_get("fr_top")?,
            cl_total: r.try_get("cl_total")?,
            cl_male: r.try_get("cl_male")?,
            cl_female: r.try_get("cl_female")?,
            cl_top: r.try_get("cl_top")?,
        });
    }
    Ok(out)
}

async fn load_top_candidates_ordered(
    state: &AppState,
    offset: i64,
    limit: i64,
    selected: &[&str],
) -> Result<Vec<CardStatsRow>> {
    let totals = state.country_total_births;
    let sel_us: i64 = if selected.contains(&"US") { 1 } else { 0 };
    let sel_fr: i64 = if selected.contains(&"FR") { 1 } else { 0 };
    let sel_cl: i64 = if selected.contains(&"CL") { 1 } else { 0 };

    let rows = sqlx::query(
        r#"
        WITH cards AS (
          SELECT
            'group' AS card_kind,
            vg.variant_group_key AS card_key,
            ns.display AS display_name,
            ns.key_folded AS folded,
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

          UNION ALL

          SELECT
            'concept' AS card_kind,
            nc.concept_key AS card_key,
            ns.display AS display_name,
            ns.key_folded AS folded,
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
        ),
        ranked AS (
          SELECT
            *,
            max(
              CASE WHEN ?1 != 0 THEN (us_total * 1000000.0 / ?4) ELSE 0 END,
              CASE WHEN ?2 != 0 THEN (fr_total * 1000000.0 / ?5) ELSE 0 END,
              CASE WHEN ?3 != 0 THEN (cl_total * 1000000.0 / ?6) ELSE 0 END
            ) AS max_ppm
          FROM cards
        )
        SELECT * FROM ranked
        WHERE max_ppm > 0
        ORDER BY max_ppm DESC, card_key ASC
        LIMIT ?7 OFFSET ?8
        "#,
    )
    .bind(sel_us)
    .bind(sel_fr)
    .bind(sel_cl)
    .bind(totals.us_total as f64)
    .bind(totals.fr_total as f64)
    .bind(totals.cl_total as f64)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.names_db)
    .await
    .context("load top candidates")?;

    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(CardStatsRow {
            card_key: r.try_get("card_key")?,
            card_kind: r.try_get("card_kind")?,
            display_name: r.try_get("display_name")?,
            folded: r.try_get("folded")?,
            us_total: r.try_get("us_total")?,
            us_male: r.try_get("us_male")?,
            us_female: r.try_get("us_female")?,
            us_top: r.try_get("us_top")?,
            fr_total: r.try_get("fr_total")?,
            fr_male: r.try_get("fr_male")?,
            fr_female: r.try_get("fr_female")?,
            fr_top: r.try_get("fr_top")?,
            cl_total: r.try_get("cl_total")?,
            cl_male: r.try_get("cl_male")?,
            cl_female: r.try_get("cl_female")?,
            cl_top: r.try_get("cl_top")?,
        });
    }
    Ok(out)
}

#[debug_handler]
pub(crate) async fn search(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<ExploreSearchQuery>,
) -> impl IntoResponse {
    match search_impl(&state, jar, q).await {
        Ok((jar, out)) => (jar, Json(out)).into_response(),
        Err(err) => {
            tracing::warn!(error=%err, "explore_search_failed");
            (status_code_for_explore_error(&err), err.to_string()).into_response()
        }
    }
}

async fn search_impl(
    state: &AppState,
    jar: CookieJar,
    q: ExploreSearchQuery,
) -> Result<(CookieJar, Vec<ExploreCard>)> {
    let (jar, user_id, _) = super::auth::require_session_user_ready(state, jar).await?;
    let selected_countries = parse_countries(q.countries)?;
    let gender = parse_gender(q.gender)?;

    let q_raw = q.q.unwrap_or_default();
    let q_folded = fold_query_key(&q_raw);
    if q_folded.len() < 2 {
        return Ok((jar, Vec::new()));
    }

    let page_size = q.page_size.unwrap_or(50).clamp(1, 200);
    let page = q.page.unwrap_or(0).max(0);

    let mut candidates = load_search_candidates(state, &q_folded, 1000).await?;
    let candidate_keys = candidates
        .iter()
        .map(|c| c.card_key.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let swiped_set = load_swiped_keys_for_candidates(state, user_id, &candidate_keys).await?;

    // Fuzzy scoring: prefer prefix matches, then small edit distance on folded keys.
    let max_dist = if q_folded.len() <= 4 { 1 } else { 2 };
    let mut scored = Vec::<(i64, i64, CardStatsRow)>::new();
    let mut seen = HashSet::<String>::new();
    for c in candidates.drain(..) {
        if !seen.insert(c.card_key.clone()) {
            continue;
        }
        if swiped_set.contains(&c.card_key) {
            continue;
        }
        if !matches_gender_any_selected(gender, &c, &selected_countries) {
            continue;
        }
        let prefix = if c.folded.starts_with(&q_folded) {
            0
        } else {
            1
        };
        let dist = levenshtein_bounded(&q_folded, &c.folded, max_dist).map(|d| d as i64);
        if prefix != 0 && dist.is_none() {
            continue;
        }
        let dist = dist.unwrap_or(0);
        scored.push((prefix, dist, c));
    }
    scored.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(a.1.cmp(&b.1))
            .then_with(|| {
                let ap = max_ppm_for_selected(state, &a.2, &selected_countries);
                let bp = max_ppm_for_selected(state, &b.2, &selected_countries);
                bp.partial_cmp(&ap).unwrap_or(std::cmp::Ordering::Equal)
            })
            .then(a.2.card_key.cmp(&b.2.card_key))
    });

    let start = (page * page_size) as usize;
    let end = start.saturating_add(page_size as usize);
    let slice = scored.into_iter().map(|(_, _, c)| c).collect::<Vec<_>>();
    let page_rows = slice.get(start..end).unwrap_or(&[]);

    let mut out = Vec::with_capacity(page_rows.len());
    for r in page_rows {
        let per_country = build_country_summaries(state, r);
        let max_ppm = max_ppm_for_selected(state, r, &selected_countries);
        out.push(ExploreCard {
            card_key: r.card_key.clone(),
            card_kind: r.card_kind.clone(),
            display_name: clean_spelling_display(&r.display_name),
            per_country,
            max_ppm: Some(max_ppm),
        });
    }
    Ok((jar, out))
}

#[debug_handler]
pub(crate) async fn top(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<ExploreTopQuery>,
) -> impl IntoResponse {
    match top_impl(&state, jar, q).await {
        Ok((jar, out)) => (jar, Json(out)).into_response(),
        Err(err) => {
            tracing::warn!(error=%err, "explore_top_failed");
            (status_code_for_explore_error(&err), err.to_string()).into_response()
        }
    }
}

async fn top_impl(
    state: &AppState,
    jar: CookieJar,
    q: ExploreTopQuery,
) -> Result<(CookieJar, Vec<ExploreCard>)> {
    let (jar, user_id, _) = super::auth::require_session_user_ready(state, jar).await?;
    let selected_countries = parse_countries(q.countries)?;
    let gender = parse_gender(q.gender)?;

    let page_size = q.page_size.unwrap_or(100).clamp(1, 500);
    let page = q.page.unwrap_or(0).max(0);

    let mut out = Vec::with_capacity(page_size as usize);
    let mut offset = page * page_size;
    let mut attempts = 0;
    while out.len() < page_size as usize && attempts < 6 {
        attempts += 1;
        let chunk =
            load_top_candidates_ordered(state, offset, page_size * 5, &selected_countries).await?;
        if chunk.is_empty() {
            break;
        }
        offset += page_size * 5;
        let chunk_keys = chunk.iter().map(|c| c.card_key.clone()).collect::<Vec<_>>();
        let swiped_set = load_swiped_keys_for_candidates(state, user_id, &chunk_keys).await?;
        for r in chunk {
            if swiped_set.contains(&r.card_key) {
                continue;
            }
            if !matches_gender_any_selected(gender, &r, &selected_countries) {
                continue;
            }
            let per_country = build_country_summaries(state, &r);
            let max_ppm = max_ppm_for_selected(state, &r, &selected_countries);
            out.push(ExploreCard {
                card_key: r.card_key,
                card_kind: r.card_kind,
                display_name: clean_spelling_display(&r.display_name),
                per_country,
                max_ppm: Some(max_ppm),
            });
            if out.len() >= page_size as usize {
                break;
            }
        }
    }

    Ok((jar, out))
}
