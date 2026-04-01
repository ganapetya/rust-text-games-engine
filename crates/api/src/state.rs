use shakti_game_engine_core::EngineDeps;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub deps: Arc<EngineDeps>,
    pub pool: PgPool,
}
