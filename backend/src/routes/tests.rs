use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;

use crate::bootstrap::bootstrap_admin;
use crate::routes::router;
use crate::state::AppState;

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

    let state = AppState::new(&db_path, false).await.unwrap();
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
