mod access;
mod bootstrap;
mod proxy;
pub mod route;
mod shutdown;
pub mod state;

pub use route::create_router;
pub use state::AppState;

use common::Config;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

pub async fn init(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let state = bootstrap::initialize(&config).await?;
    let manager = Arc::clone(&state.manager);
    let app_router = create_router(state);

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    info!("Starting LaunchLite server on http://{}", addr);
    let listener = TcpListener::bind(&addr).await?;

    axum::serve(listener, app_router)
        .with_graceful_shutdown(shutdown::wait_for_shutdown(manager))
        .await?;

    info!("LaunchLite server shut down gracefully.");
    Ok(())
}
