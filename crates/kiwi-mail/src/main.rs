use std::sync::Arc;

use tokio::net::TcpListener;
use tracing::info;

mod api;
mod config;
mod jmap;
mod upstream;

pub struct AppState {
    pub upstream: upstream::Upstream,
    pub jmap: jmap::JmapClient,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = config::Config::load()?;

    // Initialize tracing
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&config.log_level));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .init();

    info!("kiwi-mail starting");

    // Spawn and health-check upstream
    let upstream = upstream::Upstream::spawn(&config).await?;
    info!("upstream is healthy");

    // Discover JMAP session
    let jmap = jmap::JmapClient::discover(
        &config.upstream_addr,
        &config.admin_user,
        &config.admin_pass,
    )
    .await?;
    info!("JMAP session discovered");

    let state = Arc::new(AppState { upstream, jmap });

    let app = api::router(state.clone());

    let listener = TcpListener::bind(&config.listen_addr).await?;
    info!(addr = %config.listen_addr, "listening");

    // Graceful shutdown on SIGTERM/SIGINT
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            info!("shutdown signal received");
            // Extract the upstream from Arc — we can't move out of Arc,
            // so we just log that shutdown is happening. The upstream child
            // process has kill_on_drop set, so it will be cleaned up.
        })
        .await?;

    info!("kiwi-mail stopped");
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
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
