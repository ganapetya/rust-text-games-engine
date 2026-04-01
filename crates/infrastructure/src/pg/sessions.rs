use crate::pg::mapping::{
    session_state_from_db, session_state_to_db, step_from_json, step_state_to_db,
};
use async_trait::async_trait;
use shakti_game_application::{AppError, GameSessionRepository};
use shakti_game_domain::{GameSession, GameSessionId, Score};
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct PgGameSessionRepository {
    pool: PgPool,
}

impl PgGameSessionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl GameSessionRepository for PgGameSessionRepository {
    async fn insert(&self, session: &GameSession) -> Result<(), AppError> {
        let score_json = serde_json::to_value(&session.score)
            .map_err(|e| AppError::Repository(e.to_string()))?;

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::Repository(e.to_string()))?;

        sqlx::query(
            r#"INSERT INTO game_sessions (
                id, user_id, definition_id, state, current_step_index, score,
                started_at, completed_at, expires_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
        )
        .bind(session.id.0)
        .bind(session.user_id.0)
        .bind(session.definition_id.0)
        .bind(session_state_to_db(session.state))
        .bind(session.current_step_index as i32)
        .bind(score_json)
        .bind(session.started_at)
        .bind(session.completed_at)
        .bind(session.expires_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Repository(e.to_string()))?;

        for step in &session.steps {
            let prompt = serde_json::to_value(&step.prompt)
                .map_err(|e| AppError::Repository(e.to_string()))?;
            let expected = serde_json::to_value(&step.expected_answer)
                .map_err(|e| AppError::Repository(e.to_string()))?;
            let ua = step
                .user_answer
                .as_ref()
                .map(|a| serde_json::to_value(a))
                .transpose()
                .map_err(|e| AppError::Repository(e.to_string()))?;
            let ev = step
                .evaluation
                .as_ref()
                .map(|a| serde_json::to_value(a))
                .transpose()
                .map_err(|e| AppError::Repository(e.to_string()))?;

            sqlx::query(
                r#"INSERT INTO game_steps (
                    id, session_id, ordinal, state, prompt, expected_answer,
                    user_answer, evaluation, deadline_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
            )
            .bind(step.id.0)
            .bind(session.id.0)
            .bind(step.ordinal as i32)
            .bind(step_state_to_db(step.state))
            .bind(prompt)
            .bind(expected)
            .bind(ua)
            .bind(ev)
            .bind(step.deadline_at)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Repository(e.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|e| AppError::Repository(e.to_string()))?;
        Ok(())
    }

