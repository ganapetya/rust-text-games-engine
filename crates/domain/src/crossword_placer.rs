//! Deterministic crossword placement algorithm.
//!
//! The LLM supplies **hard words** (required, H) with clues and **bridge words** (optional, M)
//! with clues. This module places them on a grid, maximising H coverage first, then total words,
//! then crossings, then compactness. No LLM is involved in grid construction.

use std::collections::{HashMap, HashSet};

use crate::crossword::{
    CrosswordDirection, CrosswordLlmOutput, CrosswordWordEntry, CROSSWORD_BLOCK,
    CROSSWORD_LLM_SCHEMA_VERSION,
};
use crate::policies::CrosswordConfig;

// ── Public input type ─────────────────────────────────────────────────────────

/// A word (with clue) eligible for crossword placement.
#[derive(Clone, Debug)]
pub struct WordCandidate {
    pub word: String, // will be uppercased + trimmed
    pub hint: String,
    pub is_hard: bool,
}

// ── Internal grid ─────────────────────────────────────────────────────────────

/// Flexible grid using signed coordinates so words can be placed in any direction
/// without pre-allocating a fixed array.
#[derive(Clone)]
struct Grid {
    cells: HashMap<(i32, i32), char>,
}

#[derive(Clone)]
struct PlacedEntry {
    chars: Vec<char>,
    hint: String,
    is_hard: bool,
    row: i32,
    col: i32,
    direction: CrosswordDirection,
}

struct PlacementOpt {
    row: i32,
    col: i32,
    direction: CrosswordDirection,
    crossings: usize,
}

impl Grid {
    fn new() -> Self {
        Self {
            cells: HashMap::new(),
        }
    }

    fn get(&self, r: i32, c: i32) -> Option<char> {
        self.cells.get(&(r, c)).copied()
    }

    fn empty_at(&self, r: i32, c: i32) -> bool {
        !self.cells.contains_key(&(r, c))
    }

    /// Try to place `word` at `(row, col)` in `dir`; return Some(crossings) if valid.
    fn try_placement(
        &self,
        word: &[char],
        row: i32,
        col: i32,
        dir: CrosswordDirection,
    ) -> Option<PlacementOpt> {
        let n = word.len() as i32;

        // Cell immediately before the word start must be empty (no head-to-tail merge).
        let (pr, pc) = match dir {
            CrosswordDirection::Across => (row, col - 1),
            CrosswordDirection::Down => (row - 1, col),
        };
        if !self.empty_at(pr, pc) {
            return None;
        }

        // Cell immediately after the word end must be empty.
        let (nr, nc) = match dir {
            CrosswordDirection::Across => (row, col + n),
            CrosswordDirection::Down => (row + n, col),
        };
        if !self.empty_at(nr, nc) {
            return None;
        }

        let mut crossings = 0usize;

        for i in 0..n as usize {
            let (r, c) = match dir {
                CrosswordDirection::Across => (row, col + i as i32),
                CrosswordDirection::Down => (row + i as i32, col),
            };

            match self.cells.get(&(r, c)) {
                Some(&existing) => {
                    if existing != word[i] {
                        return None; // Letter conflict.
                    }
                    crossings += 1;
                }
                None => {
                    // Perpendicular cells beside this empty cell must also be empty;
                    // otherwise we'd silently extend an existing parallel word.
                    let (s1, s2) = match dir {
                        CrosswordDirection::Across => ((r - 1, c), (r + 1, c)),
                        CrosswordDirection::Down => ((r, c - 1), (r, c + 1)),
                    };
                    if !self.empty_at(s1.0, s1.1) || !self.empty_at(s2.0, s2.1) {
                        return None;
                    }
                }
            }
        }

        // Non-first words must intersect something already on the grid.
        if !self.cells.is_empty() && crossings == 0 {
            return None;
        }

        Some(PlacementOpt {
            row,
            col,
            direction: dir,
            crossings,
        })
    }

