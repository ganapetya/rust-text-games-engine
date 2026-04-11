use async_trait::async_trait;
use shakti_game_domain::{
    GameConfig, GameDefinition, GameDefinitionId, GameKind, ScoringPolicy, TimingPolicy,
};
use shakti_game_engine_core::{AppError, GameDefinitionRepository};
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub struct PgGameDefinitionRepository {
    pool: PgPool,
}

impl PgGameDefinitionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub(crate) fn map_row(r: &sqlx::postgres::PgRow) -> Result<GameDefinition, AppError> {
        let id: Uuid = r
            .try_get("id")
            .map_err(|e| AppError::Repository(e.to_string()))?;
        let kind: String = r
            .try_get("kind")
            .map_err(|e| AppError::Repository(e.to_string()))?;
        let version: i32 = r
            .try_get("version")
            .map_err(|e| AppError::Repository(e.to_string()))?;
        let name: String = r
            .try_get("name")
            .map_err(|e| AppError::Repository(e.to_string()))?;
        let config: serde_json::Value = r
            .try_get("config")
            .map_err(|e| AppError::Repository(e.to_string()))?;
        let scoring_policy: serde_json::Value = r
            .try_get("scoring_policy")
            .map_err(|e| AppError::Repository(e.to_string()))?;
        let timing_policy: serde_json::Value = r
            .try_get("timing_policy")
            .map_err(|e| AppError::Repository(e.to_string()))?;

        let kind_enum = match kind.as_str() {
            "gap_fill" => GameKind::GapFill,
            "correct_usage" => GameKind::CorrectUsage,
            "crossword" => GameKind::Crossword,
            _ => return Err(AppError::Repository(format!("unknown game kind {kind}"))),
        };
        let config: GameConfig =
            serde_json::from_value(config).map_err(|e| AppError::Repository(e.to_string()))?;
        let scoring_policy: ScoringPolicy = serde_json::from_value(scoring_policy)
            .map_err(|e| AppError::Repository(e.to_string()))?;
        let timing_policy: TimingPolicy = serde_json::from_value(timing_policy)
            .map_err(|e| AppError::Repository(e.to_string()))?;
        Ok(GameDefinition {
            id: GameDefinitionId(id),
            kind: kind_enum,
            version,
            name,
            config,
            scoring_policy,
            timing_policy,
        })
    }
}

#[async_trait]
impl GameDefinitionRepository for PgGameDefinitionRepository {
    async fn get(&self, id: GameDefinitionId) -> Result<GameDefinition, AppError> {
        let row = sqlx::query(
            r#"SELECT id, kind, version, name, config, scoring_policy, timing_policy
               FROM game_definitions WHERE id = $1 AND active = true"#,
        )
        .bind(id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Repository(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("game definition".into()))?;

        Self::map_row(&row)
    }

    async fn get_default_gap_fill(&self) -> Result<GameDefinition, AppError> {
        let row = sqlx::query(
            r#"SELECT id, kind, version, name, config, scoring_policy, timing_policy
               FROM game_definitions WHERE kind = 'gap_fill' AND active = true
               ORDER BY version DESC LIMIT 1"#,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Repository(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("default gap_fill definition".into()))?;

        Self::map_row(&row)
    }

    async fn get_default_correct_usage(&self) -> Result<GameDefinition, AppError> {
        let row = sqlx::query(
            r#"SELECT id, kind, version, name, config, scoring_policy, timing_policy
               FROM game_definitions WHERE kind = 'correct_usage' AND active = true
               ORDER BY version DESC LIMIT 1"#,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Repository(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("default correct_usage definition".into()))?;

        Self::map_row(&row)
    }

    async fn get_default_crossword(&self) -> Result<GameDefinition, AppError> {
        let row = sqlx::query(
            r#"SELECT id, kind, version, name, config, scoring_policy, timing_policy
               FROM game_definitions WHERE kind = 'crossword' AND active = true
               ORDER BY version DESC LIMIT 1"#,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| AppError::Repository(e.to_string()))?
        .ok_or_else(|| AppError::NotFound("default crossword definition".into()))?;

        Self::map_row(&row)
    }
}
