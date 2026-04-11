//! On-demand full-text translation of passage (`full_text`) or crossword (`story`).

use crate::billing::{read_wallet_from_base, wallet_llm_blocked};
use crate::deps::EngineDeps;
use crate::errors::AppError;
use crate::ports::GameLlmChargeArgs;
use shakti_game_domain::{
    CrosswordLlmOutput, GameKind, GameSessionId, GameSessionState, PassageGapLlmOutput, UserId,
};
use shakti_game_pricing::coins_for_usage;
use shakti_game_translation::TranslationParams;

const MAX_HINT_LANGS: usize = 16;

#[derive(Debug, Clone)]
pub struct TranslationHintOutput {
    pub translated_text: String,
    pub source_language: String,
    pub target_language: String,
}

pub fn normalize_hint_translation_languages(raw: Option<&Vec<String>>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    if let Some(list) = raw {
        for s in list {
            let t = s.trim().to_lowercase();
            if t.is_empty() {
                continue;
            }
            if seen.insert(t.clone()) && out.len() < MAX_HINT_LANGS {
                out.push(t);
            }
        }
    }
    out
}

pub fn normalize_lang_code(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Reads `_session` merged into `base_context` at materialize time.
pub fn read_session_ui_hints(base: &serde_json::Value) -> (Option<String>, Vec<String>) {
    let Some(sess) = base.get("_session") else {
        return (None, Vec::new());
    };
    let source = sess
        .get("source_language")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let langs = sess
        .get("hint_translation_languages")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    (source, langs)
}

pub async fn request_translation_hint(
    deps: &EngineDeps,
    session_id: GameSessionId,
    user_id: UserId,
    target_language_raw: &str,
    trace_id: Option<&str>,
) -> Result<TranslationHintOutput, AppError> {
    let mut session = deps.sessions.get(session_id).await?;
    if session.user_id != user_id {
        return Err(AppError::Forbidden);
    }
    if session.state != GameSessionState::InProgress {
        return Err(AppError::Conflict(
            "translation hint is only available while the session is in progress".into(),
        ));
    }

    let game_kind = session.definition().map_err(AppError::Domain)?.kind;
    if game_kind == GameKind::CorrectUsage {
        return Err(AppError::BadRequest(
            "translation hints are not available for correct_usage sessions".into(),
        ));
    }

    let (source_opt, hint_langs) = read_session_ui_hints(&session.base_context);
    let Some(source_language) = source_opt else {
        return Err(AppError::BadRequest(
            "session has no translation metadata (start the session with hintTranslationLanguages)"
                .into(),
        ));
    };

    if hint_langs.is_empty() {
        return Err(AppError::BadRequest(
            "translation hints are not enabled for this session".into(),
        ));
    }

    let target_language = normalize_lang_code(target_language_raw);
    if target_language.is_empty() {
        return Err(AppError::BadRequest("targetLanguage must be non-empty".into()));
    }
    if !hint_langs.iter().any(|l| l == &target_language) {
        return Err(AppError::BadRequest(format!(
            "targetLanguage {target_language:?} is not in the session allow-list"
        )));
    }

    if let Some(w) = read_wallet_from_base(&session.base_context) {
        if wallet_llm_blocked(&w) {
            return Err(AppError::InsufficientBalance(
                "Insufficient balance for translation hints. Please purchase more points.".into(),
            ));
        }
    }

    let source_text: String = match game_kind {
        GameKind::GapFill => {
            let passage: PassageGapLlmOutput =
                serde_json::from_value(session.base_context.clone()).map_err(|e| {
                    AppError::Repository(format!("stored passage (base_context): {e}"))
                })?;
            passage.full_text
        }
        GameKind::Crossword => {
            let cw: CrosswordLlmOutput =
                serde_json::from_value(session.base_context.clone()).map_err(|e| {
                    AppError::Repository(format!("stored crossword (base_context): {e}"))
                })?;
            cw.story
        }
        GameKind::CorrectUsage => {
            return Err(AppError::BadRequest(
                "translation hints are not available for correct_usage sessions".into(),
            ));
        }
    };

    let cache_key = target_language.clone();

    if let Some(sess) = session.base_context.get("_session") {
        if let Some(cache) = sess.get("translation_cache") {
            if let Some(hit) = cache.get(&cache_key).and_then(|v| v.as_str()) {
                let t = hit.trim();
                if !t.is_empty() {
                    tracing::info!(
                        user_id = %user_id.0,
                        trace_id = trace_id.unwrap_or(""),
                        session_id = %session_id.0,
                        target_language = %target_language,
                        "translation_hint cache hit"
                    );
                    return Ok(TranslationHintOutput {
                        translated_text: t.to_string(),
                        source_language,
                        target_language,
                    });
                }
            }
        }
    }

    tracing::info!(
        user_id = %user_id.0,
        trace_id = trace_id.unwrap_or(""),
        session_id = %session_id.0,
        target_language = %target_language,
        chars = source_text.chars().count(),
        "translation_hint llm call"
    );

    let (translated, usage) = deps
        .llm_translator
        .translate(
            &user_id.0.to_string(),
            trace_id,
            TranslationParams {
                source_lang: source_language.clone(),
                target_lang: target_language.clone(),
                text: source_text.clone(),
            },
        )
        .await
        .map_err(|e| AppError::LlmPreparation(e.to_string()))?;

    if let (Some(ref sched), Some(ref wallet)) = (
        &deps.billing_scheduler,
        read_wallet_from_base(&session.base_context),
    ) {
        let coins = coins_for_usage(
            usage.prompt_tokens,
            usage.completion_tokens,
            wallet.billing_rates.translate.input_per_1k,
            wallet.billing_rates.translate.output_per_1k,
        );
        if coins > 0 {
            sched.schedule_game_llm_charge(GameLlmChargeArgs {
                session_id,
                shakti_user_id: wallet.shakti_user_id,
                trace_id: trace_id.unwrap_or("").to_string(),
                variant: wallet.billing_rates.variant.clone(),
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                coins,
                endpoint: "/game/translate",
            });
        }
    }

    if let Some(root) = session.base_context.as_object_mut() {
        let session_obj = root
            .entry("_session".to_string())
            .or_insert(serde_json::json!({}));
        if let Some(so) = session_obj.as_object_mut() {
            let cache = so
                .entry("translation_cache".to_string())
                .or_insert(serde_json::json!({}));
            if let Some(co) = cache.as_object_mut() {
                co.insert(cache_key, serde_json::Value::String(translated.clone()));
            }
        }
    }

    deps.sessions.update(&session).await?;

    Ok(TranslationHintOutput {
        translated_text: translated,
        source_language,
        target_language,
    })
}