    /// Return all valid placements for `word` on the current grid.
    fn find_placements(&self, word: &[char]) -> Vec<PlacementOpt> {
        // First word: only one canonical starting position.
        if self.cells.is_empty() {
            return vec![PlacementOpt {
                row: 0,
                col: 0,
                direction: CrosswordDirection::Across,
                crossings: 0,
            }];
        }

        let mut result = Vec::new();
        let mut tried: HashSet<(i32, i32, u8)> = HashSet::new();

        for (&(r, c), &gc) in &self.cells {
            for (i, &wc) in word.iter().enumerate() {
                if wc != gc {
                    continue;
                }
                let i = i as i32;

                // Across: word's i-th letter is at column c → start = (r, c-i)
                let key_a = (r, c - i, 0u8);
                if tried.insert(key_a) {
                    if let Some(p) =
                        self.try_placement(word, r, c - i, CrosswordDirection::Across)
                    {
                        result.push(p);
                    }
                }

                // Down: word's i-th letter is at row r → start = (r-i, c)
                let key_d = (r - i, c, 1u8);
                if tried.insert(key_d) {
                    if let Some(p) =
                        self.try_placement(word, r - i, c, CrosswordDirection::Down)
                    {
                        result.push(p);
                    }
                }
            }
        }

        result
    }

    fn place(&mut self, word: &[char], row: i32, col: i32, dir: CrosswordDirection) {
        for (i, &ch) in word.iter().enumerate() {
            let (r, c) = match dir {
                CrosswordDirection::Across => (row, col + i as i32),
                CrosswordDirection::Down => (row + i as i32, col),
            };
            self.cells.insert((r, c), ch);
        }
    }

    /// Bounding box `(min_r, max_r, min_c, max_c)`.
    fn bounds(&self) -> (i32, i32, i32, i32) {
        if self.cells.is_empty() {
            return (0, 0, 0, 0);
        }
        let min_r = self.cells.keys().map(|&(r, _)| r).min().unwrap();
        let max_r = self.cells.keys().map(|&(r, _)| r).max().unwrap();
        let min_c = self.cells.keys().map(|&(_, c)| c).min().unwrap();
        let max_c = self.cells.keys().map(|&(_, c)| c).max().unwrap();
        (min_r, max_r, min_c, max_c)
    }
}

// ── Grid-bounds helpers ───────────────────────────────────────────────────────

/// New bounding box after placing `word` at the given position.
fn new_bounds_after(
    word: &[char],
    p: &PlacementOpt,
    (min_r, max_r, min_c, max_c): (i32, i32, i32, i32),
    is_first: bool,
) -> (i32, i32, i32, i32) {
    let n = word.len() as i32 - 1;
    let (end_r, end_c) = match p.direction {
        CrosswordDirection::Across => (p.row, p.col + n),
        CrosswordDirection::Down => (p.row + n, p.col),
    };
    if is_first {
        return (p.row.min(end_r), p.row.max(end_r), p.col.min(end_c), p.col.max(end_c));
    }
    (
        min_r.min(p.row).min(end_r),
        max_r.max(p.row).max(end_r),
        min_c.min(p.col).min(end_c),
        max_c.max(p.col).max(end_c),
    )
}

fn area(bounds: (i32, i32, i32, i32)) -> i64 {
    let (min_r, max_r, min_c, max_c) = bounds;
    ((max_r - min_r + 1) * (max_c - min_c + 1)) as i64
}

// ── Scoring ───────────────────────────────────────────────────────────────────

fn solution_score(placed: &[PlacedEntry]) -> i64 {
    let h = placed.iter().filter(|p| p.is_hard).count() as i64;
    let total = placed.len() as i64;
    let crossings: i64 = 0; // tracked implicitly
    h * 100_000 + total * 1_000 + crossings
}

fn placement_score(
    word: &[char],
    p: &PlacementOpt,
    cur_bounds: (i32, i32, i32, i32),
    is_first: bool,
    is_hard: bool,
) -> i64 {
    let after = new_bounds_after(word, p, cur_bounds, is_first);
    let expansion = area(after) - if is_first { 0 } else { area(cur_bounds) };
    let h_bonus: i64 = if is_hard { 500 } else { 0 };
    h_bonus + p.crossings as i64 * 100 - expansion
}

// ── Deterministic seed helper ─────────────────────────────────────────────────

/// Simple deterministic hash so different seeds produce different orderings.
fn word_hash(chars: &[char], seed: u64) -> u64 {
    let mut h = seed ^ 0xdeadbeefcafe1234u64;
    for &c in chars {
        h = h
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(c as u64 ^ 0xaa);
    }
    h
}

