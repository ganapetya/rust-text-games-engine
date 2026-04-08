use shakti_game_engine_core::EngineDeps;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub deps: Arc<EngineDeps>,
    pub pool: PgPool,
    /// When true, `StepPublic` may include `dev_gap_solution` (correct words per gap ordinal).
    pub dev_expose_gap_solution: bool,
    /// When set, `POST /game-sessions/bootstrap` accepts this key via `Authorization: Bearer` or `X-Shakti-Game-Service-Key`.
    pub service_api_key: Option<Arc<str>>,
}
