use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, debug_handler};
use axum_extra::extract::CookieJar;
use serde::Serialize;
use sqlx::Row;

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub(crate) struct UserSummary {
    id: i64,
    username: String,
    is_admin: bool,
}

#[debug_handler]
pub(crate) async fn list(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    match list_impl(&state, &jar).await {
        Ok(v) => Json(v).into_response(),
        Err(_) => StatusCode::UNAUTHORIZED.into_response(),
    }
}

async fn list_impl(state: &AppState, jar: &CookieJar) -> anyhow::Result<Vec<UserSummary>> {
    super::auth::require_session_user_ready(state, jar).await?;
    let rows = sqlx::query(
        "SELECT id, username, is_admin FROM app_user WHERE deleted_at IS NULL ORDER BY id",
    )
    .fetch_all(&state.db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| UserSummary {
            id: r.try_get("id").unwrap_or_default(),
            username: r.try_get("username").unwrap_or_default(),
            is_admin: r.try_get::<i64, _>("is_admin").unwrap_or(0) != 0,
        })
        .collect())
}
