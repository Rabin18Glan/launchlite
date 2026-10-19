use crate::routes::AppState;
use common::models::{CreateAppRequest, LogLine};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub async fn list_apps_handler(State(state): State<AppState>) -> Response {
    let apps = state.manager.list_apps().await;
    Json(apps).into_response()
}

pub async fn create_app_handler(
    State(state): State<AppState>,
    Json(req): Json<CreateAppRequest>,
) -> Result<Response, (StatusCode, String)> {
    match state.manager.create_app(req).await {
        Ok(app) => Ok((StatusCode::CREATED, Json(app)).into_response()),
        Err(e) => Err((StatusCode::BAD_REQUEST, e)),
    }
}

pub async fn get_app_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, String)> {
    match state.manager.get_app(&id).await {
        Some(app) => Ok(Json(app).into_response()),
        None => Err((StatusCode::NOT_FOUND, format!("App '{}' not found", id))),
    }
}

pub async fn start_app_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, String)> {
    match state.manager.start_app(&id).await {
        Ok(app) => Ok(Json(app).into_response()),
        Err(e) => Err((StatusCode::BAD_REQUEST, e)),
    }
}

pub async fn stop_app_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, String)> {
    match state.manager.stop_app(&id).await {
        Ok(app) => Ok(Json(app).into_response()),
        Err(e) => Err((StatusCode::BAD_REQUEST, e)),
    }
}

pub async fn restart_app_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, String)> {
    match state.manager.restart_app(&id).await {
        Ok(app) => Ok(Json(app).into_response()),
        Err(e) => Err((StatusCode::BAD_REQUEST, e)),
    }
}

pub async fn delete_app_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, String)> {
    match state.manager.delete_app(&id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT.into_response()),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

pub async fn get_app_logs_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<LogLine>>, (StatusCode, String)> {
    match state.manager.get_logs(&id).await {
        Some(logs) => Ok(Json(logs)),
        None => Err((StatusCode::NOT_FOUND, format!("App '{}' not found", id))),
    }
}
