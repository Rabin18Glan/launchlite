use crate::state::AppState;
use axum::{Router, routing::get};

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .merge(web::route::router())
        .route("/health", get(runtime::handlers::system::health_handler))
        .nest("/api", runtime::route::router())
        .nest("/auth", ::auth::route::router())
        .fallback(crate::proxy::proxy_handler)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::access::control_access,
        ))
        .with_state(state)
}
