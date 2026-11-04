mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use common::{TestApp, json, text};
use runtime::repository::AppRepository;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn pages_and_health_return_expected_content() {
    let app = TestApp::new();
    for (path, expected) in [
        ("/", "Projects"),
        ("/projects/new", "Application preset"),
        ("/health", "OK"),
    ] {
        let response = app.request("GET", path, None).await;
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert!(text(response).await.contains(expected), "{path}");
    }
    let response = app.request("GET", "/api/system", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(json(response).await.is_object());
    let response = app.request("GET", "/api/apps/missing/builds", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await, json!([]));
}

#[tokio::test]
async fn creating_listing_exporting_and_deleting_an_app_persists_expected_data() {
    let app = TestApp::new();
    let repository = AppRepository::new(app.database.clone());
    let response = app.request("POST", "/api/apps", Some(json!({
        "id": "example", "name": "Example", "executable": "/unused/server", "auto_start": false,
        "domain": "App.Example.com", "port": 8080, "env": {"MODE": "test"}
    }))).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let created = json(response).await;
    assert_eq!(created["config"]["id"], "example");
    assert_eq!(created["config"]["domain"], "app.example.com");
    assert_eq!(created["config"]["env"]["PORT"], "8080");
    let saved = repository.get_all_apps().unwrap();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].domain.as_deref(), Some("app.example.com"));
    assert_eq!(saved[0].env.get("MODE").map(String::as_str), Some("test"));

    let response = app.request("GET", "/api/apps/example", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["config"], created["config"]);
    let response = app.request("GET", "/api/apps", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await[0]["config"], created["config"]);
    let response = app.request("GET", "/api/system/export", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
    assert!(
        response.headers()[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .contains("attachment;")
    );
    assert_eq!(json(response).await["apps"][0]["id"], "example");

    let response = app.request("DELETE", "/api/apps/example", None).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(text(response).await.is_empty());
    assert!(repository.get_all_apps().unwrap().is_empty());
    assert_eq!(
        json(app.request("GET", "/api/apps", None).await).await,
        json!([])
    );
    let response = app.request("GET", "/api/apps/example", None).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(text(response).await.contains("example"));
}

#[tokio::test]
async fn invalid_payloads_are_rejected_without_creating_apps() {
    let app = TestApp::new();
    for payload in [
        json!({}),
        json!({"name": 123}),
        json!({"name": "Test", "port": 65536}),
    ] {
        let response = app.request("POST", "/api/apps", Some(payload)).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert!(!text(response).await.is_empty());
    }
    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/apps")
                .header("cookie", app.admin_cookie.as_ref().unwrap())
                .header("content-type", "application/json")
                .body(Body::from("{"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(!text(response).await.is_empty());
    assert!(
        AppRepository::new(app.database.clone())
            .get_all_apps()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        json(app.request("GET", "/api/apps", None).await).await,
        json!([])
    );
}

#[tokio::test]
async fn failed_persistence_returns_an_error_without_registering_the_app() {
    let app = TestApp::new();
    app.database.connection().unwrap().execute_batch(
        "CREATE TRIGGER reject_app BEFORE INSERT ON apps BEGIN SELECT RAISE(ABORT, 'test write rejection'); END;"
    ).unwrap();
    let response = app
        .request(
            "POST",
            "/api/apps",
            Some(json!({"id": "rejected", "name": "Rejected", "auto_start": false})),
        )
        .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let message = text(response).await;
    assert!(message.contains("DB error:"));
    assert!(message.contains("test write rejection"));
    assert!(
        AppRepository::new(app.database.clone())
            .get_all_apps()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        json(app.request("GET", "/api/apps", None).await).await,
        json!([])
    );
}

#[tokio::test]
async fn authentication_errors_include_useful_response_bodies() {
    let app = TestApp::new();
    for (path, status, message) in [
        (
            "/auth/me",
            StatusCode::UNAUTHORIZED,
            "Please connect with GitHub",
        ),
        (
            "/auth/github/login",
            StatusCode::BAD_REQUEST,
            "GitHub OAuth not configured",
        ),
        (
            "/auth/github/callback",
            StatusCode::BAD_REQUEST,
            "Missing OAuth code",
        ),
        (
            "/auth/github/callback?error=access_denied",
            StatusCode::BAD_REQUEST,
            "access_denied",
        ),
    ] {
        let response = app.request("GET", path, None).await;
        assert_eq!(response.status(), status, "{path}");
        assert!(text(response).await.contains(message), "{path}");
    }
}

#[tokio::test]
async fn session_cookie_returns_user_and_logout_deletes_the_session() {
    let app = TestApp::new();
    let repository = auth::repository::SessionRepository::new(app.database.clone());
    let session = ::common::UserSession {
        session_id: "test-session".into(),
        access_token: "private-token".into(),
        created_at: 1,
        user: ::common::GithubUser {
            id: 1,
            login: "test-user".into(),
            name: None,
            avatar_url: "avatar".into(),
            html_url: "profile".into(),
        },
    };
    repository.save_session(&session).unwrap();
    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/me")
                .header(
                    "cookie",
                    format!(
                        "{}; session_id=test-session",
                        app.admin_cookie.as_ref().unwrap()
                    ),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let user = json(response).await;
    assert_eq!(user["login"], "test-user");
    assert!(user.get("access_token").is_none());
    let response = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/logout")
                .header(
                    "cookie",
                    format!(
                        "{}; session_id=test-session",
                        app.admin_cookie.as_ref().unwrap()
                    ),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FOUND);
    assert_eq!(response.headers()[header::LOCATION], "/");
    assert!(
        response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    assert!(repository.get_session("test-session").unwrap().is_none());
    assert_eq!(
        app.request("GET", "/auth/me", None).await.status(),
        StatusCode::UNAUTHORIZED
    );
}