    async fn get(&self, id: GameSessionId) -> Result<GameSession, AppError> {
        let row = sqlx::query(
            r#"SELECT id, user_id, definition_id, state, current_step_index, score,
                      started_at, completed_at, expires_at
               FROM game_sessions WHERE id = $1"#,
        )
        .bind(id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Repository(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("game session".into()))?;

        let sid: Uuid = row
            .try_get("id")
            .map_err(|e| AppError::Repository(e.to_string()))?;
        let user_id: Uuid = row
            .try_get("user_id")
            .map_err(|e| AppError::Repository(e.to_string()))?;
        let definition_id: Uuid = row
            .try_get("definition_id")
            .map_err(|e| AppError::Repository(e.to_string()))?;
        let state: String = row
            .try_get("state")
            .map_err(|e| AppError::Repository(e.to_string()))?;
        let current_step_index: i32 = row
            .try_get("current_step_index")
            .map_err(|e| AppError::Repository(e.to_string()))?;
        let score_val: serde_json::Value = row
            .try_get("score")
            .map_err(|e| AppError::Repository(e.to_string()))?;
        let score: Score =
            serde_json::from_value(score_val).map_err(|e| AppError::Repository(e.to_string()))?;
        let started_at: Option<time::OffsetDateTime> = row.try_get("started_at").ok();
        let completed_at: Option<time::OffsetDateTime> = row.try_get("completed_at").ok();
        let expires_at: Option<time::OffsetDateTime> = row.try_get("expires_at").ok();

        let step_rows = sqlx::query(
            r#"SELECT id, ordinal, state, prompt, expected_answer, user_answer, evaluation, deadline_at
               FROM game_steps WHERE session_id = $1 ORDER BY ordinal ASC"#,
        )
        .bind(sid)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Repository(e.to_string()))?;

        let mut steps = Vec::new();
        for sr in step_rows {
            let id: Uuid = sr
                .try_get("id")
                .map_err(|e| AppError::Repository(e.to_string()))?;
            let ord: i32 = sr
                .try_get("ordinal")
                .map_err(|e| AppError::Repository(e.to_string()))?;
            let st: String = sr
                .try_get("state")
                .map_err(|e| AppError::Repository(e.to_string()))?;
            let prompt: serde_json::Value = sr
                .try_get("prompt")
                .map_err(|e| AppError::Repository(e.to_string()))?;
            let expected: serde_json::Value = sr
                .try_get("expected_answer")
                .map_err(|e| AppError::Repository(e.to_string()))?;
            let ua: Option<serde_json::Value> = sr.try_get("user_answer").ok();
            let ev: Option<serde_json::Value> = sr.try_get("evaluation").ok();
            let dl: Option<time::OffsetDateTime> = sr.try_get("deadline_at").ok();
            steps.push(step_from_json(id, ord, &st, prompt, expected, ua, ev, dl)?);
        }

        let def_row = sqlx::query(
            r#"SELECT id, kind, version, name, config, scoring_policy, timing_policy
               FROM game_definitions WHERE id = $1"#,
        )
        .bind(definition_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Repository(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("definition for session".into()))?;

        let definition = super::definitions::PgGameDefinitionRepository::map_row(&def_row)?;

        Ok(GameSession {
            id: GameSessionId(sid),
            user_id: shakti_game_domain::UserId(user_id),
            definition_id: shakti_game_domain::GameDefinitionId(definition_id),
            state: session_state_from_db(&state)?,
            steps,
            current_step_index: current_step_index as usize,
            score,
            started_at,
            completed_at,
            expires_at,
            definition: Some(definition),
        })
    }

    async fn update(&self, session: &GameSession) -> Result<(), AppError> {
        let score_json = serde_json::to_value(&session.score)
            .map_err(|e| AppError::Repository(e.to_string()))?;

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| AppError::Repository(e.to_string()))?;

        sqlx::query(
            r#"UPDATE game_sessions SET
                state = $2,
                current_step_index = $3,
                score = $4,
                started_at = $5,
                completed_at = $6,
                expires_at = $7,
                updated_at = now()
               WHERE id = $1"#,
        )
        .bind(session.id.0)
        .bind(session_state_to_db(session.state))
        .bind(session.current_step_index as i32)
        .bind(score_json)
        .bind(session.started_at)
        .bind(session.completed_at)
        .bind(session.expires_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::Repository(e.to_string()))?;

        for step in &session.steps {
            let prompt = serde_json::to_value(&step.prompt)
                .map_err(|e| AppError::Repository(e.to_string()))?;
            let expected = serde_json::to_value(&step.expected_answer)
                .map_err(|e| AppError::Repository(e.to_string()))?;
            let ua = step
                .user_answer
                .as_ref()
                .map(|a| serde_json::to_value(a))
                .transpose()
                .map_err(|e| AppError::Repository(e.to_string()))?;
            let ev = step
                .evaluation
                .as_ref()
                .map(|a| serde_json::to_value(a))
                .transpose()
                .map_err(|e| AppError::Repository(e.to_string()))?;

            sqlx::query(
                r#"UPDATE game_steps SET
                    state = $2,
                    prompt = $3,
                    expected_answer = $4,
                    user_answer = $5,
                    evaluation = $6,
                    deadline_at = $7,
                    updated_at = now()
                   WHERE id = $1"#,
            )
            .bind(step.id.0)
            .bind(step_state_to_db(step.state))
            .bind(prompt)
            .bind(expected)
            .bind(ua)
            .bind(ev)
            .bind(step.deadline_at)
            .execute(&mut *tx)
            .await
            .map_err(|e| AppError::Repository(e.to_string()))?;
        }

        tx.commit()
            .await
            .map_err(|e| AppError::Repository(e.to_string()))?;
        Ok(())
    }
}
