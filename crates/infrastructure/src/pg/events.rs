use async_trait::async_trait;
use shakti_game_domain::GameSessionId;
use shakti_game_engine_core::{AppError, SessionEventRepository};
use sqlx::PgPool;
use uuid::Uuid;

pub struct PgSessionEventRepository {
    pool: PgPool,
}

impl PgSessionEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SessionEventRepository for PgSessionEventRepository {
    async fn append(
        &self,
        session_id: GameSessionId,
        event_type: &str,
        payload: serde_json::Value,
    ) -> Result<(), AppError> {
        sqlx::query(
            r#"INSERT INTO session_events (id, session_id, event_type, payload)
               VALUES ($1, $2, $3, $4)"#,
        )
        .bind(Uuid::new_v4())
        .bind(session_id.0)
        .bind(event_type)
        .bind(payload)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Repository(e.to_string()))?;
        Ok(())
    }
}
