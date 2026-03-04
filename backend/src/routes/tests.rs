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
        "CREATE TABLE variant_group_country_window_total (variant_group_key TEXT NOT NULL, country TEXT NOT NULL, total INTEGER NOT NULL, male INTEGER NOT NULL, female INTEGER NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE variant_group_country_top100_any (variant_group_key TEXT NOT NULL, country TEXT NOT NULL, top100_any INTEGER NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE variant_group_member (variant_group_key TEXT NOT NULL, concept_key TEXT NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE name_string (id INTEGER PRIMARY KEY, display TEXT NOT NULL, key_folded TEXT NOT NULL)",
    )
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
        "CREATE TABLE name_concept_member (concept_id INTEGER NOT NULL, name_string_id INTEGER NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE concept_country_window_total (concept_id INTEGER NOT NULL, country TEXT NOT NULL, total INTEGER NOT NULL, male INTEGER NOT NULL, female INTEGER NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE concept_country_top100_any (concept_id INTEGER NOT NULL, country TEXT NOT NULL, top100_any INTEGER NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE card_spelling_total_all (card_kind TEXT NOT NULL, card_key TEXT NOT NULL, name_raw TEXT NOT NULL, total INTEGER NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE card_spelling_subset_top (card_kind TEXT NOT NULL, card_key TEXT NOT NULL, subset_key TEXT NOT NULL, name_raw TEXT NOT NULL, rank INTEGER NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE card_rank_last5 (card_kind TEXT NOT NULL, card_key TEXT NOT NULL, country TEXT NOT NULL, year INTEGER NOT NULL, rank INTEGER NOT NULL, total INTEGER NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("CREATE TABLE country_last5_year (country TEXT NOT NULL, year INTEGER NOT NULL)")
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
    sqlx::query(
        "INSERT INTO name_string (id, display, key_folded) VALUES
        (1, 'Kyara', 'kyara'),
        (2, 'Lina', 'lina'),
        (3, 'Noah', 'noah'),
        (4, 'Kiara', 'kiara')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO name_concept (id, concept_key, preferred_name_string_id) VALUES
        (1, 'kyara', 1),
        (2, 'lina', 2),
        (3, 'noah', 3),
        (4, 'kyara_group', 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO variant_group (variant_group_key, canonical_concept_key) VALUES
        ('kyara_group', 'kyara')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO variant_group_member (variant_group_key, concept_key) VALUES
        ('kyara_group', 'kyara'),
        ('kyara_group', 'kyara_group')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO name_concept_member (concept_id, name_string_id) VALUES
        (1, 1), (1, 4), (2, 2), (3, 3), (4, 1), (4, 4)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO concept_country_window_total (concept_id, country, total, male, female) VALUES
        (1, 'US', 10, 10, 0), (1, 'FR', 10, 10, 0), (1, 'CL', 10, 10, 0),
        (2, 'US', 8, 0, 8), (2, 'FR', 9, 0, 9), (2, 'CL', 7, 0, 7),
        (3, 'US', 12, 12, 0), (3, 'FR', 11, 11, 0), (3, 'CL', 13, 13, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO concept_country_top100_any (concept_id, country, top100_any) VALUES
        (1, 'US', 0), (1, 'FR', 0), (1, 'CL', 0),
        (2, 'US', 0), (2, 'FR', 0), (2, 'CL', 0),
        (3, 'US', 0), (3, 'FR', 0), (3, 'CL', 0),
        (4, 'US', 0), (4, 'FR', 0), (4, 'CL', 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO variant_group_country_window_total (variant_group_key, country, total, male, female) VALUES
        ('kyara_group', 'US', 10, 10, 0),
        ('kyara_group', 'FR', 10, 10, 0),
        ('kyara_group', 'CL', 10, 10, 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO variant_group_country_top100_any (variant_group_key, country, top100_any) VALUES
        ('kyara_group', 'US', 0),
        ('kyara_group', 'FR', 0),
        ('kyara_group', 'CL', 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO card_spelling_total_all (card_kind, card_key, name_raw, total) VALUES
        ('concept', 'kyara', 'Kyara', 10),
        ('concept', 'kyara', 'Kiara', 4),
        ('concept', 'lina', 'Lina', 8),
        ('concept', 'noah', 'Noah', 12),
        ('group', 'kyara_group', 'Kyara', 10),
        ('group', 'kyara_group', 'Kiara', 4)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO card_spelling_subset_top (card_kind, card_key, subset_key, name_raw, rank) VALUES
        ('concept', 'kyara', 'US', 'Kyara', 1),
        ('concept', 'kyara', 'FR', 'Kyara', 1),
        ('concept', 'kyara', 'CL', 'Kyara', 1),
        ('concept', 'kyara', 'US,FR,CL', 'Kyara', 1),
        ('concept', 'kyara', 'US,FR,CL', 'Kiara', 2),
        ('group', 'kyara_group', 'US', 'Kyara', 1),
        ('group', 'kyara_group', 'FR', 'Kyara', 1),
        ('group', 'kyara_group', 'CL', 'Kyara', 1),
        ('group', 'kyara_group', 'US,FR,CL', 'Kyara', 1),
        ('group', 'kyara_group', 'US,FR,CL', 'Kiara', 2)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO card_rank_last5 (card_kind, card_key, country, year, rank, total) VALUES
        ('concept', 'kyara', 'US', 2020, 120, 2),
        ('concept', 'kyara', 'US', 2021, 110, 2),
        ('concept', 'kyara', 'US', 2022, 100, 2),
        ('concept', 'kyara', 'US', 2023, 95, 2),
        ('concept', 'kyara', 'US', 2024, 90, 2),
        ('group', 'kyara_group', 'US', 2020, 120, 2),
        ('group', 'kyara_group', 'US', 2021, 110, 2),
        ('group', 'kyara_group', 'US', 2022, 100, 2),
        ('group', 'kyara_group', 'US', 2023, 95, 2),
        ('group', 'kyara_group', 'US', 2024, 90, 2)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO country_last5_year (country, year) VALUES
        ('US', 2020), ('US', 2021), ('US', 2022), ('US', 2023), ('US', 2024),
        ('FR', 2020), ('FR', 2021), ('FR', 2022), ('FR', 2023), ('FR', 2024),
        ('CL', 2020), ('CL', 2021), ('CL', 2022), ('CL', 2023), ('CL', 2024)",
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

async fn change_password(app: &axum::Router, cookie: &str, new_password: &str) -> StatusCode {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/change-password")
                .header("cookie", cookie)
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    r#"{{"new_password":"{new_password}"}}"#
                )))
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
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
    let partner_cookie = login(&app, "partner", "password123").await;

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
                .header("cookie", partner_cookie.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me.status(), StatusCode::UNAUTHORIZED);

    let new_cookie = login(&app, "partner", "new-password-123").await;
    let me = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/me")
                .header("cookie", new_cookie)
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
async fn host_guard_ignores_x_forwarded_host_when_host_mismatches() {
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
    assert_eq!(resp.status(), StatusCode::MISDIRECTED_REQUEST);
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

#[tokio::test]
async fn feed_next_without_session_returns_401_with_sanitized_error() {
    let (_dir, _state, app) = setup_app(None).await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/feed/next")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&body), "unauthorized");
}

#[tokio::test]
async fn swipes_create_invalid_card_kind_returns_400() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/swipes")
                .header("cookie", admin_cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"card_key":"kyara","card_kind":"invalid","action":"like"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn swipes_create_invalid_action_returns_400() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/swipes")
                .header("cookie", admin_cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"card_key":"kyara","card_kind":"concept","action":"maybe"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn swipes_create_duplicate_returns_already_swiped() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/swipes")
                .header("cookie", admin_cookie.clone())
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"card_key":"kyara","card_kind":"concept","action":"nope"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);

    let second = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/swipes")
                .header("cookie", admin_cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"card_key":"kyara","card_kind":"concept","action":"nope"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::OK);
    let v = json(second).await;
    assert_eq!(v["status"], Value::String("already_swiped".to_string()));
}

#[tokio::test]
async fn swipes_undo_when_empty_returns_nothing_to_undo() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/swipes/undo")
                .header("cookie", admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = json(resp).await;
    assert_eq!(v["status"], Value::String("nothing_to_undo".to_string()));
}

#[tokio::test]
async fn swipes_undo_like_removes_match() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let _partner_id = create_user(&app, &admin_cookie, "partner", "password123").await;
    let partner_cookie = login(&app, "partner", "password123").await;
    assert_eq!(
        change_password(&app, &partner_cookie, "new-password-123").await,
        StatusCode::NO_CONTENT
    );
    let partner_cookie = login(&app, "partner", "new-password-123").await;

    for cookie in [partner_cookie.as_str(), admin_cookie.as_str()] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/swipes")
                    .header("cookie", cookie)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"card_key":"kyara","card_kind":"concept","action":"like"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    let undo = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/swipes/undo")
                .header("cookie", admin_cookie.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(undo.status(), StatusCode::OK);
    let v = json(undo).await;
    assert_eq!(v["status"], Value::String("ok".to_string()));
    assert_eq!(v["match_removed"], Value::Bool(true));

    let matches = app
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
    let v = json(matches).await;
    assert!(
        !v.as_array()
            .unwrap()
            .iter()
            .any(|x| x["card_key"] == "kyara")
    );
}

#[tokio::test]
async fn views_likes_and_superlikes_return_only_expected_actions() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let _partner_id = create_user(&app, &admin_cookie, "partner", "password123").await;
    let partner_cookie = login(&app, "partner", "password123").await;
    assert_eq!(
        change_password(&app, &partner_cookie, "new-password-123").await,
        StatusCode::NO_CONTENT
    );

    let like = app
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
    assert_eq!(like.status(), StatusCode::OK);

    let superlike = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/swipes")
                .header("cookie", admin_cookie.clone())
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"card_key":"noah","card_kind":"concept","action":"superlike"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(superlike.status(), StatusCode::OK);

    let likes = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/likes")
                .header("cookie", admin_cookie.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(likes.status(), StatusCode::OK);
    let likes_v = json(likes).await;
    assert!(
        likes_v
            .as_array()
            .unwrap()
            .iter()
            .all(|x| x["my_action"] == "like")
    );
    assert!(
        likes_v
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["card_key"] == "kyara")
    );

    let superlikes = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/superlikes")
                .header("cookie", admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(superlikes.status(), StatusCode::OK);
    let superlikes_v = json(superlikes).await;
    assert!(
        superlikes_v
            .as_array()
            .unwrap()
            .iter()
            .all(|x| x["my_action"] == "superlike")
    );
    assert!(
        superlikes_v
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["card_key"] == "noah")
    );
}

#[tokio::test]
async fn settings_patch_invalid_values_return_400() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;

    for body in [
        r#"{"filter_countries":"US,ZZ"}"#,
        r#"{"filter_popularity":"rare,invalid"}"#,
        r#"{"filter_gender":"invalid"}"#,
        r#"{"last_name":""}"#,
    ] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/api/settings")
                    .header("cookie", admin_cookie.clone())
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
async fn settings_patch_partial_update_persists() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;

    let patch = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/settings")
                .header("cookie", admin_cookie.clone())
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"show_last_name":true,"last_name":"Stone","filter_countries":"FR,US","filter_popularity":"rare,common","filter_gender":"female"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(patch.status(), StatusCode::OK);
    let patched = json(patch).await;
    assert_eq!(patched["show_last_name"], Value::Bool(true));
    assert_eq!(patched["last_name"], Value::String("Stone".to_string()));
    assert_eq!(
        patched["filter_countries"],
        Value::String("FR,US".to_string())
    );
    assert_eq!(
        patched["filter_popularity"],
        Value::String("rare,common".to_string())
    );
    assert_eq!(
        patched["filter_gender"],
        Value::String("female".to_string())
    );

    let get = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/settings")
                .header("cookie", admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get.status(), StatusCode::OK);
    let persisted = json(get).await;
    assert_eq!(persisted["last_name"], Value::String("Stone".to_string()));
}

#[tokio::test]
async fn explore_search_short_query_returns_empty_200() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/explore/search?q=k")
                .header("cookie", admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = json(resp).await;
    assert_eq!(v, Value::Array(vec![]));
}

#[tokio::test]
async fn feed_next_with_filters_returns_matching_card() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/feed/next?countries=US,FR,CL&pop=rare,common,top&gender=female")
                .header("cookie", admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = json(resp).await;
    assert_eq!(v["card_key"], Value::String("lina".to_string()));
    let per_country = v["per_country"].as_array().unwrap();
    assert_eq!(per_country.len(), 3);
    assert!(
        per_country
            .iter()
            .all(|c| c["gender"] == "female" || c["gender"] == "mostly_female")
    );
}

#[tokio::test]
async fn feed_batch_limit_is_capped() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/feed/batch?limit=999")
                .header("cookie", admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = json(resp).await;
    let cards = v.as_array().unwrap();
    assert!(cards.len() <= 25);
    assert!(!cards.is_empty());
    let mut seen = std::collections::HashSet::<String>::new();
    for card in cards {
        let key = card["card_key"].as_str().unwrap().to_string();
        assert!(seen.insert(key), "duplicate card_key in batch response");
        assert_eq!(card["per_country"].as_array().unwrap().len(), 3);
    }
}

#[tokio::test]
async fn explore_search_pagination_out_of_range_returns_empty() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/explore/search?q=ky&page=99&page_size=50")
                .header("cookie", admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json(resp).await, Value::Array(vec![]));
}

#[tokio::test]
async fn explore_search_applies_gender_and_country_filters() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/explore/search?q=li&gender=female&countries=US,FR,CL")
                .header("cookie", admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = json(resp).await;
    let rows = v.as_array().unwrap();
    assert!(!rows.is_empty());
    assert!(rows.iter().all(|r| r["card_key"] == "lina"));
}

#[tokio::test]
async fn explore_top_pagination_and_gender_filter_work() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;

    let first_page = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/explore/top?page=0&page_size=1&gender=female&countries=US,FR,CL")
                .header("cookie", admin_cookie.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first_page.status(), StatusCode::OK);
    let p1 = json(first_page).await;
    assert_eq!(p1.as_array().unwrap().len(), 1);
    assert_eq!(p1.as_array().unwrap()[0]["card_key"], "lina");

    let second_page = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/explore/top?page=1&page_size=1&gender=female&countries=US,FR,CL")
                .header("cookie", admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second_page.status(), StatusCode::OK);
    let p2 = json(second_page).await;
    assert!(p2.as_array().unwrap().len() <= 1);
    if let Some(row) = p2.as_array().unwrap().first() {
        assert_eq!(row["card_key"], "lina");
    }
}

#[tokio::test]
async fn card_get_concept_returns_expected_shape() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/card/kyara?page=0&page_size=10&countries=US,FR,CL")
                .header("cookie", admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = json(resp).await;
    assert_eq!(v["card_kind"], Value::String("concept".to_string()));
    assert!(v["all_spellings"].is_array());
    assert!(v["last5_years"].is_object());
    assert!(v["last5_year_ranks"].is_object());
}

#[tokio::test]
async fn card_get_group_returns_expected_shape() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/card/kyara_group?page=0&page_size=10&countries=US,FR,CL")
                .header("cookie", admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v = json(resp).await;
    assert_eq!(v["card_kind"], Value::String("group".to_string()));
    assert!(v["all_spellings"].is_array());
    assert!(v["sex_split"].is_object());
}

#[tokio::test]
async fn card_get_page_size_clamped() {
    let (_dir, _state, app) = setup_app(None).await;
    let admin_cookie = login(&app, "admin", "password123").await;

    let huge = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/card/kyara?page=0&page_size=999&countries=US,FR,CL")
                .header("cookie", admin_cookie.clone())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(huge.status(), StatusCode::OK);
    let huge_v = json(huge).await;
    let huge_spellings = huge_v["all_spellings"].as_array().unwrap();
    assert!(huge_spellings.len() <= 500);

    let clamped = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/card/kyara?page=0&page_size=500&countries=US,FR,CL")
                .header("cookie", admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(clamped.status(), StatusCode::OK);
    let clamped_v = json(clamped).await;
    assert_eq!(huge_v["all_spellings"], clamped_v["all_spellings"]);
}
