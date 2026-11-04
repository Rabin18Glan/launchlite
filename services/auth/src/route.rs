use crate::handler;
use axum::{
    Router,
    routing::{get, post},
};

pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    crate::state::AuthState: axum::extract::FromRef<S>,
{
    Router::new()
        .nest("/admin", crate::admin::route::router())
        .nest(
            "/github",
            Router::new()
                .route("/login", get(handler::github_login_handler))
                .route("/callback", get(handler::github_callback_handler)),
        )
        .route("/token", post(handler::direct_token_login_handler))
        .route("/me", get(handler::get_current_user_handler))
        .route("/repos", get(handler::list_github_repos_handler))
        .route("/logout", get(handler::logout_handler))
}
