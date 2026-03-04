use anyhow::{Context, Result};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::extract::ConnectInfo;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, debug_handler};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::net::SocketAddr;
use time::OffsetDateTime;

use crate::state::AppState;

const SESSION_COOKIE: &str = "bctn_session";
const SESSION_DAYS: i64 = 90;
const SESSION_RENEW_THRESHOLD_DAYS: i64 = 30;
const CSRF_ORIGIN_HEADER: &str = "origin";

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
    must_change_password: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChangePasswordRequest {
    new_password: String,
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

fn normalize_host(host: &str) -> String {
    let host = host.split(',').next().unwrap_or(host).trim().to_lowercase();

    // IPv6 hosts are bracketed in URLs / Host headers: `[::1]:443`.
    // Normalize by stripping any port while preserving the bracketed address.
    if let Some(rest) = host.strip_prefix('[')
        && let Some(end) = rest.find(']')
    {
        return format!("[{}]", &rest[..end]);
    }

    host.split(':').next().unwrap_or(&host).to_string()
}

fn request_host(headers: &HeaderMap, trust_proxy_headers: bool) -> Option<String> {
    let host_header = if trust_proxy_headers {
        headers
            .get("x-forwarded-host")
            .or_else(|| headers.get("host"))
    } else {
        headers.get("host")
    };
    host_header
        .and_then(|v| v.to_str().ok())
        .map(normalize_host)
}

fn login_limiter_key(username: &str, connect_info: Option<ConnectInfo<SocketAddr>>) -> String {
    let source = connect_info
        .map(|c| c.0.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    format!("{source}:{}", username.trim().to_lowercase())
}

fn clear_session_cookie(jar: CookieJar, cookie_secure: bool) -> CookieJar {
    jar.remove(
        Cookie::build((SESSION_COOKIE, ""))
            .http_only(true)
            .secure(cookie_secure)
            .same_site(SameSite::Lax)
            .path("/")
            .build(),
    )
}

pub(crate) fn status_code_for_request_error(err: &anyhow::Error) -> StatusCode {
    match err.to_string().as_str() {
        "missing session" | "invalid session" => StatusCode::UNAUTHORIZED,
        "invalid origin"
        | "invalid host"
        | "cross-origin request blocked"
        | "not admin"
        | "password change required" => StatusCode::FORBIDDEN,
        _ => StatusCode::BAD_REQUEST,
    }
}

pub(crate) fn ensure_same_origin(state: &AppState, headers: &HeaderMap) -> Result<()> {
    // Simple CSRF mitigation: if the browser sends an Origin header, require it matches the Host.
    // Reverse proxies typically preserve Origin, and this blocks cross-site POST/PATCH/DELETE.
    let origin = headers
        .get(CSRF_ORIGIN_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string());
    let Some(origin) = origin else {
        return Ok(());
    };

    let origin_host = origin
        .split("://")
        .nth(1)
        .unwrap_or(&origin)
        .split('/')
        .next()
        .unwrap_or("");
    let origin_host = normalize_host(origin_host);

    let host = request_host(headers, state.trust_proxy_headers).unwrap_or_default();
    if origin_host.is_empty() || host.is_empty() {
        anyhow::bail!("invalid origin");
    }
    if let Some(allowed) = state.public_host.as_deref() {
        if host != allowed {
            anyhow::bail!("invalid host");
        }
        if origin_host != allowed {
            anyhow::bail!("cross-origin request blocked");
        }
        return Ok(());
    }
    if origin_host != host {
        anyhow::bail!("cross-origin request blocked");
    }
    Ok(())
}

pub(crate) async fn require_session_user(
    state: &AppState,
    jar: CookieJar,
) -> Result<(CookieJar, i64, bool, bool)> {
    let Some(token) = jar.get(SESSION_COOKIE).map(|c| c.value().to_string()) else {
        anyhow::bail!("missing session");
    };
    let now = now_ts();
    let session_token_hash = token_hash(&token);
    let row = sqlx::query(
        r#"
        SELECT
          u.id AS user_id,
          u.is_admin AS is_admin,
          u.must_change_password AS must_change_password,
          s.expires_at AS expires_at
        FROM app_session s
        JOIN app_user u ON u.id = s.user_id
        WHERE s.session_token_hash = ?1
          AND s.expires_at > ?2
          AND u.deleted_at IS NULL
        "#,
    )
    .bind(&session_token_hash)
    .bind(now)
    .fetch_optional(&state.db)
    .await
    .context("load session")?;

    let Some(row) = row else {
        anyhow::bail!("invalid session");
    };

    let user_id: i64 = row.try_get("user_id")?;
    let is_admin_i64: i64 = row.try_get("is_admin")?;
    let must_change_password_i64: i64 = row.try_get("must_change_password")?;
    let expires_at: i64 = row.try_get("expires_at")?;

    if let Err(err) =
        sqlx::query("UPDATE app_session SET last_seen_at = ?1 WHERE session_token_hash = ?2")
            .bind(now)
            .bind(&session_token_hash)
            .execute(&state.db)
            .await
    {
        tracing::warn!(
            error=%err,
            user_msg="Could not update session activity timestamp. The user may be asked to log in again later.",
            "session_last_seen_update_failed"
        );
    }

    let remaining = expires_at - now;
    let renew_threshold = SESSION_RENEW_THRESHOLD_DAYS * 24 * 60 * 60;
    if remaining <= renew_threshold {
        let new_expires = now + SESSION_DAYS * 24 * 60 * 60;

        if let Err(err) =
            sqlx::query("UPDATE app_session SET expires_at = ?1 WHERE session_token_hash = ?2")
                .bind(new_expires)
                .bind(&session_token_hash)
                .execute(&state.db)
                .await
        {
            tracing::warn!(
                error=%err,
                user_msg="Could not extend session expiry. The user may be asked to log in again later.",
                "session_expires_update_failed"
            );
        }

        let cookie = Cookie::build((SESSION_COOKIE, token))
            .http_only(true)
            .secure(state.cookie_secure)
            .same_site(SameSite::Lax)
            .path("/")
            .max_age(time::Duration::days(SESSION_DAYS))
            .expires(
                OffsetDateTime::from_unix_timestamp(new_expires)
                    .unwrap_or_else(|_| OffsetDateTime::now_utc()),
            )
            .build();
        let jar = jar.add(cookie);
        return Ok((
            jar,
            user_id,
            is_admin_i64 != 0,
            must_change_password_i64 != 0,
        ));
    }

    Ok((
        jar,
        user_id,
        is_admin_i64 != 0,
        must_change_password_i64 != 0,
    ))
}

pub(crate) async fn require_session_user_ready(
    state: &AppState,
    jar: CookieJar,
) -> Result<(CookieJar, i64, bool)> {
    let (jar, user_id, is_admin, must_change_password) = require_session_user(state, jar).await?;
    if must_change_password {
        anyhow::bail!("password change required");
    }
    Ok((jar, user_id, is_admin))
}

#[debug_handler]
pub(crate) async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    let now = now_ts();
    let key = login_limiter_key(&req.username, connect_info);
    if state.login_limiter.is_limited(&key, now).await {
        tracing::warn!(
            login_key=%key,
            user_msg="Too many login attempts from this client. Please wait and try again.",
            "login_rate_limited"
        );
        return (StatusCode::TOO_MANY_REQUESTS, "too many attempts").into_response();
    }

    match login_impl(&state, jar, req).await {
        Ok(resp) => {
            state.login_limiter.clear(&key).await;
            resp.into_response()
        }
        Err(err) => {
            state.login_limiter.record_failure(&key, now).await;
            tracing::warn!(
                error=%err,
                user_msg="Login failed. Check username and password and try again.",
                "login_failed"
            );
            (StatusCode::UNAUTHORIZED, "invalid credentials").into_response()
        }
    }
}

async fn login_impl(
    state: &AppState,
    jar: CookieJar,
    req: LoginRequest,
) -> Result<impl IntoResponse> {
    static DUMMY_HASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();

    let row = sqlx::query(
        r#"
        SELECT id, username, password_hash, is_admin, must_change_password
        FROM app_user
        WHERE username = ?1 AND deleted_at IS NULL
        "#,
    )
    .bind(req.username)
    .fetch_optional(&state.db)
    .await
    .context("load user")?;

    let Some(row) = row else {
        let dummy = DUMMY_HASH.get_or_init(|| {
            hash_password("dummy-password").expect("dummy hash generation should succeed")
        });
        if let Ok(parsed) = PasswordHash::new(dummy) {
            let _ = Argon2::default().verify_password(req.password.as_bytes(), &parsed);
        }
        anyhow::bail!("invalid credentials");
    };

    let user_id: i64 = row.try_get("id")?;
    let username: String = row.try_get("username")?;
    let password_hash: String = row.try_get("password_hash")?;
    let is_admin_i64: i64 = row.try_get("is_admin")?;
    let must_change_password_i64: i64 = row.try_get("must_change_password")?;

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
        .max_age(time::Duration::days(SESSION_DAYS))
        .expires(
            OffsetDateTime::from_unix_timestamp(expires)
                .unwrap_or_else(|_| OffsetDateTime::now_utc()),
        )
        .build();

    let jar = jar.add(cookie);
    let me = MeResponse {
        id: user_id,
        username,
        is_admin: is_admin_i64 != 0,
        must_change_password: must_change_password_i64 != 0,
    };
    Ok((jar, Json(me)))
}

#[debug_handler]
pub(crate) async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
) -> impl IntoResponse {
    if ensure_same_origin(&state, &headers).is_err() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(token) = jar.get(SESSION_COOKIE).map(|c| c.value().to_string()) else {
        return (jar, StatusCode::NO_CONTENT).into_response();
    };
    let token_hash = token_hash(&token);
    sqlx::query("DELETE FROM app_session WHERE session_token_hash = ?1")
        .bind(token_hash)
        .execute(&state.db)
        .await
        .ok();

    let jar = clear_session_cookie(jar, state.cookie_secure);
    (jar, StatusCode::NO_CONTENT).into_response()
}

