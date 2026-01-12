pub(crate) mod admin;
pub(crate) mod auth;
pub(crate) mod settings;

use axum::Router;
use axum::routing::{get, post};

use crate::state::AppState;

pub(crate) fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(|| async { "ok" }))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/me", get(auth::me))
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
}
