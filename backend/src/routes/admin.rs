use anyhow::{Context, Result, bail};
use argon2::{Argon2, PasswordHasher};
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, debug_handler};
use axum_extra::extract::CookieJar;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use time::OffsetDateTime;

use crate::routes::auth::require_admin;
use crate::state::AppState;

fn now_ts() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

fn hash_password(password: &str) -> Result<String> {
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

#[derive(Debug, Serialize)]
pub(crate) struct UserRow {
    id: i64,
    username: String,
    is_admin: bool,
    must_change_password: bool,
    created_at: i64,
    updated_at: i64,
    deleted_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateUserRequest {
    username: String,
    password: String,
    is_admin: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateUserRequest {
    username: Option<String>,
    password: Option<String>,
    is_admin: Option<bool>,
    undelete: Option<bool>,
}

#[debug_handler]
pub(crate) async fn list_users(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    let Ok((jar, _admin_user_id)) = require_admin(&state, jar).await else {
        return StatusCode::FORBIDDEN.into_response();
    };

    match list_users_impl(&state).await {
        Ok(users) => (jar, Json(users)).into_response(),
        Err(err) => {
            tracing::error!(error=%err, "list_users_failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn list_users_impl(state: &AppState) -> Result<Vec<UserRow>> {
    let rows = sqlx::query(
        "SELECT id, username, is_admin, must_change_password, created_at, updated_at, deleted_at FROM app_user ORDER BY id",
    )
    .fetch_all(&state.db)
    .await
    .context("list users")?;

    let users = rows
        .into_iter()
        .map(|r| UserRow {
            id: r.try_get("id").unwrap_or_default(),
            username: r.try_get("username").unwrap_or_default(),
            is_admin: r.try_get::<i64, _>("is_admin").unwrap_or(0) != 0,
            must_change_password: r.try_get::<i64, _>("must_change_password").unwrap_or(0) != 0,
            created_at: r.try_get("created_at").unwrap_or_default(),
            updated_at: r.try_get("updated_at").unwrap_or_default(),
            deleted_at: r.try_get::<Option<i64>, _>("deleted_at").unwrap_or(None),
        })
        .collect();
    Ok(users)
}

#[debug_handler]
pub(crate) async fn create_user(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Json(req): Json<CreateUserRequest>,
) -> impl IntoResponse {
    if let Err(err) = crate::routes::auth::ensure_same_origin(&state, &headers) {
        tracing::warn!(error=%err, "csrf_blocked");
        return StatusCode::FORBIDDEN.into_response();
    }
    let Ok((jar, _admin_user_id)) = require_admin(&state, jar).await else {
        return StatusCode::FORBIDDEN.into_response();
    };

    match create_user_impl(&state, req).await {
        Ok(user) => (jar, (StatusCode::CREATED, Json(user))).into_response(),
        Err(err) => {
            tracing::warn!(error=%err, "create_user_failed");
            StatusCode::BAD_REQUEST.into_response()
        }
    }
}

async fn create_user_impl(state: &AppState, req: CreateUserRequest) -> Result<UserRow> {
    if req.username.trim().is_empty() {
        bail!("username required");
    }
    if req.password.len() < 8 {
        bail!("password too short");
    }

    let total_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM app_user")
        .fetch_one(&state.db)
        .await
        .context("count users")?;
    if total_users >= 2 {
        bail!("max 2 users");
    }

    let now = now_ts();
    let password_hash = hash_password(&req.password)?;
    let is_admin = if req.is_admin { 1i64 } else { 0i64 };

    let mut tx = state.db.begin().await?;
    let user_id = sqlx::query(
        "INSERT INTO app_user (username, password_hash, is_admin, must_change_password, created_at, updated_at) VALUES (?1, ?2, ?3, 1, ?4, ?4)",
    )
    .bind(req.username.trim())
    .bind(password_hash)
    .bind(is_admin)
    .bind(now)
    .execute(&mut *tx)
    .await
    .context("insert user")?
    .last_insert_rowid();

    sqlx::query("INSERT OR IGNORE INTO user_settings (user_id) VALUES (?1)")
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .ok();

    tx.commit().await?;

    Ok(UserRow {
        id: user_id,
        username: req.username.trim().to_string(),
        is_admin: req.is_admin,
        must_change_password: true,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    })
}

#[debug_handler]
pub(crate) async fn update_user(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(req): Json<UpdateUserRequest>,
) -> impl IntoResponse {
    if let Err(err) = crate::routes::auth::ensure_same_origin(&state, &headers) {
        tracing::warn!(error=%err, "csrf_blocked");
        return StatusCode::FORBIDDEN.into_response();
    }
    let Ok((jar, _admin_user_id)) = require_admin(&state, jar).await else {
        return StatusCode::FORBIDDEN.into_response();
    };

    match update_user_impl(&state, id, req).await {
        Ok(user) => (jar, Json(user)).into_response(),
        Err(err) => {
            tracing::warn!(error=%err, "update_user_failed");
            StatusCode::BAD_REQUEST.into_response()
        }
    }
}

async fn update_user_impl(state: &AppState, id: i64, req: UpdateUserRequest) -> Result<UserRow> {
    let row = sqlx::query(
        "SELECT id, username, is_admin, must_change_password, created_at, updated_at, deleted_at FROM app_user WHERE id = ?1",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await
    .context("load user")?;

    let mut username: String = row.try_get("username")?;
    let mut is_admin: i64 = row.try_get("is_admin")?;
    let mut must_change_password: i64 = row.try_get("must_change_password")?;
    let created_at: i64 = row.try_get("created_at")?;
    let mut deleted_at: Option<i64> = row
        .try_get::<Option<i64>, _>("deleted_at")?
        .filter(|v| *v != 0);

    if let Some(u) = req.username {
        if u.trim().is_empty() {
            bail!("username required");
        }
        username = u.trim().to_string();
    }
    if let Some(a) = req.is_admin {
        if !a && is_admin != 0 {
            let active_admins: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM app_user WHERE deleted_at IS NULL AND is_admin != 0",
            )
            .fetch_one(&state.db)
            .await
            .context("count admins")?;
            if active_admins <= 1 {
                bail!("cannot remove last admin");
            }
        }
        is_admin = if a { 1 } else { 0 };
    }
    if req.undelete.unwrap_or(false) {
        deleted_at = None;
    }

    let now = now_ts();
    let password_hash = if let Some(pw) = req.password {
        if pw.len() < 8 {
            bail!("password too short");
        }
        Some(hash_password(&pw)?)
    } else {
        None
    };

    let mut tx = state.db.begin().await?;
    if let Some(ph) = password_hash {
        must_change_password = 1;
        sqlx::query(
            "UPDATE app_user SET username=?1, password_hash=?2, is_admin=?3, must_change_password=?4, updated_at=?5, deleted_at=?6 WHERE id=?7",
        )
        .bind(&username)
        .bind(ph)
        .bind(is_admin)
        .bind(must_change_password)
        .bind(now)
        .bind(deleted_at)
        .bind(id)
        .execute(&mut *tx)
        .await?;

        sqlx::query("DELETE FROM app_session WHERE user_id = ?1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .ok();
    } else {
        sqlx::query(
            "UPDATE app_user SET username=?1, is_admin=?2, updated_at=?3, deleted_at=?4 WHERE id=?5",
        )
        .bind(&username)
        .bind(is_admin)
        .bind(now)
        .bind(deleted_at)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    Ok(UserRow {
        id,
        username,
        is_admin: is_admin != 0,
        must_change_password: must_change_password != 0,
        created_at,
        updated_at: now,
        deleted_at,
    })
}

#[debug_handler]
pub(crate) async fn delete_user(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if let Err(err) = crate::routes::auth::ensure_same_origin(&state, &headers) {
        tracing::warn!(error=%err, "csrf_blocked");
        return StatusCode::FORBIDDEN.into_response();
    }
    let Ok((jar, _admin_user_id)) = require_admin(&state, jar).await else {
        return StatusCode::FORBIDDEN.into_response();
    };

    match delete_user_impl(&state, id).await {
        Ok(()) => (jar, StatusCode::NO_CONTENT).into_response(),
        Err(err) => {
            tracing::warn!(error=%err, "delete_user_failed");
            StatusCode::BAD_REQUEST.into_response()
        }
    }
}

async fn delete_user_impl(state: &AppState, id: i64) -> Result<()> {
    let active_users: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM app_user WHERE deleted_at IS NULL")
            .fetch_one(&state.db)
            .await?;
    if active_users <= 1 {
        bail!("cannot delete last active user");
    }

    let deleting_is_admin: i64 =
        sqlx::query_scalar("SELECT is_admin FROM app_user WHERE id = ?1 AND deleted_at IS NULL")
            .bind(id)
            .fetch_one(&state.db)
            .await
            .context("load deleting user is_admin")?;
    if deleting_is_admin != 0 {
        let active_admins: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM app_user WHERE deleted_at IS NULL AND is_admin != 0",
        )
        .fetch_one(&state.db)
        .await
        .context("count admins")?;
        if active_admins <= 1 {
            bail!("cannot delete last admin");
        }
    }

    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM app_user WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .context("delete user")?;

    rebuild_matches(&mut tx).await?;
    tx.commit().await?;
    Ok(())
}

async fn rebuild_matches(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> Result<()> {
    sqlx::query("DELETE FROM name_match")
        .execute(&mut **tx)
        .await
        .context("clear matches")?;

    sqlx::query(
        r#"
        INSERT OR REPLACE INTO name_match (card_key, created_at)
        SELECT card_key, MAX(created_at) AS created_at
        FROM swipe
        WHERE action IN ('like', 'superlike')
        GROUP BY card_key
        HAVING COUNT(DISTINCT user_id) >= 2
        "#,
    )
    .execute(&mut **tx)
    .await
    .context("rebuild matches")?;

    Ok(())
}
