use shakti_game_application::ApplicationDeps;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub deps: Arc<ApplicationDeps>,
    pub pool: PgPool,
}
