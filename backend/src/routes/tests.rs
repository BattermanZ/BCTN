use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
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
        "INSERT INTO import_run (id, window_start_year, window_end_year) VALUES (1, 2005, 2024)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO name_observation (run_id, country, count) VALUES (1, 'US', 1), (1, 'FR', 1), (1, 'CL', 1)")
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

#[tokio::test]
async fn bootstrap_then_login_then_me_and_admin_list_users() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("app.sqlite");
    let names_db_path = dir.path().join("names.sqlite");
    create_min_names_db(&names_db_path).await;

    let state = AppState::new(&db_path, &names_db_path, false)
        .await
        .unwrap();
    state.migrate().await.unwrap();

    bootstrap_admin(&state.db, "admin", "password123")
        .await
        .unwrap();

    let app = router(state);

    let login_body = r#"{"username":"admin","password":"password123"}"#;
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
