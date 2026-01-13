use anyhow::{Context, Result};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, debug_handler};
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub(crate) struct SettingsResponse {
    show_last_name: bool,
    last_name: String,
    best_pick_mode: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PatchSettingsRequest {
    show_last_name: Option<bool>,
    last_name: Option<String>,
    best_pick_mode: Option<bool>,
}

#[debug_handler]
pub(crate) async fn get_settings(
    State(state): State<AppState>,
    jar: CookieJar,
) -> impl IntoResponse {
    match get_settings_impl(&state, &jar).await {
        Ok(settings) => Json(settings).into_response(),
        Err(_) => StatusCode::UNAUTHORIZED.into_response(),
    }
}

async fn get_settings_impl(state: &AppState, jar: &CookieJar) -> Result<SettingsResponse> {
    let (user_id, _) = super::auth::require_session_user_ready(state, jar).await?;
    let row = sqlx::query(
        "SELECT show_last_name, last_name, best_pick_mode FROM user_settings WHERE user_id = ?1",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await
    .context("load settings")?;
    Ok(SettingsResponse {
        show_last_name: row.try_get::<i64, _>("show_last_name")? != 0,
        last_name: row.try_get("last_name")?,
        best_pick_mode: row.try_get::<i64, _>("best_pick_mode")? != 0,
    })
}

#[debug_handler]
pub(crate) async fn patch_settings(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(req): Json<PatchSettingsRequest>,
) -> impl IntoResponse {
    match patch_settings_impl(&state, &jar, req).await {
        Ok(settings) => Json(settings).into_response(),
        Err(err) => {
            tracing::warn!(error=%err, "patch_settings_failed");
            StatusCode::BAD_REQUEST.into_response()
        }
    }
}

async fn patch_settings_impl(
    state: &AppState,
    jar: &CookieJar,
    req: PatchSettingsRequest,
) -> Result<SettingsResponse> {
    let (user_id, _) = super::auth::require_session_user_ready(state, jar).await?;

    if let Some(ref ln) = req.last_name
        && (ln.trim().is_empty() || ln.len() > 64)
    {
        anyhow::bail!("invalid last_name");
    }

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO user_settings (user_id) VALUES (?1)
        "#,
    )
    .bind(user_id)
    .execute(&state.db)
    .await
    .ok();

    if req.show_last_name.is_some() || req.last_name.is_some() || req.best_pick_mode.is_some() {
        sqlx::query(
            r#"
            UPDATE user_settings
            SET
              show_last_name = COALESCE(?2, show_last_name),
              last_name = COALESCE(?3, last_name),
              best_pick_mode = COALESCE(?4, best_pick_mode)
            WHERE user_id = ?1
            "#,
        )
        .bind(user_id)
        .bind(req.show_last_name.map(|v| if v { 1i64 } else { 0i64 }))
        .bind(req.last_name.as_deref())
        .bind(req.best_pick_mode.map(|v| if v { 1i64 } else { 0i64 }))
        .execute(&state.db)
        .await
        .context("update settings")?;
    }

    get_settings_impl(state, jar).await
}
