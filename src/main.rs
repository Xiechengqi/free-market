use anyhow::Context;
use clap::Parser;
use dujiao_rust::{app, cli, config, jobs, services, state};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = cli::Cli::parse();
    let should_serve = cli::dispatch(args).await?;
    if !should_serve {
        return Ok(());
    }
    serve().await
}

async fn serve() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "dujiao_rust=info,tower_http=info,axum::rejection=trace".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = config::AppConfig::load().context("load config")?;
    let state = state::AppState::build(config.clone()).await?;
    services::bootstrap::bootstrap(&state).await?;

    if config.server.run_worker {
        jobs::worker::spawn_worker(state.clone());
        services::evm_local_service::spawn_watcher(state.clone());
    }

    let router = app::router(state.clone());
    let addr = SocketAddr::from((config.server.host, config.server.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("dujiao-rust listening on http://{}", listener.local_addr()?);
    let shutdown_pool = state.pool.clone();
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    tracing::info!("shutdown signal received, draining db pool");
    shutdown_pool.close().await;
    tracing::info!("dujiao-rust stopped cleanly");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sig.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}
