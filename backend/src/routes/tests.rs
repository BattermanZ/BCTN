use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::Response;
use serde_json::Value;
use time::OffsetDateTime;
use tower::ServiceExt;

use crate::bootstrap::bootstrap_admin;
use crate::routes::router;
use crate::state::AppState;

async fn create_min_names_db(path: &std::path::Path) {
    let opts = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();

    sqlx::query("CREATE TABLE import_run (id INTEGER PRIMARY KEY, window_start_year INTEGER NOT NULL, window_end_year INTEGER NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE name_observation (run_id INTEGER NOT NULL, country TEXT NOT NULL, count INTEGER NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE variant_group (variant_group_key TEXT PRIMARY KEY, canonical_concept_key TEXT NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE variant_group_country_window_total (variant_group_key TEXT NOT NULL, country TEXT NOT NULL, male INTEGER NOT NULL, female INTEGER NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("CREATE TABLE name_string (id INTEGER PRIMARY KEY, display TEXT NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE name_concept (id INTEGER PRIMARY KEY, concept_key TEXT NOT NULL UNIQUE, preferred_name_string_id INTEGER NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE concept_country_window_total (concept_id INTEGER NOT NULL, country TEXT NOT NULL, male INTEGER NOT NULL, female INTEGER NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO import_run (id, window_start_year, window_end_year) VALUES (1, 2005, 2024)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO name_observation (run_id, country, count) VALUES (1, 'US', 1), (1, 'FR', 1), (1, 'CL', 1)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO name_string (id, display) VALUES (1, 'Kyara')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO name_concept (id, concept_key, preferred_name_string_id) VALUES (1, 'kyara', 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO concept_country_window_total (concept_id, country, male, female) VALUES (1, 'US', 10, 0), (1, 'FR', 10, 0), (1, 'CL', 10, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
}

fn cookie_from_set_cookie(set_cookie: &str) -> String {
    set_cookie
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

async fn login(app: &axum::Router, username: &str, password: &str) -> String {
    let login_body = format!(r#"{{"username":"{username}","password":"{password}"}}"#);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(login_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let set_cookie = resp
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let cookie = cookie_from_set_cookie(&set_cookie);
    assert!(cookie.starts_with("bctn_session="));
    cookie
}

async fn setup_app(public_host: Option<&str>) -> (tempfile::TempDir, AppState, axum::Router) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("app.sqlite");
    let names_db_path = dir.path().join("names.sqlite");
    create_min_names_db(&names_db_path).await;

    let state = AppState::new(
        &db_path,
        &names_db_path,
        false,
        public_host.map(|h| h.to_string()),
    )
    .await
    .unwrap();
    state.migrate().await.unwrap();
    bootstrap_admin(&state.db, "admin", "password123")
        .await
        .unwrap();

    let app = router(state.clone());
    (dir, state, app)
}

async fn json(resp: Response) -> Value {
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn create_user(
    app: &axum::Router,
    admin_cookie: &str,
    username: &str,
    password: &str,
) -> i64 {
    let body = format!(r#"{{"username":"{username}","password":"{password}","is_admin":false}}"#);
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/users")
                .header("cookie", admin_cookie)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        resp.status() == StatusCode::OK || resp.status() == StatusCode::CREATED,
        "unexpected status: {}",
        resp.status()
    );
    json(resp).await["id"].as_i64().unwrap()
}

async fn current_user_id(app: &axum::Router, cookie: &str) -> i64 {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/me")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    json(resp).await["id"].as_i64().unwrap()
}

#[tokio::test]
async fn bootstrap_then_login_then_me_and_admin_list_users() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("app.sqlite");
    let names_db_path = dir.path().join("names.sqlite");
    create_min_names_db(&names_db_path).await;

    let state = AppState::new(&db_path, &names_db_path, false, None)
        .await
        .unwrap();
    state.migrate().await.unwrap();

    bootstrap_admin(&state.db, "admin", "password123")
        .await
        .unwrap();

    let app = router(state);

    let cookie = login(&app, "admin", "password123").await;

    let me_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/me")
                .header("cookie", cookie.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me_resp.status(), StatusCode::OK);

    let admin_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/admin/users")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(admin_resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(admin_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert!(v.is_array());
    assert_eq!(v.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn update_swipe_reconciles_matches() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("app.sqlite");
    let names_db_path = dir.path().join("names.sqlite");
    create_min_names_db(&names_db_path).await;

    let state = AppState::new(&db_path, &names_db_path, false, None)
        .await
        .unwrap();
    state.migrate().await.unwrap();

    bootstrap_admin(&state.db, "admin", "password123")
        .await
        .unwrap();

    let app = router(state);

    let admin_cookie = login(&app, "admin", "password123").await;
    let create_user_body = r#"{"username":"partner","password":"password123","is_admin":false}"#;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/users")
                .header("cookie", admin_cookie.clone())
                .header("content-type", "application/json")
                .body(Body::from(create_user_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        resp.status() == StatusCode::OK || resp.status() == StatusCode::CREATED,
        "unexpected status: {}",
        resp.status()
    );

    let partner_cookie = login(&app, "partner", "password123").await;
    // Admin-created users must change password before they can use the app.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/change-password")
                .header("cookie", partner_cookie.clone())
                .header("content-type", "application/json")
                .body(Body::from(r#"{"new_password":"password123"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Partner likes first.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/swipes")
                .header("cookie", partner_cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"card_key":"kyara","card_kind":"concept","action":"like"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Admin likes second -> match.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/swipes")
                .header("cookie", admin_cookie.clone())
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"card_key":"kyara","card_kind":"concept","action":"like"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/matches")
                .header("cookie", admin_cookie.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert!(
        v.as_array()
            .unwrap()
            .iter()
            .any(|x| x["card_key"] == "kyara")
    );

    // Update our swipe to nope -> match disappears.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/swipes/update")
                .header("cookie", admin_cookie.clone())
                .header("content-type", "application/json")
                .body(Body::from(r#"{"card_key":"kyara","action":"nope"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/matches")
                .header("cookie", admin_cookie.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert!(
        !v.as_array()
            .unwrap()
            .iter()
            .any(|x| x["card_key"] == "kyara")
    );

    // Toggle back to superlike -> match is restored.
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/swipes/update")
                .header("cookie", admin_cookie.clone())
                .header("content-type", "application/json")
                .body(Body::from(r#"{"card_key":"kyara","action":"superlike"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/matches")
                .header("cookie", admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let v: Value = serde_json::from_slice(&body).unwrap();
    assert!(
        v.as_array()
            .unwrap()
            .iter()
            .any(|x| x["card_key"] == "kyara")
    );
}

#[tokio::test]
async fn auth_login_invalid_credentials_returns_401() {
    let (_dir, _state, app) = setup_app(None).await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"username":"admin","password":"wrong-password"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn auth_login_rate_limit_returns_429() {
    let (_dir, _state, app) = setup_app(None).await;

    for _ in 0..20 {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header("x-real-ip", "10.0.0.1")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"username":"admin","password":"wrong-password"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("x-real-ip", "10.0.0.1")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"username":"admin","password":"wrong-password"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn auth_me_without_session_returns_401() {
    let (_dir, _state, app) = setup_app(None).await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn auth_logout_rejects_cross_origin_returns_403() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/logout")
                .header("cookie", admin_cookie)
                .header("host", "good.local")
                .header("origin", "https://evil.local")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn auth_logout_success_returns_204_and_clears_cookie() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/logout")
                .header("cookie", admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let set_cookie = resp
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(set_cookie.contains("bctn_session="));
}

#[tokio::test]
async fn auth_change_password_short_password_returns_400() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let partner_id = create_user(&app, &admin_cookie, "partner", "password123").await;
    assert!(partner_id > 0);
    let partner_cookie = login(&app, "partner", "password123").await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/change-password")
                .header("cookie", partner_cookie)
                .header("content-type", "application/json")
                .body(Body::from(r#"{"new_password":"short"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn auth_change_password_cross_origin_returns_403() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let partner_id = create_user(&app, &admin_cookie, "partner", "password123").await;
    assert!(partner_id > 0);
    let partner_cookie = login(&app, "partner", "password123").await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/change-password")
                .header("cookie", partner_cookie)
                .header("host", "good.local")
                .header("origin", "https://evil.local")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"new_password":"new-password-123"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn auth_change_password_success_clears_must_change_password() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let partner_id = create_user(&app, &admin_cookie, "partner", "password123").await;
    assert!(partner_id > 0);
    let partner_cookie = login(&app, "partner", "password123").await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/change-password")
                .header("cookie", partner_cookie.clone())
                .header("content-type", "application/json")
                .body(Body::from(r#"{"new_password":"new-password-123"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let me = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/me")
                .header("cookie", partner_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me.status(), StatusCode::OK);
    let v = json(me).await;
    assert_eq!(v["must_change_password"], Value::Bool(false));
}

#[tokio::test]
async fn host_guard_public_host_mismatch_returns_421() {
    let (_dir, _state, app) = setup_app(Some("good.local")).await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/health")
                .header("host", "evil.local")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::MISDIRECTED_REQUEST);
}

#[tokio::test]
async fn host_guard_public_host_match_allows_request() {
    let (_dir, _state, app) = setup_app(Some("good.local")).await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/health")
                .header("host", "good.local")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn host_guard_public_host_missing_host_returns_421() {
    let (_dir, _state, app) = setup_app(Some("good.local")).await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::MISDIRECTED_REQUEST);
}

#[tokio::test]
async fn host_guard_uses_x_forwarded_host_when_present() {
    let (_dir, _state, app) = setup_app(Some("good.local")).await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/health")
                .header("host", "evil.local")
                .header("x-forwarded-host", "good.local")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn session_near_expiry_is_renewed_and_sets_cookie() {
    let (_dir, state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let token = admin_cookie
        .strip_prefix("bctn_session=")
        .unwrap_or_default()
        .to_string();

    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(token.as_bytes());
    let token_hash = hex::encode(hasher.finalize());
    let now = OffsetDateTime::now_utc().unix_timestamp();

    sqlx::query("UPDATE app_session SET expires_at = ?1 WHERE session_token_hash = ?2")
        .bind(now + 60)
        .bind(token_hash)
        .execute(&state.db)
        .await
        .unwrap();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/me")
                .header("cookie", admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let set_cookie = resp.headers().get("set-cookie").unwrap().to_str().unwrap();
    assert!(set_cookie.contains("bctn_session="));
}

#[tokio::test]
async fn admin_create_user_non_admin_returns_403() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let partner_id = create_user(&app, &admin_cookie, "partner", "password123").await;
    assert!(partner_id > 0);
    let partner_cookie = login(&app, "partner", "password123").await;
    let changed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/change-password")
                .header("cookie", partner_cookie.clone())
                .header("content-type", "application/json")
                .body(Body::from(r#"{"new_password":"new-password-123"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(changed.status(), StatusCode::NO_CONTENT);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/users")
                .header("cookie", partner_cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"username":"third","password":"password123","is_admin":false}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_update_user_cannot_remove_last_admin_returns_400() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let admin_id = current_user_id(&app, &admin_cookie).await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(&format!("/api/admin/users/{admin_id}"))
                .header("cookie", admin_cookie)
                .header("content-type", "application/json")
                .body(Body::from(r#"{"is_admin":false}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn admin_delete_user_cannot_delete_last_active_user_returns_400() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let admin_id = current_user_id(&app, &admin_cookie).await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(&format!("/api/admin/users/{admin_id}"))
                .header("cookie", admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn admin_delete_user_cannot_delete_last_admin_returns_400() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let admin_id = current_user_id(&app, &admin_cookie).await;
    let partner_id = create_user(&app, &admin_cookie, "partner", "password123").await;
    assert!(partner_id > 0);

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(&format!("/api/admin/users/{admin_id}"))
                .header("cookie", admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn admin_password_reset_invalidates_existing_sessions() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let partner_id = create_user(&app, &admin_cookie, "partner", "password123").await;
    let partner_cookie = login(&app, "partner", "password123").await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(&format!("/api/admin/users/{partner_id}"))
                .header("cookie", admin_cookie)
                .header("content-type", "application/json")
                .body(Body::from(r#"{"password":"reset-password-123"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let me = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/me")
                .header("cookie", partner_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn users_list_requires_auth() {
    let (_dir, _state, app) = setup_app(None).await;
    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/users")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let admin_cookie = login(&app, "admin", "password123").await;
    let authorized = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/users")
                .header("cookie", admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(authorized.status(), StatusCode::OK);
    let v = json(authorized).await;
    assert!(v.is_array());
}
