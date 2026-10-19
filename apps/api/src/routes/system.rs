use crate::routes::AppState;
use common::models::{ImportSummary, PlatformExport, SystemMetrics};
use axum::{
    Json,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
};

pub async fn get_system_metrics_handler(State(state): State<AppState>) -> Json<SystemMetrics> {
    Json(state.manager.get_system_metrics().await)
}

pub async fn health_handler() -> &'static str {
    "OK"
}

pub async fn get_caddyfile_handler(State(state): State<AppState>) -> impl IntoResponse {
    let caddyfile = state.manager.generate_caddyfile().await;
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        caddyfile,
    )
}

pub async fn export_handler(State(state): State<AppState>) -> impl IntoResponse {
    let export = state.manager.export_data().await;
    let json_bytes = serde_json::to_vec_pretty(&export).unwrap_or_default();
    let filename = format!("launchlite-backup-{}.json", export.exported_at);

    (
        [
            (header::CONTENT_TYPE, "application/json".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        json_bytes,
    )
}

pub async fn import_handler(
    State(state): State<AppState>,
    Json(payload): Json<PlatformExport>,
) -> Result<Json<ImportSummary>, (StatusCode, String)> {
    state
        .manager
        .import_data(payload)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))
}
