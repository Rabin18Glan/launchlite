use auth::state::AuthState;
use common::Config;
use runtime::ExecutionManager;
use std::sync::Arc;
use web::TemplateEngine;

#[derive(Clone)]
pub struct AppState {
    pub manager: Arc<ExecutionManager>,
    pub auth: AuthState,
    pub config: Config,
    pub engine: TemplateEngine,
}

impl axum::extract::FromRef<AppState> for AuthState {
    fn from_ref(state: &AppState) -> Self {
        state.auth.clone()
    }
}

impl axum::extract::FromRef<AppState> for runtime::state::RuntimeState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            manager: state.manager.clone(),
            auth: state.auth.clone(),
        }
    }
}

impl axum::extract::FromRef<AppState> for web::state::WebState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            manager: state.manager.clone(),
            auth: state.auth.clone(),
            config: state.config.clone(),
            engine: state.engine.clone(),
        }
    }
}
