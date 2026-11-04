use crate::handlers::{apps, builds, system};
use axum::{
    Router,
    routing::{get, post},
};

pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    crate::state::RuntimeState: axum::extract::FromRef<S>,
{
    Router::new()
        // Keep collection routes here to preserve URLs without a trailing slash.
        .route("/system", get(system::get_system_metrics_handler))
        .nest("/system", system_router())
        .route(
            "/apps",
            get(apps::list_apps_handler).post(apps::create_app_handler),
        )
        .nest("/apps", apps_router())
        .nest(
            "/deploy",
            Router::new().route("/git", post(builds::deploy_git_handler)),
        )
}

fn system_router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    crate::state::RuntimeState: axum::extract::FromRef<S>,
{
    Router::new()
        .route("/caddyfile", get(system::get_caddyfile_handler))
        .route("/export", get(system::export_handler))
        .route("/import", post(system::import_handler))
}

fn apps_router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    crate::state::RuntimeState: axum::extract::FromRef<S>,
{
    Router::new()
        .route(
            "/{id}",
            get(apps::get_app_handler).delete(apps::delete_app_handler),
        )
        .route("/{id}/start", post(apps::start_app_handler))
        .route("/{id}/stop", post(apps::stop_app_handler))
        .route("/{id}/restart", post(apps::restart_app_handler))
        .route("/{id}/redeploy", post(builds::redeploy_app_handler))
        .route("/{id}/logs", get(apps::get_app_logs_handler))
        .route("/{id}/builds", get(builds::get_builds_handler))
}
