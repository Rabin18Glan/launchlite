use crate::state::RuntimeState;
use auth::handler::get_session_from_headers;
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use common::models::{BuildJob, DeployRepoRequest};

pub async fn deploy_git_handler(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    Json(req): Json<DeployRepoRequest>,
) -> Result<Response, (StatusCode, String)> {
    let token = get_session_from_headers(&state.auth, &headers)
        .ok()
        .map(|s| s.access_token);

    match state.manager.deploy_git_repo(req, token).await {
        Ok(app) => Ok((StatusCode::ACCEPTED, Json(app)).into_response()),
        Err(e) => Err((StatusCode::BAD_REQUEST, e)),
    }
}

pub async fn redeploy_app_handler(
    State(state): State<RuntimeState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, (StatusCode, String)> {
    let token = get_session_from_headers(&state.auth, &headers)
        .ok()
        .map(|s| s.access_token);

    match state.manager.redeploy_app(&id, token).await {
        Ok(_) => Ok((StatusCode::ACCEPTED, "Redeploy triggered").into_response()),
        Err(e) => Err((StatusCode::BAD_REQUEST, e)),
    }
}

pub async fn get_builds_handler(
    State(state): State<RuntimeState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<BuildJob>>, (StatusCode, String)> {
    match state.manager.get_builds_for_app(&id) {
        Ok(builds) => Ok(Json(builds)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}
