pub mod apps;
pub mod auth;
pub mod builds;
pub mod proxy;
pub mod system;

use axum::{
    Router,
    extract::{Request, State},
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use axum_template::RenderHtml;
use std::sync::Arc;

use common::Config;
use runtime::ExecutionManager;
use ::auth::GithubClient;
use web::{TemplateEngine, prepare_dashboard_data};

#[derive(Clone)]
pub struct AppState {
    pub manager: Arc<ExecutionManager>,
    pub github: GithubClient,
    pub config: Config,
    pub engine: TemplateEngine,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(dashboard_or_proxy_handler))
        .route("/favicon.ico", get(favicon_handler))
        .route("/health", get(system::health_handler))
        .route("/api/system", get(system::get_system_metrics_handler))
        .route("/api/system/caddyfile", get(system::get_caddyfile_handler))
        .route("/api/system/export", get(system::export_handler))
        .route("/api/system/import", post(system::import_handler))
        // App lifecycle
        .route(
            "/api/apps",
            get(apps::list_apps_handler).post(apps::create_app_handler),
        )
        .route(
            "/api/apps/{id}",
            get(apps::get_app_handler).delete(apps::delete_app_handler),
        )
        .route("/api/apps/{id}/start", post(apps::start_app_handler))
        .route("/api/apps/{id}/stop", post(apps::stop_app_handler))
        .route("/api/apps/{id}/restart", post(apps::restart_app_handler))
        .route(
            "/api/apps/{id}/redeploy",
            post(builds::redeploy_app_handler),
        )
        .route("/api/apps/{id}/logs", get(apps::get_app_logs_handler))
        .route("/api/apps/{id}/builds", get(builds::get_builds_handler))
        // Git deployment
        .route("/api/deploy/git", post(builds::deploy_git_handler))
        // GitHub Auth & API
        .route("/auth/github/login", get(auth::github_login_handler))
        .route("/auth/github/callback", get(auth::github_callback_handler))
        .route("/auth/token", post(auth::direct_token_login_handler))
        .route("/auth/me", get(auth::get_current_user_handler))
        .route("/auth/repos", get(auth::list_github_repos_handler))
        .route("/auth/logout", get(auth::logout_handler))
        .fallback(proxy::proxy_handler)
        .with_state(state)
}

async fn dashboard_or_proxy_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    req: Request,
) -> Response {
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // If host matches an application's custom domain, proxy the request to that app!
    if let Some((_app_id, target_port)) = state.manager.get_app_by_domain(host).await {
        let target_url = format!("http://127.0.0.1:{}/", target_port);
        match proxy::forward_request(req, &target_url, host).await {
            Ok(resp) => return resp,
            Err(e) => {
                return (
                    axum::http::StatusCode::BAD_GATEWAY,
                    format!("Bad Gateway: {}", e),
                )
                    .into_response();
            }
        }
    }

    // Otherwise render Control Dashboard using axum-template
    let metrics = state.manager.get_system_metrics().await;
    let apps = state.manager.list_apps().await;
    let current_user = auth::get_session_from_headers(&state, &headers)
        .ok()
        .map(|s| s.user);

    let data = prepare_dashboard_data(
        &metrics,
        &apps,
        current_user.as_ref(),
        state.config.has_github_oauth(),
    );

    RenderHtml("dashboard.html", state.engine.clone(), data).into_response()
}

const FAVICON_BYTES: &[u8] = include_bytes!("../../../../assets/favicon.ico");

async fn favicon_handler() -> Response {
    (
        [(header::CONTENT_TYPE, "image/x-icon")],
        FAVICON_BYTES,
    )
        .into_response()
}
