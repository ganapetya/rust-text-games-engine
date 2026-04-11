//! New playable round from the same stored LLM payload (no second LLM call). Deletes old steps and inserts fresh ones.

use crate::deps::EngineDeps;
use crate::errors::AppError;
use crate::services::translation_hint::read_session_ui_hints;
use shakti_game_domain::{
    CorrectUsageLlmOutput, CrosswordLlmOutput, GameKind, GameSession, GameSessionId,
    GameSessionState, LearningItem, PassageGapLlmOutput, UserId,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub async fn play_again(
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

    let definition = session.definition().map_err(AppError::Domain)?.clone();
    let engine = deps.engines.get(definition.kind)?;

    let items: Vec<LearningItem> = Vec::new();
    let mut prepared = engine
        .prepare_content(&items, &definition)
        .map_err(AppError::Domain)?;

    match definition.kind {
        GameKind::GapFill => {
            let gap_cfg = definition.gap_fill_config().map_err(AppError::Domain)?;
            let passage: PassageGapLlmOutput =
                serde_json::from_value(session.base_context.clone()).map_err(|e| {
                    AppError::Repository(format!("stored passage (base_context): {e}"))
                })?;
            passage
                .validate_against_gap_fill_config(gap_cfg)
                .map_err(|e| AppError::LlmPreparation(e.to_string()))?;
            prepared.passage = Some(passage);
        }
        GameKind::CorrectUsage => {
            let batch: CorrectUsageLlmOutput =
                serde_json::from_value(session.base_context.clone()).map_err(|e| {
                    AppError::Repository(format!("stored correct_usage batch (base_context): {e}"))
                })?;
            let cfg = definition.correct_usage_config().map_err(AppError::Domain)?;
            let words: Vec<String> = batch.puzzles.iter().map(|p| p.word.clone()).collect();
            batch
                .validate(&words, cfg.max_sentence_words)
                .map_err(AppError::from)?;
            prepared.correct_usage_batch = Some(batch);
        }
        GameKind::Crossword => {
            let cw_cfg = definition.crossword_config().map_err(AppError::Domain)?;
            let cw: CrosswordLlmOutput =
                serde_json::from_value(session.base_context.clone()).map_err(|e| {
                    AppError::Repository(format!("stored crossword (base_context): {e}"))
                })?;
            cw.validate_against_crossword_config(cw_cfg)
                .map_err(|e| AppError::LlmPreparation(e.to_string()))?;
            prepared.crossword = Some(cw);
        }
    }

    let mut h_seed = DefaultHasher::new();
    session.id.hash(&mut h_seed);
    prepared.session_seed = Some(h_seed.finish());
    let (source_lang, _) = read_session_ui_hints(&session.base_context);
    prepared.crossword_ui_language = source_lang;
    let diff = session
        .base_context
        .get("_session")
        .and_then(|s| s.get("crossword_difficulty"))
        .and_then(|v| v.as_u64())
        .map(|u| u as u8);
    prepared.crossword_difficulty = diff;

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
            "session_play_again",
            serde_json::json!({ "at": now, "kind": definition.kind }),
        )
        .await?;

    Ok(session)
}
