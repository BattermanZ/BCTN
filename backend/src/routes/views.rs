use anyhow::{Context, Result};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, debug_handler};
use axum_extra::extract::CookieJar;
use serde::Serialize;
use sqlx::Row;

use crate::display::clean_spelling_display;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub(crate) struct CardRef {
    card_key: String,
    card_kind: String,
    display_name: String,
    my_action: String,
    gender: String,
}

async fn kind_for_key(state: &AppState, card_key: &str) -> Result<&'static str> {
    let exists: i64 = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM variant_group WHERE variant_group_key = ?1)",
    )
    .bind(card_key)
    .fetch_one(&state.names_db)
    .await
    .context("determine card kind")?;
    Ok(if exists != 0 { "group" } else { "concept" })
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

async fn global_gender_for_key(state: &AppState, card_key: &str) -> Result<String> {
    let is_group: i64 = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM variant_group WHERE variant_group_key = ?1)",
    )
    .bind(card_key)
    .fetch_one(&state.names_db)
    .await
    .context("determine card kind for global gender")?;

    if is_group != 0 {
        let row = sqlx::query(
            r#"
            SELECT
              COALESCE(SUM(male), 0) AS male,
              COALESCE(SUM(female), 0) AS female
            FROM variant_group_country_window_total
            WHERE variant_group_key = ?1 AND country IN ('US','FR','CL')
            "#,
        )
        .bind(card_key)
        .fetch_one(&state.names_db)
        .await
        .context("load global gender (group)")?;
        let male: i64 = row.try_get("male")?;
        let female: i64 = row.try_get("female")?;
        return Ok(gender_label(male, female));
    }

    let row = sqlx::query(
        r#"
        SELECT
          COALESCE(SUM(ct.male), 0) AS male,
          COALESCE(SUM(ct.female), 0) AS female
        FROM name_concept nc
        LEFT JOIN concept_country_window_total ct
          ON ct.concept_id = nc.id AND ct.country IN ('US','FR','CL')
        WHERE nc.concept_key = ?1
        "#,
    )
    .bind(card_key)
    .fetch_one(&state.names_db)
    .await
    .context("load global gender (concept)")?;
    let male: i64 = row.try_get("male")?;
    let female: i64 = row.try_get("female")?;
    Ok(gender_label(male, female))
}

async fn display_name(state: &AppState, card_key: &str) -> Result<String> {
    let name: String = sqlx::query_scalar(
        r#"
        SELECT ns.display
        FROM name_concept nc
        JOIN name_string ns ON ns.id = nc.preferred_name_string_id
        WHERE nc.concept_key = ?1
        "#,
    )
    .bind(card_key)
    .fetch_one(&state.names_db)
    .await
    .context("load display name")?;
    Ok(clean_spelling_display(&name))
}

async fn card_ref(state: &AppState, card_key: &str, my_action: &str) -> Result<CardRef> {
    let gender = global_gender_for_key(state, card_key).await?;
    Ok(CardRef {
        card_key: card_key.to_string(),
        card_kind: kind_for_key(state, card_key).await?.to_string(),
        display_name: display_name(state, card_key).await?,
        my_action: my_action.to_string(),
        gender,
    })
}

#[debug_handler]
pub(crate) async fn matches(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    match matches_impl(&state, jar).await {
        Ok((jar, v)) => (jar, Json(v)).into_response(),
        Err(_) => StatusCode::UNAUTHORIZED.into_response(),
    }
}

async fn matches_impl(state: &AppState, jar: CookieJar) -> Result<(CookieJar, Vec<CardRef>)> {
    let (jar, user_id, _is_admin) = super::auth::require_session_user_ready(state, jar).await?;
    let rows: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT m.card_key, s.action AS my_action
        FROM name_match m
        JOIN swipe s ON s.user_id = ?1 AND s.card_key = m.card_key
        WHERE s.action IN ('like','superlike')
        ORDER BY m.created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .context("load matches")?;
    let mut out = Vec::new();
    for (k, my_action) in rows {
        out.push(card_ref(state, &k, &my_action).await?);
    }
    Ok((jar, out))
}

#[debug_handler]
pub(crate) async fn likes(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    match likes_impl(&state, jar).await {
        Ok((jar, v)) => (jar, Json(v)).into_response(),
        Err(_) => StatusCode::UNAUTHORIZED.into_response(),
    }
}

async fn likes_impl(state: &AppState, jar: CookieJar) -> Result<(CookieJar, Vec<CardRef>)> {
    let (jar, user_id, _) = super::auth::require_session_user_ready(state, jar).await?;
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT card_key FROM swipe WHERE user_id = ?1 AND action = 'like' GROUP BY card_key ORDER BY MAX(created_at) DESC",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .context("load likes")?;
    let mut out = Vec::new();
    for k in rows {
        out.push(card_ref(state, &k, "like").await?);
    }
    Ok((jar, out))
}

#[debug_handler]
pub(crate) async fn superlikes(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    match superlikes_impl(&state, jar).await {
        Ok((jar, v)) => (jar, Json(v)).into_response(),
        Err(_) => StatusCode::UNAUTHORIZED.into_response(),
    }
}

async fn superlikes_impl(state: &AppState, jar: CookieJar) -> Result<(CookieJar, Vec<CardRef>)> {
    let (jar, user_id, _) = super::auth::require_session_user_ready(state, jar).await?;
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT card_key FROM swipe WHERE user_id = ?1 AND action = 'superlike' GROUP BY card_key ORDER BY MAX(created_at) DESC",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .context("load superlikes")?;
    let mut out = Vec::new();
    for k in rows {
        out.push(card_ref(state, &k, "superlike").await?);
    }
    Ok((jar, out))
}
