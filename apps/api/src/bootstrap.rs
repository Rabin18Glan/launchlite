use crate::state::AppState;
use auth::{
    GithubClient,
    admin::{repository::AdminRepository, services::AdminService},
    repository::SessionRepository,
    state::AuthState,
};
use common::Config;
use db::Database;
use runtime::ExecutionManager;
use std::sync::Arc;
use tracing::info;
use web::create_template_engine;

pub(crate) async fn initialize(config: &Config) -> Result<AppState, Box<dyn std::error::Error>> {
    info!("Initializing SQLite database at: {}", config.db_path);
    let db = Database::new(&config.db_path)?;

    let (manager, restart_rx) = ExecutionManager::new(db.clone(), &config.data_dir);
    manager.init(restart_rx).await?;

    let secure_cookies = std::env::var("LAUNCHLITE_COOKIE_SECURE")
        .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"));
    let admin = AdminService::from_data_dir(
        AdminRepository::new(db.clone()),
        &config.data_dir,
        secure_cookies,
    )
    .map_err(std::io::Error::other)?;
    let github = GithubClient::new();
    let engine = create_template_engine();
    Ok(AppState {
        manager: Arc::clone(&manager),
        auth: AuthState {
            admin,
            github,
            config: config.clone(),
            repository: SessionRepository::new(db),
        },
        config: config.clone(),
        engine,
    })
}