#[debug_handler]
pub(crate) async fn me(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    match me_impl(&state, jar).await {
        Ok((jar, me)) => (jar, Json(me)).into_response(),
        Err(_) => StatusCode::UNAUTHORIZED.into_response(),
    }
}

async fn me_impl(state: &AppState, jar: CookieJar) -> Result<(CookieJar, MeResponse)> {
    let (jar, user_id, _, _) = require_session_user(state, jar).await?;
    let row = sqlx::query("SELECT id, username, is_admin, must_change_password FROM app_user WHERE id = ?1 AND deleted_at IS NULL")
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    Ok((
        jar,
        MeResponse {
            id: row.try_get("id")?,
            username: row.try_get("username")?,
            is_admin: row.try_get::<i64, _>("is_admin")? != 0,
            must_change_password: row.try_get::<i64, _>("must_change_password")? != 0,
        },
    ))
}

pub(crate) async fn require_admin(state: &AppState, jar: CookieJar) -> Result<(CookieJar, i64)> {
    let (jar, user_id, is_admin) = require_session_user_ready(state, jar).await?;
    if !is_admin {
        anyhow::bail!("not admin");
    }
    Ok((jar, user_id))
}

fn hash_password(password: &str) -> Result<String> {
    use argon2::PasswordHasher;
    use argon2::password_hash::SaltString;
    let salt = {
        let mut bytes = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        SaltString::encode_b64(&bytes).map_err(|e| anyhow::anyhow!("salt: {e}"))?
    };
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("hash password: {e}"))?
        .to_string();
    Ok(hash)
}

