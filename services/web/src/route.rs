use crate::{handler, state::WebState};
use axum::{Router, extract::FromRef, routing::get};

pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    WebState: FromRef<S>,
{
    Router::new()
        .route("/", get(handler::dashboard_handler))
        .route("/login", get(handler::login_page))
        .route("/setup", get(handler::login_page))
        .nest(
            "/projects",
            Router::new().route("/new", get(handler::new_project_handler)),
        )
        .route("/favicon.ico", get(handler::favicon_handler))
}
