use anyhow::{Context, Result};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, debug_handler};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use time::OffsetDateTime;

use crate::state::AppState;

const SESSION_COOKIE: &str = "bctn_session";
const SESSION_DAYS: i64 = 30;

#[derive(Debug, Deserialize)]
pub(crate) struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct MeResponse {
    id: i64,
    username: String,
    is_admin: bool,
}

fn now_ts() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

fn token_hex_32() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn token_hash(token: &str) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

pub(crate) async fn require_session_user(state: &AppState, jar: &CookieJar) -> Result<(i64, bool)> {
    let Some(token) = jar.get(SESSION_COOKIE).map(|c| c.value().to_string()) else {
        anyhow::bail!("missing session");
    };
    let session_token_hash = token_hash(&token);
    let row = sqlx::query(
        r#"
        SELECT u.id AS user_id, u.is_admin AS is_admin
        FROM app_session s
        JOIN app_user u ON u.id = s.user_id
        WHERE s.session_token_hash = ?1
          AND s.expires_at > ?2
          AND u.deleted_at IS NULL
        "#,
    )
    .bind(session_token_hash)
    .bind(now_ts())
    .fetch_optional(&state.db)
    .await
    .context("load session")?;

    let Some(row) = row else {
        anyhow::bail!("invalid session");
    };

    let user_id: i64 = row.try_get("user_id")?;
    let is_admin_i64: i64 = row.try_get("is_admin")?;

    sqlx::query("UPDATE app_session SET last_seen_at = ?1 WHERE session_token_hash = ?2")
        .bind(now_ts())
        .bind(token_hash(&token))
        .execute(&state.db)
        .await
        .ok();

    Ok((user_id, is_admin_i64 != 0))
}

#[debug_handler]
pub(crate) async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    match login_impl(&state, jar, req).await {
        Ok(resp) => resp.into_response(),
        Err(err) => {
            tracing::warn!(error=%err, "login_failed");
            (StatusCode::UNAUTHORIZED, "invalid credentials").into_response()
        }
    }
}

async fn login_impl(
    state: &AppState,
    jar: CookieJar,
    req: LoginRequest,
) -> Result<impl IntoResponse> {
    let row = sqlx::query(
        r#"
        SELECT id, username, password_hash, is_admin
        FROM app_user
        WHERE username = ?1 AND deleted_at IS NULL
        "#,
    )
    .bind(req.username)
    .fetch_one(&state.db)
    .await
    .context("load user")?;

    let user_id: i64 = row.try_get("id")?;
    let username: String = row.try_get("username")?;
    let password_hash: String = row.try_get("password_hash")?;
    let is_admin_i64: i64 = row.try_get("is_admin")?;

    let parsed_hash = PasswordHash::new(&password_hash)
        .map_err(|e| anyhow::anyhow!("parse password hash: {e}"))?;
    Argon2::default()
        .verify_password(req.password.as_bytes(), &parsed_hash)
        .map_err(|e| anyhow::anyhow!("verify password: {e}"))?;

    let token = token_hex_32();
    let session_token_hash = token_hash(&token);
    let now = now_ts();
    let expires = now + SESSION_DAYS * 24 * 60 * 60;

    sqlx::query(
        "INSERT INTO app_session (user_id, session_token_hash, created_at, expires_at, last_seen_at) VALUES (?1, ?2, ?3, ?4, ?3)",
    )
    .bind(user_id)
    .bind(session_token_hash)
    .bind(now)
    .bind(expires)
    .execute(&state.db)
    .await
    .context("insert session")?;

    let cookie = Cookie::build((SESSION_COOKIE, token))
        .http_only(true)
        .secure(state.cookie_secure)
        .same_site(SameSite::Lax)
        .path("/")
        .build();

    let jar = jar.add(cookie);
    let me = MeResponse {
        id: user_id,
        username,
        is_admin: is_admin_i64 != 0,
    };
    Ok((jar, Json(me)))
}

#[debug_handler]
pub(crate) async fn logout(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    let Some(token) = jar.get(SESSION_COOKIE).map(|c| c.value().to_string()) else {
        return (jar, StatusCode::NO_CONTENT).into_response();
    };
    let token_hash = token_hash(&token);
    sqlx::query("DELETE FROM app_session WHERE session_token_hash = ?1")
        .bind(token_hash)
        .execute(&state.db)
        .await
        .ok();

    let jar = jar.remove(Cookie::build((SESSION_COOKIE, "")).path("/").build());
    (jar, StatusCode::NO_CONTENT).into_response()
}

#[debug_handler]
pub(crate) async fn me(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    match me_impl(&state, &jar).await {
        Ok(me) => Json(me).into_response(),
        Err(_) => StatusCode::UNAUTHORIZED.into_response(),
    }
}

async fn me_impl(state: &AppState, jar: &CookieJar) -> Result<MeResponse> {
    let (user_id, _) = require_session_user(state, jar).await?;
    let row = sqlx::query(
        "SELECT id, username, is_admin FROM app_user WHERE id = ?1 AND deleted_at IS NULL",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    Ok(MeResponse {
        id: row.try_get("id")?,
        username: row.try_get("username")?,
        is_admin: row.try_get::<i64, _>("is_admin")? != 0,
    })
}

pub(crate) async fn require_admin(state: &AppState, jar: &CookieJar) -> Result<i64> {
    let (user_id, is_admin) = require_session_user(state, jar).await?;
    if !is_admin {
        anyhow::bail!("not admin");
    }
    Ok(user_id)
}
