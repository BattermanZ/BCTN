pub(crate) mod admin;
pub(crate) mod auth;
pub(crate) mod card;
pub(crate) mod explore;
pub(crate) mod feed;
pub(crate) mod settings;
pub(crate) mod swipes;
pub(crate) mod users;
pub(crate) mod views;

#[cfg(test)]
mod tests;

use crate::state::AppState;
use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};

fn normalize_host(host: &str) -> String {
    let host = host.split(',').next().unwrap_or(host).trim().to_lowercase();
    if host.starts_with('[') {
        return host;
    }
    host.split(':').next().unwrap_or(&host).to_string()
}

async fn host_guard(State(state): State<AppState>, req: Request<Body>, next: Next) -> Response {
    let Some(allowed) = state.public_host.as_deref() else {
        return next.run(req).await;
    };

    let headers = req.headers();
    let proxy_secret_ok = state.proxy_header_secret.as_deref().is_none_or(|expected| {
        headers
            .get("x-bctn-proxy-secret")
            .and_then(|v| v.to_str().ok())
            == Some(expected)
    });
    let host_header = if state.trust_proxy_headers && proxy_secret_ok {
        headers
            .get("x-forwarded-host")
            .or_else(|| headers.get("host"))
    } else {
        headers.get("host")
    };
    let host = host_header
        .and_then(|v| v.to_str().ok())
        .map(normalize_host);

    let Some(host) = host else {
        return StatusCode::MISDIRECTED_REQUEST.into_response();
    };

    if host != allowed {
        return StatusCode::MISDIRECTED_REQUEST.into_response();
    }

    next.run(req).await
}

pub(crate) fn router(state: AppState) -> Router {
    let guard_state = state.clone();
    Router::new()
        .route("/api/health", get(|| async { "ok" }))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/auth/change-password", post(auth::change_password))
        .route("/api/me", get(auth::me))
        .route("/api/users", get(users::list))
        .route("/api/feed/next", get(feed::next))
        .route("/api/feed/batch", get(feed::batch))
        .route("/api/swipes", post(swipes::create))
        .route("/api/swipes/update", post(swipes::update))
        .route("/api/swipes/undo", post(swipes::undo))
        .route("/api/matches", get(views::matches))
        .route("/api/likes", get(views::likes))
        .route("/api/superlikes", get(views::superlikes))
        .route("/api/explore/search", get(explore::search))
        .route("/api/explore/top", get(explore::top))
        .route("/api/card/:card_key", get(card::get))
        .route(
            "/api/settings",
            get(settings::get_settings).patch(settings::patch_settings),
        )
        .route(
            "/api/admin/users",
            get(admin::list_users).post(admin::create_user),
        )
        .route(
            "/api/admin/users/:id",
            axum::routing::patch(admin::update_user).delete(admin::delete_user),
        )
        .with_state(state)
        .layer(middleware::from_fn_with_state(guard_state, host_guard))
}
