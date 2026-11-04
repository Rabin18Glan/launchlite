mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use common::{PASSWORD, SETUP_TOKEN, TestApp, json, text};
use serde_json::json;
use tower::ServiceExt;

fn setup_payload() -> serde_json::Value {
    json!({"setup_token": SETUP_TOKEN, "username": "owner", "password": PASSWORD})
}

fn cookie(response: &axum::response::Response) -> String {
    let value = response.headers()[header::SET_COOKIE].to_str().unwrap();
    assert!(value.contains("HttpOnly"));
    assert!(value.contains("SameSite=Lax"));
    assert!(value.contains("Path=/"));
    value.split(';').next().unwrap().to_string()
}

async fn get_with_cookie(app: &TestApp, path: &str, cookie: &str) -> axum::response::Response {
    app.router
        .clone()
        .oneshot(
            Request::builder()
                .uri(path)
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn unconfigured_instance_requires_setup_and_blocks_management_endpoints() {
    let app = TestApp::unconfigured();
    for path in ["/", "/projects/new"] {
        let response = app.request("GET", path, None).await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.headers()[header::LOCATION], "/setup");
    }
    let response = app.request("GET", "/setup", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert!(text(response).await.contains("setupToken"));
    for (method, path) in [
        ("GET", "/api/apps"),
        ("POST", "/api/apps"),
        ("DELETE", "/api/apps/example"),
        ("POST", "/api/apps/example/start"),
        ("POST", "/api/apps/example/stop"),
        ("POST", "/api/apps/example/restart"),
        ("GET", "/api/apps/example/logs"),
        ("GET", "/api/apps/example/builds"),
        ("GET", "/api/system"),
        ("GET", "/api/system/export"),
        ("POST", "/api/system/import"),
        ("POST", "/api/deploy/git"),
        ("POST", "/api/apps/example/redeploy"),
        ("POST", "/auth/token"),
        ("GET", "/auth/repos"),
        ("GET", "/auth/github/callback"),
    ] {
        let response = app.request(method, path, None).await;
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "{method} {path}"
        );
        assert!(
            text(response)
                .await
                .contains("Administrator login required")
        );
    }
    assert_eq!(
        app.request("GET", "/health", None).await.status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn setup_hashes_credentials_issues_session_and_cannot_be_repeated() {
    let app = TestApp::unconfigured();
    let response = app
        .request("POST", "/auth/admin/setup", Some(setup_payload()))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let session_cookie = cookie(&response);
    assert!(
        response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=43200")
    );
    assert_eq!(json(response).await, json!({"ok": true}));
    let conn = app.database.connection().unwrap();
    let hash: String = conn
        .query_row("SELECT password_hash FROM admin_account", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(hash.starts_with("$argon2id$"));
    assert!(!hash.contains(PASSWORD));
    let stored_token: String = conn
        .query_row("SELECT token_hash FROM admin_sessions", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_ne!(stored_token, session_cookie.split_once('=').unwrap().1);
    drop(conn);
    let response = get_with_cookie(&app, "/", &session_cookie).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(text(response).await.contains("Projects"));
    assert_eq!(
        get_with_cookie(&app, "/api/apps", &session_cookie)
            .await
            .status(),
        StatusCode::OK
    );
    let response = app
        .request("POST", "/auth/admin/setup", Some(setup_payload()))
        .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert!(response.headers().get(header::SET_COOKIE).is_none());
    assert_eq!(
        app.request("GET", "/setup", None).await.headers()[header::LOCATION],
        "/login"
    );
    assert_eq!(
        app.request("GET", "/", None).await.headers()[header::LOCATION],
        "/login"
    );
}

#[tokio::test]
async fn setup_requires_the_server_token_and_a_strong_password() {
    let app = TestApp::unconfigured();
    let mut payload = setup_payload();
    payload["setup_token"] = json!("wrong-token");
    assert_eq!(
        app.request("POST", "/auth/admin/setup", Some(payload))
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let mut payload = setup_payload();
    payload["password"] = json!("short");
    assert_eq!(
        app.request("POST", "/auth/admin/setup", Some(payload))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    let mut payload = setup_payload();
    payload["username"] = json!("invalid name");
    assert_eq!(
        app.request("POST", "/auth/admin/setup", Some(payload))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        app.database
            .connection()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM admin_account", [], |row| row
                .get::<_, u64>(0))
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn login_rejects_wrong_credentials_rotates_tokens_and_logout_revokes_access() {
    let app = TestApp::new();
    for payload in [
        json!({"username": "test-admin", "password": "incorrect"}),
        json!({"username": "wrong-user", "password": PASSWORD}),
    ] {
        let response = app
            .request("POST", "/auth/admin/login", Some(payload))
            .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(response.headers().get(header::SET_COOKIE).is_none());
        assert_eq!(text(response).await, "Invalid username or password.");
    }
    let response = app
        .request(
            "POST",
            "/auth/admin/login",
            Some(json!({"username": "test-admin", "password": PASSWORD})),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let new_cookie = cookie(&response);
    assert_ne!(new_cookie, *app.admin_cookie.as_ref().unwrap());
    assert_eq!(
        get_with_cookie(&app, "/api/apps", &new_cookie)
            .await
            .status(),
        StatusCode::OK
    );
    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/admin/logout")
                .header("cookie", &new_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(
        response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    assert_eq!(
        get_with_cookie(&app, "/api/apps", &new_cookie)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn expired_or_tampered_cookies_cannot_access_the_control_panel() {
    let app = TestApp::new();
    assert_eq!(
        get_with_cookie(&app, "/api/apps", "launchlite_admin=invalid")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        get_with_cookie(
            &app,
            "/api/apps",
            &format!("launchlite_admin={}", "0".repeat(64))
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    app.database
        .connection()
        .unwrap()
        .execute("UPDATE admin_sessions SET expires_at = 0", [])
        .unwrap();
    assert_eq!(
        get_with_cookie(&app, "/api/apps", app.admin_cookie.as_ref().unwrap())
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn configured_https_cookies_are_secure_and_cross_origin_mutations_are_rejected() {
    let app = TestApp::with_secure_cookies(true);
    let response = app
        .request("POST", "/auth/admin/setup", Some(setup_payload()))
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("; Secure")
    );
    let admin_cookie = cookie(&response);
    for origin in [
        "https://attacker.example",
        "null",
        "https://control.example.attacker.example",
    ] {
        let response = app
            .router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/admin/logout")
                    .header("host", "control.example")
                    .header("origin", origin)
                    .header("cookie", &admin_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/admin/logout")
                .header("host", "control.example")
                .header("origin", "https://control.example")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn repeated_setup_attempts_are_rate_limited() {
    let app = TestApp::unconfigured();
    for _ in 0..10 {
        assert_eq!(
            app.request(
                "POST",
                "/auth/admin/setup",
                Some(json!({"setup_token": "invalid", "username": "owner", "password": PASSWORD}))
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
    }
    let response = app
        .request("POST", "/auth/admin/setup", Some(setup_payload()))
        .await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers()[header::RETRY_AFTER], "60");
}

#[tokio::test]
async fn stored_github_credentials_do_not_authenticate_anonymous_requests() {
    let app = TestApp::new();
    auth::repository::SessionRepository::new(app.database.clone())
        .save_session(&::common::UserSession {
            session_id: "github-session".into(),
            access_token: "github-token".into(),
            created_at: 1,
            user: ::common::GithubUser {
                id: 1,
                login: "test".into(),
                name: None,
                avatar_url: "".into(),
                html_url: "".into(),
            },
        })
        .unwrap();
    assert_eq!(
        get_with_cookie(&app, "/api/apps", "session_id=github-session")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    // Even an authenticated administrator must explicitly connect this browser's GitHub session.
    assert_eq!(
        app.request("GET", "/auth/me", None).await.status(),
        StatusCode::UNAUTHORIZED
    );
}
