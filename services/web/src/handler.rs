use crate::prepare_dashboard_data;
use crate::state::WebState;
use axum::{
    extract::State,
    http::{HeaderMap, header},
    response::{Html, IntoResponse, Response},
};
use axum_template::RenderHtml;

pub async fn new_project_handler() -> Html<&'static str> {
    Html(include_str!("templates/new_project.html"))
}

pub async fn dashboard_handler(State(state): State<WebState>, headers: HeaderMap) -> Response {
    // Otherwise render Control Dashboard using axum-template
    let metrics = state.manager.get_system_metrics().await;
    let apps = state.manager.list_apps().await;
    let current_user = ::auth::handler::get_session_from_headers(&state.auth, &headers)
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

const FAVICON_BYTES: &[u8] = include_bytes!("../../../assets/favicon.ico");

pub async fn favicon_handler() -> Response {
    ([(header::CONTENT_TYPE, "image/x-icon")], FAVICON_BYTES).into_response()
}

pub async fn login_page(
    State(state): State<WebState>,
    headers: HeaderMap,
    uri: axum::http::Uri,
) -> Response {
    use auth::admin::{
        handler::{auth_error, cookie},
        services::COOKIE_NAME,
    };
    use axum::response::Redirect;
    match state
        .auth
        .admin
        .authenticated(cookie(&headers, COOKIE_NAME))
    {
        Ok(true) => return Redirect::to("/").into_response(),
        Err(error) => return auth_error(error),
        _ => {}
    }
    match state.auth.admin.setup_required() {
        Ok(true) if uri.path() != "/setup" => Redirect::to("/setup").into_response(),
        Ok(false) if uri.path() == "/setup" => Redirect::to("/login").into_response(),
        Ok(_) => Html(include_str!("templates/login.html")).into_response(),
        Err(error) => auth_error(error),
    }
}
