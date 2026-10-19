mod api;
mod auth;
mod builder;
mod config;
mod db;
mod execution;
mod github;
mod metrics;
mod models;
mod web;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::api::{AppState, create_router};
use crate::config::Config;
use crate::db::Database;
use crate::execution::ExecutionManager;
use crate::github::GithubClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "launchlite=info,serverless_platform=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();

    info!("Initializing SQLite database at: {}", config.db_path);
    let db = Database::new(&config.db_path)?;

    let (manager, restart_rx) = ExecutionManager::new(db, &config.data_dir);
    manager.init(restart_rx).await?;

    let github = GithubClient::new();
    let engine = crate::web::create_template_engine();
    let state = AppState {
        manager: Arc::clone(&manager),
        github,
        config: config.clone(),
        engine,
    };

    let app = create_router(state);

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    info!("Starting LaunchLite on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(Arc::clone(&manager)))
        .await?;

    info!("LaunchLite shut down gracefully.");
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
