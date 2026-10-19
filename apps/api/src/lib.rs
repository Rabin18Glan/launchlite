pub mod routes;

pub use routes::{AppState, create_router};

use auth::GithubClient;
use common::Config;
use db::Database;
use runtime::ExecutionManager;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tracing::info;
use web::create_template_engine;

pub async fn run_server(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    info!("Initializing SQLite database at: {}", config.db_path);
    let db = Database::new(&config.db_path)?;

    let (manager, restart_rx) = ExecutionManager::new(db, &config.data_dir);
    manager.init(restart_rx).await?;

    let github = GithubClient::new();
    let engine = create_template_engine();
    let state = AppState {
        manager: Arc::clone(&manager),
        github,
        config: config.clone(),
        engine,
    };

    let app = create_router(state);

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    info!("Starting LaunchLite server on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(Arc::clone(&manager)))
        .await?;

    info!("LaunchLite server shut down gracefully.");
    Ok(())
}

async fn shutdown_signal(manager: Arc<ExecutionManager>) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Termination signal received. Gracefully stopping all applications...");
    let apps = manager.list_apps().await;
    for app in apps {
        if let Err(e) = manager.stop_app(&app.config.id).await {
            tracing::warn!("Error stopping app {}: {}", app.config.id, e);
        }
    }
    info!("All applications stopped.");
}