#[debug_handler]
pub(crate) async fn change_password(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Json(req): Json<ChangePasswordRequest>,
) -> impl IntoResponse {
    if let Err(err) = ensure_same_origin(&state, &headers) {
        tracing::warn!(
            error=%err,
            user_msg="Request blocked for security reasons (origin check failed).",
            "csrf_blocked"
        );
        return StatusCode::FORBIDDEN.into_response();
    }
    match change_password_impl(&state, jar, req).await {
        Ok(jar) => (jar, StatusCode::NO_CONTENT).into_response(),
        Err(err) => {
            tracing::warn!(
                error=%err,
                user_msg="Could not change password. Check the requirements and try again.",
                "change_password_failed"
            );
            StatusCode::BAD_REQUEST.into_response()
        }
    }
}

async fn change_password_impl(
    state: &AppState,
    jar: CookieJar,
    req: ChangePasswordRequest,
) -> Result<CookieJar> {
    if req.new_password.len() < 8 {
        anyhow::bail!("password too short");
    }

    let (jar, user_id, _, _) = require_session_user(state, jar).await?;
    let now = now_ts();
    let password_hash = hash_password(&req.new_password)?;

    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE app_user SET password_hash=?1, must_change_password=0, updated_at=?2 WHERE id=?3 AND deleted_at IS NULL")
        .bind(password_hash)
        .bind(now)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .context("update password")?;

    sqlx::query("DELETE FROM app_session WHERE user_id = ?1")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .context("revoke sessions after password change")?;

    tx.commit().await?;
    Ok(clear_session_cookie(jar, state.cookie_secure))
}
