use crate::state::AppState;
use auth::admin::{
    handler::{auth_error, cookie},
    services::COOKIE_NAME,
};
use axum::{
    extract::{Request, State},
    http::{Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};

pub async fn control_access(State(state): State<AppState>, req: Request, next: Next) -> Response {
    // Host applications retain their own access policies and never enter control routes.
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if state.manager.get_app_by_domain(host).await.is_some() {
        return crate::proxy::proxy_handler(State(state), req).await;
    }
    let response = check_access(state, req, next).await;
    let (mut parts, body) = response.into_parts();
    parts
        .headers
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    parts
        .headers
        .insert(header::X_FRAME_OPTIONS, "DENY".parse().unwrap());
    parts
        .headers
        .insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    Response::from_parts(parts, body)
}

async fn check_access(state: AppState, req: Request, next: Next) -> Response {
    if !matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS) {
        // Browser mutations must originate from this control panel. Do not trust
        // forwarded host headers supplied by arbitrary clients.
        let cross_site = req
            .headers()
            .get("sec-fetch-site")
            .is_some_and(|value| value == "cross-site");
        let wrong_origin = req.headers().get(header::ORIGIN).is_some_and(|origin| {
            let origin = origin
                .to_str()
                .ok()
                .and_then(|value| reqwest::Url::parse(value).ok());
            let host = req
                .headers()
                .get(header::HOST)
                .and_then(|value| value.to_str().ok());
            match (origin, host) {
                (Some(origin), Some(host)) if matches!(origin.scheme(), "http" | "https") => origin
                    .as_str()
                    .strip_prefix(&format!("{}://", origin.scheme()))
                    .and_then(|value| value.split('/').next())
                    .is_none_or(|authority| !authority.eq_ignore_ascii_case(host)),
                _ => true,
            }
        });
        if cross_site || wrong_origin {
            return (StatusCode::FORBIDDEN, "Cross-origin request rejected.").into_response();
        }
    }
    let path = req.uri().path();
    if matches!(
        path,
        "/login"
            | "/setup"
            | "/auth/admin/login"
            | "/auth/admin/setup"
            | "/favicon.ico"
            | "/health"
    ) {
        return next.run(req).await;
    }
    match state
        .auth
        .admin
        .authenticated(cookie(req.headers(), COOKIE_NAME))
    {
        Ok(true) => next.run(req).await,
        Err(error) => auth_error(error),
        Ok(false) => {
            if path.starts_with("/api/") || path.starts_with("/auth/") {
                return (StatusCode::UNAUTHORIZED, "Administrator login required.").into_response();
            }
            match state.auth.admin.setup_required() {
                Ok(true) => Redirect::to("/setup").into_response(),
                Ok(false) => Redirect::to("/login").into_response(),
                Err(error) => auth_error(error),
            }
        }
    }
}