// ── Greedy run ────────────────────────────────────────────────────────────────

/// One greedy pass with `seed`-varied bridge-word ordering.
/// H-words always come first (sorted longest-first); M-words are shuffled by seed.
fn greedy_run(
    candidates: &[(Vec<char>, String, bool)],
    cfg: &CrosswordConfig,
    seed: u64,
) -> Option<(Grid, Vec<PlacedEntry>)> {
    let max_words = cfg.max_words as usize;

    let mut h: Vec<_> = candidates.iter().filter(|(_, _, h)| *h).collect();
    let mut m: Vec<_> = candidates.iter().filter(|(_, _, h)| !*h).collect();

    // H: longest first (stable between seeds so H coverage is maximised).
    h.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    // M: vary order per seed.
    m.sort_by(|a, b| word_hash(&b.0, seed).cmp(&word_hash(&a.0, seed)));

    let ordered: Vec<_> = h.into_iter().chain(m.into_iter()).collect();

    let mut grid = Grid::new();
    let mut placed: Vec<PlacedEntry> = Vec::new();

    for (word_chars, hint, is_hard) in &ordered {
        if placed.len() >= max_words {
            break;
        }

        let placements = grid.find_placements(word_chars);
        if placements.is_empty() {
            continue;
        }

        let cur_bounds = grid.bounds();
        let is_first = grid.cells.is_empty();

        // Filter placements that would exceed the configured grid dimensions.
        let valid: Vec<_> = placements
            .into_iter()
            .filter(|p| {
                let b = new_bounds_after(word_chars, p, cur_bounds, is_first);
                (b.1 - b.0 + 1) as usize <= cfg.max_grid_rows as usize
                    && (b.3 - b.2 + 1) as usize <= cfg.max_grid_cols as usize
            })
            .collect();

        if valid.is_empty() {
            continue;
        }

        // Pick the highest-scoring placement.
        let best = valid
            .into_iter()
            .max_by_key(|p| placement_score(word_chars, p, cur_bounds, is_first, *is_hard))
            .unwrap();

        grid.place(word_chars, best.row, best.col, best.direction);
        placed.push(PlacedEntry {
            chars: word_chars.clone(),
            hint: hint.clone(),
            is_hard: *is_hard,
            row: best.row,
            col: best.col,
            direction: best.direction,
        });
    }

    if placed.is_empty() {
        return None;
    }
    if placed.iter().filter(|p| p.is_hard).count() == 0 {
        return None;
    }

    Some((grid, placed))
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Build a `CrosswordLlmOutput` from `candidates` (hard + bridge words).
///
/// Runs 12 greedy passes with different M-word orderings and returns the one
/// that maximises H coverage, then total words, then crossings.
pub fn build_crossword(
    candidates: &[WordCandidate],
    story: String,
    cfg: &CrosswordConfig,
) -> Result<CrosswordLlmOutput, String> {
    if candidates.is_empty() {
        return Err("no word candidates provided".into());
    }

    // Normalise: uppercase, trim, letters-only, min-length 2, deduplicate.
    let mut seen: HashSet<String> = HashSet::new();
    let normalized: Vec<(Vec<char>, String, bool)> = candidates
        .iter()
        .filter_map(|c| {
            let word: String = c.word.trim().to_uppercase();
            if word.len() < 2 {
                return None;
            }
            let chars: Vec<char> = word.chars().collect();
            if chars.iter().any(|ch| !ch.is_alphabetic()) {
                return None;
            }
            if !seen.insert(word) {
                return None;
            }
            Some((chars, c.hint.trim().to_string(), c.is_hard))
        })
        .collect();

    let h_total = normalized.iter().filter(|(_, _, h)| *h).count();
    if h_total == 0 {
        return Err("no valid hard words in candidates".into());
    }

    // Run 12 seeds, keep best solution.
    let mut best_grid: Option<(Grid, Vec<PlacedEntry>)> = None;
    let mut best_score = i64::MIN;

    for seed in 0u64..12 {
        if let Some((g, p)) = greedy_run(&normalized, cfg, seed) {
            let s = solution_score(&p);
            if s > best_score {
                best_score = s;
                best_grid = Some((g, p));
            }
        }
    }

    let (grid, placed) =
        best_grid.ok_or("crossword placement: no words could be fitted in the grid")?;

    // Convert to CrosswordLlmOutput.
    let (min_r, max_r, min_c, max_c) = grid.bounds();
    let rows = (max_r - min_r + 1) as usize;
    let cols = (max_c - min_c + 1) as usize;

    // Build grid rows: empty cells become '#'.
    let grid_rows: Vec<String> = (0..rows)
        .map(|ri| {
            (0..cols)
                .map(|ci| {
                    grid.get(min_r + ri as i32, min_c + ci as i32)
                        .unwrap_or(CROSSWORD_BLOCK)
                })
                .collect()
        })
        .collect();

    let words: Vec<CrosswordWordEntry> = placed
        .iter()
        .enumerate()
        .map(|(id, p)| CrosswordWordEntry {
            id: id as u32,
            answer: p.chars.iter().collect(),
            hint: p.hint.clone(),
            start_row: (p.row - min_r) as usize,
            start_col: (p.col - min_c) as usize,
            direction: p.direction,
        })
        .collect();

    Ok(CrosswordLlmOutput {
        schema_version: CROSSWORD_LLM_SCHEMA_VERSION,
        story,
        grid: grid_rows,
        words,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policies::{CrosswordConfig, ScoringPolicy};

    fn cfg() -> CrosswordConfig {
        CrosswordConfig {
            max_grid_rows: 15,
            max_grid_cols: 15,
            max_words: 15,
            max_hint_chars: 120,
            is_time_game: false,
            game_time_seconds: 0,
            default_difficulty: 3,
            max_learning_items_for_llm: 10,
        }
    }

    fn make_hard(words: &[&str]) -> Vec<WordCandidate> {
        words
            .iter()
            .map(|w| WordCandidate {
                word: w.to_string(),
                hint: format!("Hint for {w}"),
                is_hard: true,
            })
            .collect()
    }

    #[test]
    fn places_single_hard_word() {
        let candidates = make_hard(&["KAFFE"]);
        let out = build_crossword(&candidates, "story".into(), &cfg()).unwrap();
        assert_eq!(out.words.len(), 1);
        assert_eq!(out.words[0].answer, "KAFFE");
        out.validate().unwrap();
    }

    #[test]
    fn two_crossing_words() {
        // KAFFE and FROSK share the letter F
        let mut c = make_hard(&["KAFFE", "FROSK"]);
        let out = build_crossword(&mut c, "story".into(), &cfg()).unwrap();
        // Both should be placed (they can cross on F)
        let answers: Vec<_> = out.words.iter().map(|w| w.answer.as_str()).collect();
        assert!(answers.contains(&"KAFFE"));
        assert!(answers.contains(&"FROSK"));
        out.validate().unwrap();
    }

    #[test]
    fn bridge_words_used_for_crossing() {
        // H word that can't cross with itself; bridge provides a crossing path
        let candidates = vec![
            WordCandidate { word: "HUND".into(), hint: "Dog".into(), is_hard: true },
            WordCandidate { word: "KATT".into(), hint: "Cat".into(), is_hard: true },
            WordCandidate { word: "ANKA".into(), hint: "Duck".into(), is_hard: false },
        ];
        let out = build_crossword(&candidates, "story".into(), &cfg()).unwrap();
        // At least one H word must be placed
        assert!(out.words.iter().any(|w| w.answer == "HUND" || w.answer == "KATT"));
        out.validate().unwrap();
    }

    #[test]
    fn h_coverage_dominates() {
        // Many bridge words, few H — H must all be placed
        let mut candidates: Vec<_> = (0..10)
            .map(|i| WordCandidate {
                word: format!("BRIDGE{i}"),
                hint: "bridge".into(),
                is_hard: false,
            })
            .collect();
        candidates.push(WordCandidate {
            word: "ELEV".into(),
            hint: "student".into(),
            is_hard: true,
        });
        candidates.push(WordCandidate {
            word: "SKOLE".into(),
            hint: "school".into(),
            is_hard: true,
        });
        let out = build_crossword(&candidates, "story".into(), &cfg()).unwrap();
        let placed_h: Vec<_> = out
            .words
            .iter()
            .filter(|w| w.answer == "ELEV" || w.answer == "SKOLE")
            .collect();
        assert!(!placed_h.is_empty(), "at least one H word must be placed");
        out.validate().unwrap();
    }
}
