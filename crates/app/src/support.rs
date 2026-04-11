//! Shared wiring: database pool, migrations, [`EngineDeps`], [`AppState`], and HTTP router.
//! Integration tests and the binary both use this so scenarios exercise the same stack as production.

use shakti_game_api::{build_router, AppState};
use shakti_game_domain::{CorrectUsageEngine, CrosswordEngine, GameEngineRegistry, GapFillEngine};
use shakti_game_engine_core::ports::BillingChargeScheduler;
use shakti_game_engine_core::{EngineDeps, LlmContentPreparer};
use shakti_game_infrastructure::{
    build_llm_stack, ActorsGameBillingClient, DbContentProvider, PgBillingChargeScheduler,
    PgGameDefinitionRepository, PgGameSessionRepository, PgHardWordsRepository,
    PgSessionEventRepository, SystemClock,
};
use shakti_game_translation::LlmTextTranslator;
use sqlx::postgres::PgPoolOptions;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub async fn connect_pool(database_url: &str) -> Result<sqlx::PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}

pub async fn run_migrations(pool: &sqlx::PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("../../migrations").run(pool).await
}

pub fn build_app_state(
    pool: sqlx::PgPool,
    llm_preparer: Arc<dyn LlmContentPreparer>,
    llm_translator: Arc<dyn LlmTextTranslator>,
    dev_expose_gap_solution: bool,
    service_api_key: Option<String>,
    shakti_actors_internal_url: Option<String>,
    require_billing_for_llm: bool,
) -> AppState {
    let sessions: Arc<PgGameSessionRepository> =
        Arc::new(PgGameSessionRepository::new(pool.clone()));
    let sessions_trait: Arc<dyn shakti_game_engine_core::ports::GameSessionRepository> =
        sessions.clone();

    let billing_client: Option<ActorsGameBillingClient> = match (
        shakti_actors_internal_url.as_deref(),
        service_api_key.as_deref(),
    ) {
        (Some(base), Some(key)) => match ActorsGameBillingClient::new(base, key) {
            Ok(c) => Some(c),
            Err(e) => {
                tracing::warn!(error = %e, "ActorsGameBillingClient not configured; game LLM debits disabled");
                None
            }
        },
        _ => None,
    };

    let billing_scheduler: Option<Arc<dyn BillingChargeScheduler>> =
        billing_client.as_ref().map(|c| {
            Arc::new(PgBillingChargeScheduler::new(
                c.clone(),
                sessions_trait.clone(),
            )) as Arc<dyn BillingChargeScheduler>
        });

    let mut engines = GameEngineRegistry::new();
    engines.register(Arc::new(GapFillEngine::new()));
    engines.register(Arc::new(CorrectUsageEngine::new()));
    engines.register(Arc::new(CrosswordEngine::new()));
    let engines = Arc::new(engines);

    let deps = Arc::new(EngineDeps {
        sessions: sessions_trait,
        definitions: Arc::new(PgGameDefinitionRepository::new(pool.clone())),
        content: Arc::new(DbContentProvider::new(pool.clone())),
        hard_words: Arc::new(PgHardWordsRepository::new(pool.clone())),
        events: Arc::new(PgSessionEventRepository::new(pool.clone())),
        clock: Arc::new(SystemClock),
        engines,
        llm_preparer,
        llm_translator,
        dev_expose_gap_solution,
        require_billing_for_llm,
        billing_scheduler,
    });

    AppState {
        deps,
        pool,
        dev_expose_gap_solution,
        service_api_key: service_api_key.map(|s| s.into_boxed_str().into()),
        billing_client,
        balance_cache: Arc::new(Mutex::new(HashMap::new())),
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
