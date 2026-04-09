//! New playable round from the same stored passage (no LLM). Deletes old steps and inserts fresh ones.

use crate::deps::EngineDeps;
use crate::errors::AppError;
use shakti_game_domain::{
    GameSession, GameSessionId, GameSessionState, LearningItem, PassageGapLlmOutput, UserId,
};

pub async fn play_again_gap_fill(
    deps: &EngineDeps,
    session_id: GameSessionId,
    user_id: UserId,
) -> Result<GameSession, AppError> {
    let mut session = deps.sessions.get(session_id).await?;
    if session.user_id != user_id {
        return Err(AppError::Forbidden);
    }

    match session.state {
        GameSessionState::InProgress | GameSessionState::Completed | GameSessionState::TimedOut => {}
        _ => {
            return Err(AppError::Conflict(format!(
                "play again is only for an active or finished round (got {:?})",
                session.state
            )));
        }
    }

    let passage: PassageGapLlmOutput = serde_json::from_value(session.base_context.clone())
        .map_err(|e| AppError::Repository(format!("stored passage (base_context): {e}")))?;

    let definition = session.definition().map_err(AppError::Domain)?.clone();
    let gap_cfg = definition.gap_fill_config().map_err(AppError::Domain)?;
    passage
        .validate_against_gap_fill_config(gap_cfg)
        .map_err(|e| AppError::LlmPreparation(e.to_string()))?;
    let engine = deps.engines.get(definition.kind)?;

    let items: Vec<LearningItem> = Vec::new();
    let mut prepared = engine
        .prepare_content(&items, &definition)
        .map_err(AppError::Domain)?;
    prepared.passage = Some(passage);

    let steps = engine
        .generate_steps(&prepared, &definition)
        .map_err(AppError::Domain)?;

    deps.sessions.delete_steps(session_id).await?;
    deps.sessions.insert_steps(session_id, &steps).await?;

    session.steps = steps;
    session.current_step_index = 0;
    session.score.earned_points = 0;
    session.score.correct_count = 0;
    session.score.answered_count = 0;
    session
        .recompute_total_points()
        .map_err(AppError::Domain)?;
    session.completed_at = None;
    session.state = GameSessionState::Prepared;

    let now = deps.clock.now();
    session.start(now).map_err(AppError::Domain)?;

    deps.sessions.update(&session).await?;

    deps.events
        .append(
            session_id,
            "gap_fill_play_again",
            serde_json::json!({ "at": now }),
        )
        .await?;

    Ok(session)
}
