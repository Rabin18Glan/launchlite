use crate::services::{get_authorization_url, handle_callback, handle_direct_token};
use crate::{services, state::AuthState};
use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
};
use common::models::{DirectTokenRequest, OAuthCallbackQuery};

pub async fn github_login_handler(
    State(state): State<AuthState>,
) -> Result<Redirect, (StatusCode, String)> {
    match get_authorization_url(&state.config) {
        Ok(url) => Ok(Redirect::temporary(&url)),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            format!(
                "GitHub OAuth not configured: {}. You can use a Personal Access Token instead.",
                e
            ),
        )),
    }
}

pub async fn github_callback_handler(
    State(state): State<AuthState>,
    Query(params): Query<OAuthCallbackQuery>,
) -> Result<Response, (StatusCode, String)> {
    if let Some(err) = params.error {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("OAuth error from GitHub: {}", err),
        ));
    }

    let code = params
        .code
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing OAuth code".to_string()))?;

    let session = handle_callback(&state.config, &state.github, &state.repository, &code)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    // Set cookie and redirect to root dashboard
    let cookie = format!(
        "session_id={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000",
        session.session_id
    );

    let mut headers = HeaderMap::new();
    headers.insert(header::SET_COOKIE, cookie.parse().unwrap());
    headers.insert(header::LOCATION, "/".parse().unwrap());

    Ok((StatusCode::FOUND, headers).into_response())
}

pub async fn direct_token_login_handler(
    State(state): State<AuthState>,
    Json(payload): Json<DirectTokenRequest>,
) -> Result<Response, (StatusCode, String)> {
    let session = handle_direct_token(&state.github, &state.repository, &payload.token)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let cookie = format!(
        "session_id={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000",
        session.session_id
    );

    let mut headers = HeaderMap::new();
    headers.insert(header::SET_COOKIE, cookie.parse().unwrap());

    Ok((StatusCode::OK, headers, Json(session.user)).into_response())
}

pub async fn get_current_user_handler(
    State(state): State<AuthState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    let session = get_session_from_headers(&state, &headers)?;
    Ok(Json(session.user).into_response())
}

pub async fn list_github_repos_handler(
    State(state): State<AuthState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    let session = get_session_from_headers(&state, &headers)?;
    let repos = services::list_repositories(&state, &session)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

    Ok(Json(repos).into_response())
}

pub async fn logout_handler(
    State(state): State<AuthState>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, String)> {
    if let Some(cookie_val) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) {
        for cookie in cookie_val.split(';') {
            let parts: Vec<&str> = cookie.trim().split('=').collect();
            if parts.len() == 2 && parts[0] == "session_id" {
                services::logout(&state, parts[1]);
            }
        }
    }

    let cookie = "session_id=; Path=/; HttpOnly; Max-Age=0";
    let mut headers = HeaderMap::new();
    headers.insert(header::SET_COOKIE, cookie.parse().unwrap());
    headers.insert(header::LOCATION, "/".parse().unwrap());

    Ok((StatusCode::FOUND, headers).into_response())
}

pub fn get_session_from_headers(
    state: &AuthState,
    headers: &HeaderMap,
) -> Result<common::models::UserSession, (StatusCode, String)> {
    let session_id = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|value| {
            value.split(';').find_map(|cookie| {
                let (key, value) = cookie.trim().split_once('=')?;
                (key == "session_id").then_some(value)
            })
        });
    services::find_session(state, session_id).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Please connect with GitHub to view your repositories".to_string(),
        )
    })
}
