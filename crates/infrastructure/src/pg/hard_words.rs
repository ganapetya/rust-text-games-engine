use async_trait::async_trait;
use shakti_game_domain::UserId;
use shakti_game_engine_core::{AppError, HardWordsRepository};
use sqlx::PgPool;

pub struct PgHardWordsRepository {
    pool: PgPool,
}

impl PgHardWordsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl HardWordsRepository for PgHardWordsRepository {
    async fn fetch_registered(
        &self,
        user_id: UserId,
        language: &str,
    ) -> Result<Vec<String>, AppError> {
        let rows = sqlx::query_scalar::<_, String>(
            r#"SELECT word FROM user_hard_words
               WHERE user_id = $1 AND language = $2
               ORDER BY created_at ASC, word ASC"#,
        )
        .bind(user_id.0)
        .bind(language)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Repository(e.to_string()))?;
        Ok(rows)
    }
}
