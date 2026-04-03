use shakti_game_engine_core::EngineDeps;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub deps: Arc<EngineDeps>,
    pub pool: PgPool,
    /// When true, `StepPublic` may include `dev_gap_solution` (correct words per gap ordinal).
    pub dev_expose_gap_solution: bool,
}
