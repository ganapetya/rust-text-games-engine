use shakti_game_domain::{
    CorrectUsageLlmOutput, CrosswordHintsLlmOutput, CrosswordLlmOutput, PassageGapLlmOutput,
    PassageHardWordOccurrence,
};

fn spans_overlap(a: (usize, usize), b: (usize, usize)) -> bool {
    a.0 < b.1 && b.0 < a.1
}

/// Finds `surface` in `full_text` at char index `start >= min_start` such that `[start, end)` does not overlap any span in `used`.
fn find_surface_non_overlapping(
    full_text: &str,
    surface: &str,
    used: &[(usize, usize)],
    min_start: usize,
) -> Option<(usize, usize)> {
    let hay: Vec<char> = full_text.chars().collect();
    let needle: Vec<char> = surface.chars().collect();
    if needle.is_empty() || min_start > hay.len() {
        return None;
    }
    let last_start = hay.len().saturating_sub(needle.len());
    for start in min_start..=last_start {
        if hay[start..start + needle.len()] != needle[..] {
            continue;
        }
        let span = (start, start + needle.len());
        if used.iter().any(|u| spans_overlap(*u, span)) {
            continue;
        }
        return Some(span);
    }
    None
}

/// Locates each `hard_words[].surface` in `full_text`, fixes indices, drops entries not present in the passage.
///
/// The **puzzle** is whatever the model returns in `hard_words` that actually exists in `full_text`.
/// Suggested vocabulary from the user may be omitted by the model — orphan `hard_words` entries
/// (surface not in text) are dropped. **Same surface N times** ⇒ N entries, matched using non-overlapping
/// spans (prefer after the previous match, then anywhere in the text).
pub fn reconcile_hard_word_spans(output: &mut PassageGapLlmOutput) -> Result<(), String> {
    if output.hard_words.is_empty() {
        return Ok(());
    }

    let mut order: Vec<usize> = (0..output.hard_words.len()).collect();
    order.sort_by_key(|&i| output.hard_words[i].id);

    let mut used: Vec<(usize, usize)> = Vec::new();
    let mut search_from = 0usize;
    let mut kept: Vec<PassageHardWordOccurrence> = Vec::new();

    for &i in &order {
        let surface = output.hard_words[i].surface.clone();
        let span = find_surface_non_overlapping(&output.full_text, &surface, &used, search_from)
            .or_else(|| find_surface_non_overlapping(&output.full_text, &surface, &used, 0));

        match span {
            Some((s, e)) => {
                used.push((s, e));
                kept.push(PassageHardWordOccurrence {
                    id: 0,
                    start_char: s,
                    end_char: e,
                    surface,
                });
                search_from = e;
            }
            None => {
                tracing::warn!(
                    model_id = output.hard_words[i].id,
                    surface = %output.hard_words[i].surface,
                    "dropping hard_word: surface not found in full_text (non-overlapping)"
                );
            }
        }
    }

    if kept.is_empty() {
        return Err(
            "after reconciliation no hard_words remain (none of the model surfaces appear in full_text)"
                .into(),
        );
    }

    kept.sort_by_key(|h| h.start_char);
    for (n, hw) in kept.iter_mut().enumerate() {
        hw.id = n as u32;
    }
    output.hard_words = kept;
    Ok(())
}

/// Removes optional ```json ... ``` fences from model output.
pub fn strip_code_fences(s: &str) -> String {
    let t = s.trim();
    if let Some(rest) = t.strip_prefix("```") {
        let rest = rest.trim_start();
        let rest = rest.strip_prefix("json").unwrap_or(rest).trim_start();
        if let Some(idx) = rest.rfind("```") {
            return rest[..idx].trim().to_string();
        }
        return rest.trim().to_string();
    }
    t.to_string()
}

/// Parses and returns [`PassageGapLlmOutput`] (caller runs reconcile if needed, then [`PassageGapLlmOutput::validate`]).
pub fn parse_passage_gap_response(raw: &str) -> Result<PassageGapLlmOutput, String> {
    let cleaned = strip_code_fences(raw);
    serde_json::from_str::<PassageGapLlmOutput>(&cleaned)
        .map_err(|e| format!("invalid LLM JSON: {e}"))
}

pub fn parse_correct_usage_response(raw: &str) -> Result<CorrectUsageLlmOutput, String> {
    let cleaned = strip_code_fences(raw);
    let mut out = serde_json::from_str::<CorrectUsageLlmOutput>(&cleaned)
        .map_err(|e| format!("invalid correct_usage LLM JSON: {e}"))?;
    out.repair_pairwise_duplicate_sentences();
    Ok(out)
}

