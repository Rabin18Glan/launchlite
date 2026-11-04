use super::services::{AuthError, COOKIE_NAME, SESSION_SECONDS};
use crate::state::AuthState;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}
#[derive(Deserialize)]
pub struct Setup {
    pub setup_token: String,
    pub username: String,
    pub password: String,
}

pub fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|value| {
            let (key, value) = value.trim().split_once('=')?;
            (key == name).then_some(value)
        })
}

pub async fn setup(State(state): State<AuthState>, Json(input): Json<Setup>) -> Response {
    let secure = state.admin.secure_cookies;
    match tokio::task::spawn_blocking(move || {
        state
            .admin
            .setup(&input.setup_token, &input.username, &input.password)
    })
    .await
    {
        Ok(Ok(token)) => session_response(&token, secure),
        Ok(Err(error)) => auth_error(error),
        Err(error) => auth_error(AuthError::Internal(error.to_string())),
    }
}

pub async fn login(State(state): State<AuthState>, Json(input): Json<Credentials>) -> Response {
    let secure = state.admin.secure_cookies;
    match tokio::task::spawn_blocking(move || state.admin.login(&input.username, &input.password))
        .await
    {
        Ok(Ok(token)) => session_response(&token, secure),
        Ok(Err(error)) => auth_error(error),
        Err(error) => auth_error(AuthError::Internal(error.to_string())),
    }
}

pub async fn logout(State(state): State<AuthState>, headers: HeaderMap) -> Response {
    if let Some(token) = cookie(&headers, COOKIE_NAME) {
        if let Err(error) = state.admin.logout(token) {
            return auth_error(error);
        }
    }
    let secure = if state.admin.secure_cookies {
        "; Secure"
    } else {
        ""
    };
    (
        [(
            header::SET_COOKIE,
            format!("{COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{secure}"),
        )],
        StatusCode::NO_CONTENT,
    )
        .into_response()
}

fn session_response(token: &str, secure: bool) -> Response {
    let secure = if secure { "; Secure" } else { "" };
    ([(header::SET_COOKIE, format!("{COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={SESSION_SECONDS}{secure}"))], Json(serde_json::json!({"ok": true}))).into_response()
}

pub fn auth_error(error: AuthError) -> Response {
    let (status, message) = match error {
        AuthError::InvalidCredentials => {
            (StatusCode::UNAUTHORIZED, "Invalid username or password.")
        }
        AuthError::InvalidSetupToken => (
            StatusCode::FORBIDDEN,
            "Invalid setup token. Read the initial-admin-token file on the server.",
        ),
        AuthError::AlreadyConfigured => (
            StatusCode::CONFLICT,
            "Administrator already configured. Please sign in.",
        ),
        AuthError::InvalidInput(message) => (StatusCode::BAD_REQUEST, message),
        AuthError::RateLimited => {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                [(header::RETRY_AFTER, "60")],
                "Too many attempts. Try again in one minute.",
            )
                .into_response();
        }
        AuthError::Internal(error) => {
            tracing::error!(error = %error, "Administrator authentication failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Unable to complete authentication.",
            )
        }
    };
    (status, message).into_response()
}
