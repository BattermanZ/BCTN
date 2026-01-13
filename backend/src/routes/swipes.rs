use anyhow::{Context, Result, bail};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, debug_handler};
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use time::OffsetDateTime;

use crate::state::AppState;

fn now_ts() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateSwipeRequest {
    card_key: String,
    card_kind: String,
    action: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreateSwipeResponse {
    status: String,
    match_created: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct UndoSwipeResponse {
    status: String,
    undone: Option<UndoneSwipe>,
    match_removed: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct UndoneSwipe {
    card_key: String,
    action: String,
}

fn validate_card_kind(s: &str) -> Result<()> {
    match s {
        "group" | "concept" => Ok(()),
        other => bail!("invalid card_kind {other:?}"),
    }
}

fn validate_action(s: &str) -> Result<()> {
    match s {
        "like" | "nope" | "superlike" => Ok(()),
        other => bail!("invalid action {other:?}"),
    }
}

async fn partner_user_id(state: &AppState, user_id: i64) -> Result<i64> {
    let partner: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM app_user WHERE deleted_at IS NULL AND id != ?1 ORDER BY id LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .context("find partner user")?;
    partner.context("missing partner user")
}

#[debug_handler]
pub(crate) async fn create(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(req): Json<CreateSwipeRequest>,
) -> impl IntoResponse {
    match create_impl(&state, jar, req).await {
        Ok((jar, resp)) => (jar, Json(resp)).into_response(),
        Err(err) => {
            tracing::warn!(error=%err, "create_swipe_failed");
            (StatusCode::BAD_REQUEST, err.to_string()).into_response()
        }
    }
}

async fn create_impl(
    state: &AppState,
    jar: CookieJar,
    req: CreateSwipeRequest,
) -> Result<(CookieJar, CreateSwipeResponse)> {
    let (jar, user_id, _) = super::auth::require_session_user_ready(state, jar).await?;
    validate_card_kind(&req.card_kind)?;
    validate_action(&req.action)?;

    let now = now_ts();
    let insert = sqlx::query(
        "INSERT INTO swipe (user_id, card_key, card_kind, action, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(user_id)
    .bind(&req.card_key)
    .bind(&req.card_kind)
    .bind(&req.action)
    .bind(now)
    .execute(&state.db)
    .await;

    let inserted = match insert {
        Ok(_) => true,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("UNIQUE") {
                return Ok((
                    jar,
                    CreateSwipeResponse {
                        status: "already_swiped".to_string(),
                        match_created: false,
                    },
                ));
            }
            return Err(e).context("insert swipe");
        }
    };
    if !inserted {
        return Ok((
            jar,
            CreateSwipeResponse {
                status: "already_swiped".to_string(),
                match_created: false,
            },
        ));
    }

    let mut match_created = false;
    if req.action == "like" || req.action == "superlike" {
        let partner_id = partner_user_id(state, user_id).await?;
        let partner_like: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM swipe WHERE user_id=?1 AND card_key=?2 AND action IN ('like','superlike') LIMIT 1",
        )
        .bind(partner_id)
        .bind(&req.card_key)
        .fetch_optional(&state.db)
        .await
        .context("check partner like")?;
        if partner_like.is_some() {
            let res = sqlx::query(
                "INSERT OR IGNORE INTO name_match (card_key, created_at) VALUES (?1, ?2)",
            )
            .bind(&req.card_key)
            .bind(now)
            .execute(&state.db)
            .await
            .context("insert match")?;
            if res.rows_affected() > 0 {
                match_created = true;
            }
        }
    }

    Ok((
        jar,
        CreateSwipeResponse {
            status: "ok".to_string(),
            match_created,
        },
    ))
}

#[debug_handler]
pub(crate) async fn undo(State(state): State<AppState>, jar: CookieJar) -> impl IntoResponse {
    match undo_impl(&state, jar).await {
        Ok((jar, resp)) => (jar, Json(resp)).into_response(),
        Err(err) => {
            tracing::warn!(error=%err, "undo_swipe_failed");
            (StatusCode::BAD_REQUEST, err.to_string()).into_response()
        }
    }
}

async fn undo_impl(state: &AppState, jar: CookieJar) -> Result<(CookieJar, UndoSwipeResponse)> {
    let (jar, user_id, _) = super::auth::require_session_user_ready(state, jar).await?;

    let row = sqlx::query(
        "SELECT id, card_key, action FROM swipe WHERE user_id=?1 ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .context("load last swipe")?;

    let Some(row) = row else {
        return Ok((
            jar,
            UndoSwipeResponse {
                status: "nothing_to_undo".to_string(),
                undone: None,
                match_removed: false,
            },
        ));
    };

    let swipe_id: i64 = row.try_get("id")?;
    let card_key: String = row.try_get("card_key")?;
    let action: String = row.try_get("action")?;

    sqlx::query("DELETE FROM swipe WHERE id=?1")
        .bind(swipe_id)
        .execute(&state.db)
        .await?;

    let mut match_removed = false;
    if action == "like" || action == "superlike" {
        // A match exists only when both users like/superlike the same card.
        // Undoing our own like/superlike breaks mutuality, so the match must be removed.
        let res = sqlx::query("DELETE FROM name_match WHERE card_key=?1")
            .bind(&card_key)
            .execute(&state.db)
            .await?;
        if res.rows_affected() > 0 {
            match_removed = true;
        }
    }

    Ok((
        jar,
        UndoSwipeResponse {
            status: "ok".to_string(),
            undone: Some(UndoneSwipe { card_key, action }),
            match_removed,
        },
    ))
}
