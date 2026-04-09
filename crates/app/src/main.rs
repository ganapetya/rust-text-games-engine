use shakti_game_engine::config;
use shakti_game_engine::support;
use std::net::SocketAddr;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Log target host/port/db without credentials (everything after the last `@` in the authority).
fn redacted_db_target(database_url: &str) -> String {
    let rest = database_url
        .split("://")
        .nth(1)
        .unwrap_or("(missing scheme)");
    rest.rsplit_once('@').map(|(_, host_db)| host_db).unwrap_or(rest).to_string()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = config::Config::from_env()?;

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(fmt::layer().json().with_current_span(true))
        .with(filter)
        .init();

    tracing::info!(
        llm_mode = ?config.llm_mode,
        openai_model = %config.openai_model,
        openai_key_source = config.openai_key_source.as_str(),
        shakti_actors_openai_key_url_configured = config.shakti_actors_internal_url.is_some(),
        dev_expose_gap_solution = config.dev_expose_gap_solution,
        game_bootstrap_configured = config.service_api_key.is_some(),
        "game engine config loaded"
    );

    let pool = match support::connect_pool(&config.database_url).await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(
                error = %e,
                database_url_host = %redacted_db_target(&config.database_url),
                "database pool failed (e.g. PoolTimedOut = could not reach Postgres in time)"
            );
            tracing::error!(
                hint = "from shakti-game-engine: docker compose up -d shakti-game-db  (Postgres on 127.0.0.1:5435 per env.example)"
            );
            return Err(e.into());
        }
    };
    support::run_migrations(&pool).await?;

    let (llm_preparer, llm_translator) = support::llm_stack_from_config(&config)?;
    let state = support::build_app_state(
        pool,
        llm_preparer,
        llm_translator,
        config.dev_expose_gap_solution,
        config.service_api_key.clone(),
    );
    let app = support::build_app_router(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], config.app_port));
    tracing::info!(%addr, "listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
