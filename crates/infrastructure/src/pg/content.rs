use async_trait::async_trait;
use serde_json::json;
use shakti_game_domain::{LearningItem, LearningItemId, UserId};
use shakti_game_engine_core::{AppError, ContentProvider, ContentRequest};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// DoS guard only. Business cap comes from `game_definitions` (`max_learning_items_for_llm`), merged in `start_session` into `ContentRequest.limit`.
const CONTENT_FETCH_ABSOLUTE_MAX: i64 = 10_000;

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
        if let Some(ref texts) = request.llm_source_texts {
            let trimmed: Vec<String> = texts
                .iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !trimmed.is_empty() {
                let language = request
                    .language
                    .as_deref()
                    .filter(|s| !s.trim().is_empty())
                    .ok_or_else(|| {
                        AppError::BadRequest(
                            "content_request.language required when llm_source_texts is set".into(),
                        )
                    })?
                    .trim()
                    .to_string();
                return Ok(trimmed
                    .into_iter()
                    .map(|source_text| LearningItem {
                        id: LearningItemId(Uuid::new_v4()),
                        user_id,
                        source_text,
                        context_text: None,
                        hard_fragment: String::new(),
                        lemma: None,
                        language: language.clone(),
                        metadata: json!({"origin": "inline_ui"}),
                    })
                    .collect());
            }
        }

        let limit = request.limit.max(1).min(CONTENT_FETCH_ABSOLUTE_MAX);

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
