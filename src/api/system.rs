use crate::api::AppState;
use crate::models::SystemMetrics;
use axum::{Json, extract::State, response::IntoResponse};

pub async fn get_system_metrics_handler(State(state): State<AppState>) -> Json<SystemMetrics> {
    Json(state.manager.get_system_metrics().await)
}

pub async fn health_handler() -> &'static str {
    "OK"
}

pub async fn get_caddyfile_handler(State(state): State<AppState>) -> impl IntoResponse {
    let caddyfile = state.manager.generate_caddyfile().await;
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; charset=utf-8",
        )],
        caddyfile,
    )
}
