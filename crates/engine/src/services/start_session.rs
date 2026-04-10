//! Materializes a **Draft**: loads content + vocabulary, runs LLM, persists [`GameSession::base_context`] and steps, then starts play.

use crate::billing::{wallet_from_deferred, write_wallet_to_base};
use crate::deps::EngineDeps;
use crate::errors::AppError;
use crate::ports::{ContentRequest, GameLlmChargeArgs};
use crate::services::create_game_session::SessionOptions;
use crate::services::translation_hint::normalize_hint_translation_languages;
use serde::Deserialize;
use shakti_game_domain::{GameSessionId, GameSessionState, UserId};
use shakti_game_pricing::coins_for_usage;

#[derive(Deserialize)]
struct DeferredPayload {
    content_request: ContentRequest,
    #[serde(default)]
    session_options: SessionOptions,
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

    let wallet_opt = wallet_from_deferred(&deferred_raw);
    if deps.require_billing_for_llm && wallet_opt.is_none() {
        return Err(AppError::BadRequest(
            "billing metadata required for LLM (shaktiUserId and billingRates)".into(),
        ));
    }

    let lang = deferred
        .content_request
        .language
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("content_request.language required".into()))?
        .to_string();

    let definition = session.definition().map_err(AppError::Domain)?.clone();
    let gap_cfg = definition.gap_fill_config().map_err(AppError::Domain)?;

    let mut content_request = deferred.content_request.clone();
    let cap = gap_cfg.max_learning_items_for_llm.max(1) as i64;
    content_request.limit = content_request.limit.max(1).min(cap);

    let items = deps
        .content
        .fetch_learning_items(user_id, content_request)
        .await?; // history snippets (`learning_items`) or inline synthetic items

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

    let trace = deferred.trace_id.as_deref();

    let (passage, usage) = deps
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
        .validate_against_gap_fill_config(gap_cfg)
        .map_err(|e| AppError::LlmPreparation(e.to_string()))?; // spans, word count, gap cap

    let engine = deps.engines.get(definition.kind)?; // `GapFillEngine`
    let mut prepared = engine.prepare_content(&items, &definition)?; // audit payload only; passage attached next
    prepared.passage = Some(passage.clone());

    let steps = engine.generate_steps(&prepared, &definition)?; // one multi-gap `GameStep`

    let mut base_context =
        serde_json::to_value(&passage).map_err(|e| AppError::Repository(e.to_string()))?;

    let hint_langs =
        normalize_hint_translation_languages(deferred.session_options.hint_translation_languages.as_ref());
    if let Some(obj) = base_context.as_object_mut() {
        obj.insert(
            "_session".to_string(),
            serde_json::json!({
                "source_language": lang,
                "hint_translation_languages": hint_langs,
                "translation_cache": serde_json::json!({}),
            }),
        );
    }

    if let Some(ref w) = wallet_opt {
        write_wallet_to_base(&mut base_context, w)
            .map_err(|e| AppError::Repository(format!("wallet in base_context: {e}")))?;
    }

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

    if let (Some(ref sched), Some(ref wallet)) = (&deps.billing_scheduler, &wallet_opt) {
        let coins = coins_for_usage(
            usage.prompt_tokens,
            usage.completion_tokens,
            wallet.billing_rates.prepare.input_per_1k,
            wallet.billing_rates.prepare.output_per_1k,
        );
        if coins > 0 {
            let trace_owned = trace.unwrap_or("").to_string();
            sched.schedule_game_llm_charge(GameLlmChargeArgs {
                session_id,
                shakti_user_id: wallet.shakti_user_id,
                trace_id: trace_owned,
                variant: wallet.billing_rates.variant.clone(),
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                coins,
                endpoint: "/game/prepare",
            });
        }
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
