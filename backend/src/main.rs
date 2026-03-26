mod bootstrap;
mod infrastructure;
mod router;
mod shared;

use crate::bootstrap::container::Container;
use crate::router::create_router;
use crate::shared::tracing::init_tracing;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    init_tracing();

    info!("Starting JustQiu Backend Server...");

    // Build application container
    let state = Container::build().await?;

    // Create router
    let app = create_router(state.clone());

    // Define address
    let addr = SocketAddr::from(([0, 0, 0, 0], state.config.port));
    let listener = TcpListener::bind(addr).await?;
    
    info!("Server listening on {}", addr);

    // Run server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Server stopped.");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, starting graceful shutdown...");
        },
        _ = terminate => {
            info!("Received SIGTERM, starting graceful shutdown...");
        },
    }
}
