//! Materializes a **Draft**: loads content + vocabulary, runs LLM, persists [`GameSession::base_context`] and steps, then starts play.

use crate::deps::EngineDeps;
use crate::errors::AppError;
use crate::ports::ContentRequest;
use crate::services::create_game_session::SessionOptions;
use serde::Deserialize;
use shakti_game_domain::{GameSessionId, GameSessionState, UserId};

#[derive(Deserialize)]
struct DeferredPayload {
    content_request: ContentRequest,
    /// Reserved for future use when materializing the session (timing is applied at create).
    #[serde(default)]
    _session_options: SessionOptions,
    trace_id: Option<String>,
}

pub async fn start_game_session(
    deps: &EngineDeps,
    session_id: GameSessionId,
    user_id: UserId,
) -> Result<shakti_game_domain::GameSession, AppError> {
    let mut session = deps.sessions.get(session_id).await?; // loads session, steps, definition join
    if session.user_id != user_id {
        return Err(AppError::Forbidden);
    }
    if session.state != GameSessionState::Draft {
        return Err(AppError::Conflict(format!(
            "session must be draft, got {:?}",
            session.state
        )));
    }

    let deferred_raw = session
        .deferred_payload
        .clone()
        .ok_or_else(|| AppError::Conflict("missing deferred_payload".into()))?;
    let deferred: DeferredPayload = serde_json::from_value(deferred_raw.clone())
        .map_err(|e| AppError::Repository(format!("deferred payload: {e}")))?;

    let lang = deferred
        .content_request
        .language
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("content_request.language required".into()))?
        .to_string();

    let items = deps
        .content
        .fetch_learning_items(user_id, deferred.content_request.clone())
        .await?; // history snippets (`learning_items`)

    if items.is_empty() {
        return Err(AppError::BadRequest(
            "no learning items for this user/language".into(),
        ));
    }

    let vocabulary: Vec<String> = match &deferred.content_request.llm_hard_words {
        Some(words) => {
            let w: Vec<String> = words
                .iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if w.is_empty() {
                return Err(AppError::BadRequest(
                    "llm_hard_words must contain at least one non-empty word when provided".into(),
                ));
            }
            w
        }
        None => deps
            .hard_words
            .fetch_registered(user_id, &lang)
            .await?, // user's registered vocabulary for the target language
    };
    if vocabulary.is_empty() {
        return Err(AppError::BadRequest(
            "no registered hard words for this language".into(),
        ));
    }

    let definition = session.definition().map_err(AppError::Domain)?.clone();
    let trace = deferred.trace_id.as_deref();

    let passage = deps
        .llm_preparer
        .build_passage_gap_context(
            user_id,
            trace,
            &items,
            &vocabulary,
            &lang,
            &definition,
        )
        .await?; // LLM or mock → `PassageGapLlmOutput`

    passage
        .validate()
        .map_err(|e| AppError::LlmPreparation(e.to_string()))?; // span/surface checks before persistence

    let engine = deps.engines.get(definition.kind)?; // `GapFillEngine`
    let mut prepared = engine.prepare_content(&items, &definition)?; // audit payload only; passage attached next
    prepared.passage = Some(passage.clone());

    let steps = engine.generate_steps(&prepared, &definition)?; // one multi-gap `GameStep`

    let mut base_context =
        serde_json::to_value(&passage).map_err(|e| AppError::Repository(e.to_string()))?;

    if deps.dev_expose_gap_solution {
        if let Some(obj) = base_context.as_object_mut() {
            let mut dev_llm = serde_json::json!({
                "language": deferred.content_request.language,
                "llmSourceTexts": deferred.content_request.llm_source_texts,
                "llmHardWords": deferred.content_request.llm_hard_words,
                "learningItemsCount": items.len(),
            });
            if let (Some(dev_obj), Some(ri)) = (
                dev_llm.as_object_mut(),
                deferred_raw
                    .get("content_package")
                    .and_then(|p| p.get("recapInputItems")),
            ) {
                dev_obj.insert("recapInputItems".into(), ri.clone());
            }
            obj.insert("_dev_llm_inputs".to_string(), dev_llm);
        }
    }

    session.steps = steps;
    session.base_context = base_context;
    session.deferred_payload = None;
    session.recompute_total_points().map_err(AppError::Domain)?; // total_points = f(gaps, scoring_mode)

    let now = deps.clock.now();
    session.start(now)?; // `Draft` + steps → `InProgress`, first step active + deadline

    // One transaction + advisory lock so concurrent POST /start cannot double-insert steps.
    let persisted = deps.sessions.persist_materialized_start(&session).await?;
    if !persisted {
        let s = deps.sessions.get(session_id).await?;
        if s.user_id != user_id {
            return Err(AppError::Forbidden);
        }
        return Ok(s);
    }

    deps.events
        .append(
            session_id,
            "session_started",
            serde_json::json!({
                "at": now,
                "passage_gaps": session.steps.first().map(|s| match &s.expected_answer {
                    shakti_game_domain::ExpectedAnswer::GapFillSlots { values } => values.len(),
                    _ => 0,
                }).unwrap_or(0),
            }),
        )
        .await?;

    Ok(session)
}
