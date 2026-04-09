//! Shared wiring: database pool, migrations, [`EngineDeps`], [`AppState`], and HTTP router.
//! Integration tests and the binary both use this so scenarios exercise the same stack as production.

use shakti_game_api::{build_router, AppState};
use shakti_game_domain::{GameEngineRegistry, GapFillEngine};
use shakti_game_engine_core::{EngineDeps, LlmContentPreparer};
use shakti_game_infrastructure::{
    build_llm_stack, DbContentProvider, PgGameDefinitionRepository, PgGameSessionRepository,
    PgHardWordsRepository, PgSessionEventRepository, SystemClock,
};
use shakti_game_translation::LlmTextTranslator;
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

pub fn build_engine_deps(
    pool: sqlx::PgPool,
    llm_preparer: Arc<dyn LlmContentPreparer>,
    llm_translator: Arc<dyn LlmTextTranslator>,
    dev_expose_gap_solution: bool,
) -> Arc<EngineDeps> {
    let mut engines = GameEngineRegistry::new();
    engines.register(Arc::new(GapFillEngine::new()));
    let engines = Arc::new(engines);

    Arc::new(EngineDeps {
        sessions: Arc::new(PgGameSessionRepository::new(pool.clone())),
        definitions: Arc::new(PgGameDefinitionRepository::new(pool.clone())),
        content: Arc::new(DbContentProvider::new(pool.clone())),
        hard_words: Arc::new(PgHardWordsRepository::new(pool.clone())),
        events: Arc::new(PgSessionEventRepository::new(pool.clone())),
        clock: Arc::new(SystemClock),
        engines,
        llm_preparer,
        llm_translator,
        dev_expose_gap_solution,
    })
}

pub fn build_app_state(
    pool: sqlx::PgPool,
    llm_preparer: Arc<dyn LlmContentPreparer>,
    llm_translator: Arc<dyn LlmTextTranslator>,
    dev_expose_gap_solution: bool,
    service_api_key: Option<String>,
) -> AppState {
    let deps = build_engine_deps(
        pool.clone(),
        llm_preparer,
        llm_translator,
        dev_expose_gap_solution,
    );
    AppState {
        deps,
        pool,
        dev_expose_gap_solution,
        service_api_key: service_api_key.map(|s| s.into_boxed_str().into()),
    }
}

/// Wires [`build_llm_stack`] from process environment (see [`crate::config::Config`]).
pub fn llm_stack_from_config(
    config: &crate::config::Config,
) -> Result<(Arc<dyn LlmContentPreparer>, Arc<dyn LlmTextTranslator>), String> {
    build_llm_stack(
        config.llm_mode,
        config.openai_api_key.clone(),
        config.openai_model.clone(),
        config.shakti_actors_internal_url.clone(),
        config.shakti_actors_openai_key_name.clone(),
        config.shakti_actors_openai_consumer_service.clone(),
    )
}

pub fn build_app_router(state: AppState) -> axum::Router {
    build_router(state)
}
