use anyhow::{Context, Result};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, debug_handler};
use axum_extra::extract::CookieJar;
use serde::Serialize;

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub(crate) struct CardRef {
    card_key: String,
    card_kind: String,
    display_name: String,
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
    Ok(name)
}

async fn card_ref(state: &AppState, card_key: &str) -> Result<CardRef> {
    Ok(CardRef {
        card_key: card_key.to_string(),
        card_kind: kind_for_key(state, card_key).await?.to_string(),
        display_name: display_name(state, card_key).await?,
    })
}

#[debug_handler]
pub(crate) async fn matches(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    match matches_impl(&state, &jar).await {
        Ok(v) => Json(v).into_response(),
        Err(_) => StatusCode::UNAUTHORIZED.into_response(),
    }
}

async fn matches_impl(state: &AppState, jar: &CookieJar) -> Result<Vec<CardRef>> {
    super::auth::require_session_user(state, jar).await?;
    let rows: Vec<String> =
        sqlx::query_scalar("SELECT card_key FROM name_match ORDER BY created_at DESC")
            .fetch_all(&state.db)
            .await
            .context("load matches")?;
    let mut out = Vec::new();
    for k in rows {
        out.push(card_ref(state, &k).await?);
    }
    Ok(out)
}

#[debug_handler]
pub(crate) async fn likes(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    match likes_impl(&state, &jar).await {
        Ok(v) => Json(v).into_response(),
        Err(_) => StatusCode::UNAUTHORIZED.into_response(),
    }
}

async fn likes_impl(state: &AppState, jar: &CookieJar) -> Result<Vec<CardRef>> {
    super::auth::require_session_user(state, jar).await?;
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT card_key FROM swipe WHERE action = 'like' GROUP BY card_key ORDER BY MAX(created_at) DESC",
    )
    .fetch_all(&state.db)
    .await
    .context("load likes")?;
    let mut out = Vec::new();
    for k in rows {
        out.push(card_ref(state, &k).await?);
    }
    Ok(out)
}

#[debug_handler]
pub(crate) async fn superlikes(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    match superlikes_impl(&state, &jar).await {
        Ok(v) => Json(v).into_response(),
        Err(_) => StatusCode::UNAUTHORIZED.into_response(),
    }
}

async fn superlikes_impl(state: &AppState, jar: &CookieJar) -> Result<Vec<CardRef>> {
    super::auth::require_session_user(state, jar).await?;
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT card_key FROM swipe WHERE action = 'superlike' GROUP BY card_key ORDER BY MAX(created_at) DESC",
    )
    .fetch_all(&state.db)
    .await
    .context("load superlikes")?;
    let mut out = Vec::new();
    for k in rows {
        out.push(card_ref(state, &k).await?);
    }
    Ok(out)
}
