//! Materializes a **Draft**: loads content + vocabulary, runs LLM, persists [`GameSession::base_context`] and steps, then starts play.

use crate::billing::{wallet_from_deferred, write_wallet_to_base};
use crate::deps::EngineDeps;
use crate::errors::AppError;
use crate::ports::{ContentRequest, GameLlmChargeArgs};
use crate::services::create_game_session::SessionOptions;
use crate::services::translation_hint::normalize_hint_translation_languages;
use serde::Deserialize;
use shakti_game_domain::{build_crossword, GameKind, GameSessionId, GameSessionState, UserId, WordCandidate};
use shakti_game_pricing::coins_for_usage;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Deserialize)]
struct DeferredPayload {
    content_request: ContentRequest,
    #[serde(default)]
    session_options: SessionOptions,
    trace_id: Option<String>,
}

/// `override_crossword_difficulty`: override the difficulty stored at bootstrap time (crossword only).
pub async fn start_game_session(
    deps: &EngineDeps,
    session_id: GameSessionId,
    user_id: UserId,
    override_crossword_difficulty: Option<u8>,
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

    let mut definition = session.definition().map_err(AppError::Domain)?.clone();
    if let Ok(cw) = definition.crossword_config() {
        if cw.is_time_game && cw.game_time_seconds > 0 {
            definition.timing_policy.session_limit_secs = Some(cw.game_time_seconds as u32);
        }
    }
    session.definition = Some(definition.clone());

    let mut content_request = deferred.content_request.clone();
    let cap_llm_items = match definition.kind {
        GameKind::GapFill => definition
            .gap_fill_config()
            .map_err(AppError::Domain)?
            .max_learning_items_for_llm,
        GameKind::CorrectUsage => definition
            .correct_usage_config()
            .map_err(AppError::Domain)?
            .max_learning_items_for_llm,
        GameKind::Crossword => definition
            .crossword_config()
            .map_err(AppError::Domain)?
            .max_learning_items_for_llm,
    };
    let cap = cap_llm_items.max(1) as i64;
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

    let engine = deps.engines.get(definition.kind)?;
    let mut prepared = engine.prepare_content(&items, &definition)?;
    let mut h_seed = DefaultHasher::new();
    session.id.hash(&mut h_seed);
    prepared.session_seed = Some(h_seed.finish());
    prepared.crossword_ui_language = Some(lang.clone());
    prepared.crossword_difficulty = override_crossword_difficulty
        .or(deferred.session_options.crossword_difficulty);

    let (steps, mut base_context, usage) = match definition.kind {
        GameKind::GapFill => {
            let gap_cfg = definition.gap_fill_config().map_err(AppError::Domain)?;
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
                .await?;
            passage
                .validate_against_gap_fill_config(gap_cfg)
                .map_err(|e| AppError::LlmPreparation(e.to_string()))?;
            prepared.passage = Some(passage.clone());
            let steps = engine.generate_steps(&prepared, &definition)?;
            let base =
                serde_json::to_value(&passage).map_err(|e| AppError::Repository(e.to_string()))?;
            (steps, base, usage)
        }
        GameKind::CorrectUsage => {
            let cu_cfg = definition.correct_usage_config().map_err(AppError::Domain)?;
            let (batch, usage) = deps
                .llm_preparer
                .build_correct_usage_context(
                    user_id,
                    trace,
                    &items,
                    &vocabulary,
                    &lang,
                    &definition,
                )
                .await?;
            batch
                .validate(&vocabulary, cu_cfg.max_sentence_words)
                .map_err(AppError::from)?;
            prepared.correct_usage_batch = Some(batch.clone());
            let steps = engine.generate_steps(&prepared, &definition)?;
            let base =
                serde_json::to_value(&batch).map_err(|e| AppError::Repository(e.to_string()))?;
            (steps, base, usage)
        }
        GameKind::Crossword => {
            let cw_cfg = definition.crossword_config().map_err(AppError::Domain)?;

            // Step 1: ask the LLM for story + clues + bridge words only (no grid).
            let (hints, usage) = deps
                .llm_preparer
                .build_crossword_hints(
                    user_id,
                    trace,
                    &items,
                    &vocabulary,
                    &lang,
                    &definition,
                )
                .await?;

            // Step 2: assemble WordCandidates — hard words first, then bridge words.
            //         For hard words that the LLM didn't provide a hint for, use a fallback.
            let hint_map: std::collections::HashMap<String, String> = hints
                .hard_word_hints
                .iter()
                .map(|h| (h.word.trim().to_uppercase(), h.hint.clone()))
                .collect();

            let mut candidates: Vec<WordCandidate> = vocabulary
                .iter()
                .map(|w| {
                    let upper = w.trim().to_uppercase();
                    let hint = hint_map
                        .get(&upper)
                        .cloned()
                        .unwrap_or_else(|| format!("Practice word: {upper}"));
                    WordCandidate { word: upper, hint, is_hard: true }
                })
                .collect();

            for bw in &hints.bridge_words {
                candidates.push(WordCandidate {
                    word: bw.word.trim().to_uppercase(),
                    hint: bw.hint.clone(),
                    is_hard: false,
                });
            }

            // Step 3: run the deterministic placement algorithm.
            let cw = build_crossword(&candidates, hints.story, cw_cfg)
                .map_err(AppError::LlmPreparation)?;

            cw.validate_against_crossword_config(cw_cfg)
                .map_err(|e| AppError::LlmPreparation(e.to_string()))?;
            prepared.crossword = Some(cw.clone());
            let steps = engine.generate_steps(&prepared, &definition)?;
            let base =
                serde_json::to_value(&cw).map_err(|e| AppError::Repository(e.to_string()))?;
            (steps, base, usage)
        }
    };

    let hint_langs =
        normalize_hint_translation_languages(deferred.session_options.hint_translation_languages.as_ref());
    let mut session_meta = serde_json::json!({
        "source_language": lang,
        "hint_translation_languages": hint_langs,
        "translation_cache": serde_json::json!({}),
    });
    if definition.kind == GameKind::Crossword {
        if let Ok(cw) = definition.crossword_config() {
            let d = override_crossword_difficulty
                .or(deferred.session_options.crossword_difficulty)
                .unwrap_or(cw.default_difficulty);
            if let Some(obj) = session_meta.as_object_mut() {
                obj.insert("crossword_difficulty".into(), serde_json::json!(d));
                if cw.is_time_game && cw.game_time_seconds > 0 {
                    obj.insert(
                        "crossword_timer_secs".into(),
                        serde_json::json!(cw.game_time_seconds),
                    );
                }
            }
        }
    }
    if let Some(obj) = base_context.as_object_mut() {
        obj.insert("_session".to_string(), session_meta);
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
                "game_kind": definition.kind,
                "steps_count": session.steps.len(),
                "passage_gaps": session.steps.first().map(|s| match &s.expected_answer {
                    shakti_game_domain::ExpectedAnswer::GapFillSlots { values } => values.len(),
                    shakti_game_domain::ExpectedAnswer::Crossword { words, .. } => words.len(),
                    _ => 0,
                }).unwrap_or(0),
            }),
        )
        .await?;

    Ok(session)
}
