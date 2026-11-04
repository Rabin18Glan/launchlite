#[derive(Clone)]
pub struct RuntimeState {
    pub manager: std::sync::Arc<crate::ExecutionManager>,
    pub auth: auth::state::AuthState,
}
