use shakti_game_infrastructure::ActorsGameBillingClient;
use shakti_game_engine_core::EngineDeps;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub deps: Arc<EngineDeps>,
    pub pool: PgPool,
    /// When true, `StepPublic` may include `dev_gap_solution` (correct words per gap ordinal).
    pub dev_expose_gap_solution: bool,
    /// When set, `POST /game-sessions/bootstrap` accepts this key via `Authorization: Bearer` or `X-Shakti-Game-Service-Key`.
    pub service_api_key: Option<Arc<str>>,
    pub billing_client: Option<ActorsGameBillingClient>,
    /// Short TTL cache: session UUID → (fetched at, balance).
    pub balance_cache: Arc<Mutex<HashMap<Uuid, (Instant, i64)>>>,
}
