use super::handler;
use axum::{Router, routing::post};

pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    crate::state::AuthState: axum::extract::FromRef<S>,
{
    Router::new()
        .route("/setup", post(handler::setup))
        .route("/login", post(handler::login))
        .route("/logout", post(handler::logout))
}
