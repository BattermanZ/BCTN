use anyhow::{Context, Result};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, debug_handler};
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::state::AppState;

fn parse_filter_countries(s: &str) -> Result<String> {
    let mut out: Vec<&'static str> = Vec::new();
    for part in s.split(',').map(|x| x.trim()).filter(|x| !x.is_empty()) {
        match part {
            "US" => {
                if !out.contains(&"US") {
                    out.push("US");
                }
            }
            "FR" => {
                if !out.contains(&"FR") {
                    out.push("FR");
                }
            }
            "CL" => {
                if !out.contains(&"CL") {
                    out.push("CL");
                }
            }
            other => anyhow::bail!("invalid country {other:?}; expected US,FR,CL"),
        }
    }
    if out.is_empty() {
        anyhow::bail!("countries must not be empty");
    }
    Ok(out.join(","))
}

fn parse_filter_popularity(s: &str) -> Result<String> {
    let mut out: Vec<&'static str> = Vec::new();
    for part in s.split(',').map(|x| x.trim()).filter(|x| !x.is_empty()) {
        match part {
            "extra_rare" => {
                if !out.contains(&"extra_rare") {
                    out.push("extra_rare");
                }
            }
            "rare" => {
                if !out.contains(&"rare") {
                    out.push("rare");
                }
            }
            "common" => {
                if !out.contains(&"common") {
                    out.push("common");
                }
            }
            "top" => {
                if !out.contains(&"top") {
                    out.push("top");
                }
            }
            other => anyhow::bail!("invalid popularity tier {other:?}"),
        }
    }
    if out.is_empty() {
        anyhow::bail!("popularity must not be empty");
    }
    Ok(out.join(","))
}

fn parse_filter_gender(s: &str) -> Result<String> {
    match s {
        "any" | "unisex" | "mostly_male" | "mostly_female" | "male" | "female" => Ok(s.to_string()),
        other => anyhow::bail!("invalid gender {other:?}"),
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct SettingsResponse {
    show_last_name: bool,
    last_name: String,
    best_pick_mode: bool,
    filter_countries: String,
    filter_popularity: String,
    filter_gender: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PatchSettingsRequest {
    show_last_name: Option<bool>,
    last_name: Option<String>,
    best_pick_mode: Option<bool>,
    filter_countries: Option<String>,
    filter_popularity: Option<String>,
    filter_gender: Option<String>,
}

#[debug_handler]
pub(crate) async fn get_settings(
    State(state): State<AppState>,
    jar: CookieJar,
) -> impl IntoResponse {
    match get_settings_impl(&state, jar).await {
        Ok((jar, settings)) => (jar, Json(settings)).into_response(),
        Err(_) => StatusCode::UNAUTHORIZED.into_response(),
    }
}

async fn get_settings_impl(
    state: &AppState,
    jar: CookieJar,
) -> Result<(CookieJar, SettingsResponse)> {
    let (jar, user_id, _) = super::auth::require_session_user_ready(state, jar).await?;
    let row = sqlx::query(
        "SELECT show_last_name, last_name, best_pick_mode, filter_countries, filter_popularity, filter_gender FROM user_settings WHERE user_id = ?1",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await
    .context("load settings")?;
    Ok((
        jar,
        SettingsResponse {
            show_last_name: row.try_get::<i64, _>("show_last_name")? != 0,
            last_name: row.try_get("last_name")?,
            best_pick_mode: row.try_get::<i64, _>("best_pick_mode")? != 0,
            filter_countries: row.try_get("filter_countries")?,
            filter_popularity: row.try_get("filter_popularity")?,
            filter_gender: row.try_get("filter_gender")?,
        },
    ))
}

#[debug_handler]
pub(crate) async fn patch_settings(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Json(req): Json<PatchSettingsRequest>,
) -> impl IntoResponse {
    if let Err(err) = super::auth::ensure_same_origin(&state, &headers) {
        tracing::warn!(
            error=%err,
            user_msg="Request blocked for security reasons (origin check failed).",
            "csrf_blocked"
        );
        return StatusCode::FORBIDDEN.into_response();
    }
    match patch_settings_impl(&state, jar, req).await {
        Ok((jar, settings)) => (jar, Json(settings)).into_response(),
        Err(err) => {
            tracing::warn!(
                error=%err,
                user_msg="Could not update settings. Try again.",
                "patch_settings_failed"
            );
            StatusCode::BAD_REQUEST.into_response()
        }
    }
}

async fn patch_settings_impl(
    state: &AppState,
    jar: CookieJar,
    mut req: PatchSettingsRequest,
) -> Result<(CookieJar, SettingsResponse)> {
    let (jar, user_id, _) = super::auth::require_session_user_ready(state, jar).await?;

    if let Some(ref ln) = req.last_name
        && (ln.trim().is_empty() || ln.len() > 64)
    {
        anyhow::bail!("invalid last_name");
    }
    if let Some(ref v) = req.filter_countries {
        req.filter_countries = Some(parse_filter_countries(v)?);
    }
    if let Some(ref v) = req.filter_popularity {
        req.filter_popularity = Some(parse_filter_popularity(v)?);
    }
    if let Some(ref v) = req.filter_gender {
        req.filter_gender = Some(parse_filter_gender(v)?);
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

    if req.show_last_name.is_some()
        || req.last_name.is_some()
        || req.best_pick_mode.is_some()
        || req.filter_countries.is_some()
        || req.filter_popularity.is_some()
        || req.filter_gender.is_some()
    {
        sqlx::query(
            r#"
            UPDATE user_settings
            SET
              show_last_name = COALESCE(?2, show_last_name),
              last_name = COALESCE(?3, last_name),
              best_pick_mode = COALESCE(?4, best_pick_mode),
              filter_countries = COALESCE(?5, filter_countries),
              filter_popularity = COALESCE(?6, filter_popularity),
              filter_gender = COALESCE(?7, filter_gender)
            WHERE user_id = ?1
            "#,
        )
        .bind(user_id)
        .bind(req.show_last_name.map(|v| if v { 1i64 } else { 0i64 }))
        .bind(req.last_name.as_deref())
        .bind(req.best_pick_mode.map(|v| if v { 1i64 } else { 0i64 }))
        .bind(req.filter_countries.as_deref())
        .bind(req.filter_popularity.as_deref())
        .bind(req.filter_gender.as_deref())
        .execute(&state.db)
        .await
        .context("update settings")?;
    }

    get_settings_impl(state, jar).await
}
