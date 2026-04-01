use crate::deps::EngineDeps;
use crate::errors::AppError;
use crate::ports::ContentRequest;
use serde::{Deserialize, Serialize};
use shakti_game_domain::{GameKind, GameSession, GameSessionId, GameSessionState, UserId};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionOptions {
    #[serde(default)]
    pub step_time_limit_secs: Option<u32>,
    #[serde(default)]
    pub llm_preparation_enabled: bool,
}

pub struct CreateGameSessionCommand {
    pub user_id: UserId,
    pub game_kind: GameKind,
    pub definition_id: Option<shakti_game_domain::GameDefinitionId>,
    pub content_request: ContentRequest,
    pub options: SessionOptions,
}

pub async fn create_game_session(
    deps: &EngineDeps,
    cmd: CreateGameSessionCommand,
) -> Result<GameSession, AppError> {
    if cmd.game_kind != GameKind::GapFill {
        return Err(AppError::BadRequest("only gap_fill supported".into()));
    }

    let mut definition = if let Some(id) = cmd.definition_id {
        deps.definitions.get(id).await?
    } else {
        deps.definitions.get_default_gap_fill().await?
    };

    if let Some(secs) = cmd.options.step_time_limit_secs {
        definition.timing_policy.per_step_limit_secs = Some(secs);
    }

    let engine = deps.engines.get(definition.kind)?;
    let items = deps
        .content
        .fetch_learning_items(cmd.user_id, cmd.content_request.clone())
        .await?;

    let need = definition.config.steps_count;
    if items.len() < need {
        return Err(AppError::BadRequest(format!(
            "not enough learning items: need {}, have {}",
            need,
            items.len()
        )));
    }

    let prepared = engine.prepare_content(&items, &definition.config)?;
    let steps = engine.generate_steps(&prepared, &definition.config)?;
    if steps.is_empty() {
        return Err(AppError::BadRequest("no steps generated".into()));
    }

    let session = GameSession::new(
        GameSessionId(Uuid::new_v4()),
        cmd.user_id,
        definition.id,
        steps,
        definition,
    );

    deps.sessions.insert(&session).await?;
    deps.events
        .append(
            session.id,
            "session_created",
            serde_json::json!({
                "state": GameSessionState::Prepared,
                "steps_count": session.steps.len(),
            }),
        )
        .await?;

    Ok(session)
}