/// Parse and lightly repair the old full-grid crossword LLM response (kept for compatibility).
#[allow(dead_code)]
pub fn parse_crossword_response(raw: &str) -> Result<CrosswordLlmOutput, String> {
    let cleaned = strip_code_fences(raw);
    let mut out = serde_json::from_str::<CrosswordLlmOutput>(&cleaned)
        .map_err(|e| format!("invalid crossword LLM JSON: {e}"))?;
    out.normalize_case();
    out.repair_grid_widths();
    out.repair_word_grid_conflicts();
    Ok(out)
}

/// Parse the hints-and-bridges LLM response (new strategy).
pub fn parse_crossword_hints_response(raw: &str) -> Result<CrosswordHintsLlmOutput, String> {
    let cleaned = strip_code_fences(raw);
    let mut out = serde_json::from_str::<CrosswordHintsLlmOutput>(&cleaned)
        .map_err(|e| format!("invalid crossword hints LLM JSON: {e}"))?;

    // Normalise all words to uppercase.
    for hw in &mut out.hard_word_hints {
        hw.word = hw.word.trim().to_uppercase();
    }
    for bw in &mut out.bridge_words {
        bw.word = bw.word.trim().to_uppercase();
    }

    // Filter bridge words to letters-only (some models sneak in hyphens etc.).
    out.bridge_words
        .retain(|bw| !bw.word.is_empty() && bw.word.chars().all(|c| c.is_alphabetic()));

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakti_game_domain::PASSAGE_LLM_SCHEMA_VERSION;

    #[test]
    fn strip_fences() {
        let s = "```json\n{\"schema_version\":1}\n```";
        assert!(strip_code_fences(s).contains("schema_version"));
    }

    #[test]
    fn parse_passage() {
        let raw = r#"{"schema_version":1,"full_text":"ab","hard_words":[{"id":0,"start_char":0,"end_char":1,"surface":"a"}],"fake_words":["z"]}"#;
        let mut p = parse_passage_gap_response(raw).unwrap();
        assert_eq!(p.schema_version, PASSAGE_LLM_SCHEMA_VERSION);
        reconcile_hard_word_spans(&mut p).unwrap();
        p.validate().unwrap();
    }

    #[test]
    fn reconcile_fixes_wrong_offsets_norwegian() {
        let raw = r#"{"schema_version":1,"full_text":"Morgenen da solen kilte å løftet seg.","hard_words":[{"id":0,"start_char":99,"end_char":99,"surface":"solen"},{"id":1,"start_char":0,"end_char":0,"surface":"løftet"}],"fake_words":["x"]}"#;
        let mut p = parse_passage_gap_response(raw).unwrap();
        reconcile_hard_word_spans(&mut p).unwrap();
        p.validate().unwrap();
        let surfaces: Vec<_> = p.hard_words.iter().map(|h| h.surface.as_str()).collect();
        assert!(surfaces.contains(&"solen"));
        assert!(surfaces.contains(&"løftet"));
    }

    #[test]
    fn reconcile_drops_phantom_surface() {
        let raw = r#"{"schema_version":1,"full_text":"Bare solen og liste her.","hard_words":[{"id":0,"start_char":0,"end_char":0,"surface":"solen"},{"id":1,"start_char":0,"end_char":0,"surface":"danset"},{"id":2,"start_char":0,"end_char":0,"surface":"liste"}],"fake_words":["x"]}"#;
        let mut p = parse_passage_gap_response(raw).unwrap();
        reconcile_hard_word_spans(&mut p).unwrap();
        p.validate().unwrap();
        assert_eq!(p.hard_words.len(), 2);
        let surfaces: Vec<_> = p.hard_words.iter().map(|h| h.surface.as_str()).collect();
        assert!(surfaces.contains(&"solen"));
        assert!(surfaces.contains(&"liste"));
        assert!(!surfaces.contains(&"danset"));
    }

    #[test]
    fn reconcile_same_surface_twice() {
        let raw = r#"{"schema_version":1,"full_text":"solen skinner og solen gikk ned.","hard_words":[{"id":0,"surface":"solen","start_char":0,"end_char":0},{"id":1,"surface":"solen","start_char":0,"end_char":0}],"fake_words":["x"]}"#;
        let mut p = parse_passage_gap_response(raw).unwrap();
        reconcile_hard_word_spans(&mut p).unwrap();
        p.validate().unwrap();
        assert_eq!(p.hard_words.len(), 2);
        assert_ne!(p.hard_words[0].start_char, p.hard_words[1].start_char);
    }
}
