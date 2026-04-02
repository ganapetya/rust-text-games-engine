use shakti_game_engine::config;
use shakti_game_engine::support;
use std::net::SocketAddr;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = config::Config::from_env()?;

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(fmt::layer().json().with_current_span(true))
        .with(filter)
        .init();

    let pool = support::connect_pool(&config.database_url).await?;
    support::run_migrations(&pool).await?;

    let llm_preparer = support::llm_preparer_from_config(&config)?;
    let state = support::build_app_state(pool, llm_preparer);
    let app = support::build_app_router(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], config.app_port));
    tracing::info!(%addr, "listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
