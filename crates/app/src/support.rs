//! Shared wiring: database pool, migrations, [`ApplicationDeps`], [`AppState`], and HTTP router.
//! Integration tests and the binary both use this so scenarios exercise the same stack as production.

use shakti_game_api::{build_router, AppState};
use shakti_game_application::ApplicationDeps;
use shakti_game_domain::{GameEngineRegistry, GapFillEngine};
use shakti_game_infrastructure::{
    DbContentProvider, PgGameDefinitionRepository, PgGameSessionRepository,
    PgSessionEventRepository, SystemClock,
};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

pub async fn connect_pool(database_url: &str) -> Result<sqlx::PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}

pub async fn run_migrations(pool: &sqlx::PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("../../migrations").run(pool).await
}

pub fn build_application_deps(pool: sqlx::PgPool) -> Arc<ApplicationDeps> {
    let mut engines = GameEngineRegistry::new();
    engines.register(Arc::new(GapFillEngine::new()));
    let engines = Arc::new(engines);

    Arc::new(ApplicationDeps {
        sessions: Arc::new(PgGameSessionRepository::new(pool.clone())),
        definitions: Arc::new(PgGameDefinitionRepository::new(pool.clone())),
        content: Arc::new(DbContentProvider::new(pool.clone())),
        events: Arc::new(PgSessionEventRepository::new(pool.clone())),
        clock: Arc::new(SystemClock),
        engines,
    })
}

pub fn build_app_state(pool: sqlx::PgPool) -> AppState {
    let deps = build_application_deps(pool.clone());
    AppState { deps, pool }
}

pub fn build_app_router(state: AppState) -> axum::Router {
    build_router(state)
}
