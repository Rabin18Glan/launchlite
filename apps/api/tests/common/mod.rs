use api::{AppState, create_router};
use auth::{GithubClient, repository::SessionRepository, state::AuthState};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, Response},
};
use common::Config;
use db::Database;
use tower::ServiceExt;

pub const SETUP_TOKEN: &str = "test-only-setup-token";
pub const PASSWORD: &str = "test-password-12345";

pub struct TestApp {
    pub router: Router,
    pub database: Database,
    pub admin_cookie: Option<String>,
}

impl TestApp {
    pub fn new() -> Self {
        let mut app = Self::unconfigured();
        let service = auth::admin::services::AdminService::new(
            auth::admin::repository::AdminRepository::new(app.database.clone()),
            Some(SETUP_TOKEN),
            false,
        );
        let token = service.setup(SETUP_TOKEN, "test-admin", PASSWORD).unwrap();
        app.admin_cookie = Some(format!("launchlite_admin={token}"));
        app
    }

    pub fn unconfigured() -> Self {
        Self::with_secure_cookies(false)
    }

    pub fn with_secure_cookies(secure: bool) -> Self {
        let database = Database::memory().unwrap();
        // These tests never start processes, clone repositories, or write to data_dir.
        let config = Config {
            host: "127.0.0.1".into(),
            port: 3000,
            db_path: ":memory:".into(),
            data_dir: "/unused/test-data".into(),
            github_client_id: None,
            github_client_secret: None,
            redirect_uri: "http://127.0.0.1:3000/auth/github/callback".into(),
        };
        let (manager, _restart_rx) =
            runtime::ExecutionManager::new(database.clone(), &config.data_dir);
        let state = AppState {
            manager,
            auth: AuthState {
                admin: auth::admin::services::AdminService::new(
                    auth::admin::repository::AdminRepository::new(database.clone()),
                    Some(SETUP_TOKEN),
                    secure,
                ),
                github: GithubClient::new(),
                config: config.clone(),
                repository: SessionRepository::new(database.clone()),
            },
            config,
            engine: web::create_template_engine(),
        };
        Self {
            router: create_router(state),
            database,
            admin_cookie: None,
        }
    }

    pub async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Response<Body> {
        let mut request = Request::builder().method(method).uri(path);
        if let Some(cookie) = &self.admin_cookie {
            request = request.header("cookie", cookie);
        }
        let request = match body {
            Some(body) => request
                .header("content-type", "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        }
        .unwrap();
        self.router.clone().oneshot(request).await.unwrap()
    }
}

pub async fn text(response: Response<Body>) -> String {
    String::from_utf8(
        to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

pub async fn json(response: Response<Body>) -> serde_json::Value {
    serde_json::from_str(&text(response).await).unwrap()
}
