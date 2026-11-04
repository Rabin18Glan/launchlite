use crate::TemplateEngine;
use auth::state::AuthState;
use common::Config;
use runtime::ExecutionManager;
use std::sync::Arc;

#[derive(Clone)]
pub struct WebState {
    pub manager: Arc<ExecutionManager>,
    pub auth: AuthState,
    pub config: Config,
    pub engine: TemplateEngine,
}
