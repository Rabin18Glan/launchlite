use runtime::ExecutionManager;
use std::sync::Arc;
use tokio::signal;
use tracing::info;

pub(crate) async fn wait_for_shutdown(manager: Arc<ExecutionManager>) {
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
