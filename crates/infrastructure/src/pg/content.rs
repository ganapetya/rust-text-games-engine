use async_trait::async_trait;
use shakti_game_application::{AppError, ContentProvider, ContentRequest};
use shakti_game_domain::{LearningItem, LearningItemId, UserId};
use sqlx::{PgPool, Row};

pub struct DbContentProvider {
    pool: PgPool,
}

impl DbContentProvider {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ContentProvider for DbContentProvider {
    async fn fetch_learning_items(
        &self,
        user_id: UserId,
        request: ContentRequest,
    ) -> Result<Vec<LearningItem>, AppError> {
        let limit = request.limit.max(1).min(100);

        let rows = match &request.language {
            Some(lang) => {
                sqlx::query(
                    r#"SELECT id, user_id, source_text, context_text, hard_fragment, lemma, language, metadata
                       FROM learning_items
                       WHERE user_id = $1 AND language = $2
                       ORDER BY created_at ASC, id ASC
                       LIMIT $3"#,
                )
                .bind(user_id.0)
                .bind(lang)
                .bind(limit)
                .fetch_all(&self.pool)
                .await
            }
            None => {
                sqlx::query(
                    r#"SELECT id, user_id, source_text, context_text, hard_fragment, lemma, language, metadata
                       FROM learning_items
                       WHERE user_id = $1
                       ORDER BY created_at ASC, id ASC
                       LIMIT $2"#,
                )
                .bind(user_id.0)
                .bind(limit)
                .fetch_all(&self.pool)
                .await
            }
        }
        .map_err(|e| AppError::Repository(e.to_string()))?;

        let mut out = Vec::new();
        for r in rows {
            out.push(LearningItem {
                id: LearningItemId(
                    r.try_get("id")
                        .map_err(|e| AppError::Repository(e.to_string()))?,
                ),
                user_id: UserId(
                    r.try_get("user_id")
                        .map_err(|e| AppError::Repository(e.to_string()))?,
                ),
                source_text: r
                    .try_get("source_text")
                    .map_err(|e| AppError::Repository(e.to_string()))?,
                context_text: r.try_get("context_text").ok(),
                hard_fragment: r
                    .try_get("hard_fragment")
                    .map_err(|e| AppError::Repository(e.to_string()))?,
                lemma: r.try_get("lemma").ok(),
                language: r
                    .try_get("language")
                    .map_err(|e| AppError::Repository(e.to_string()))?,
                metadata: r
                    .try_get("metadata")
                    .map_err(|e| AppError::Repository(e.to_string()))?,
            });
        }
        Ok(out)
    }
}
