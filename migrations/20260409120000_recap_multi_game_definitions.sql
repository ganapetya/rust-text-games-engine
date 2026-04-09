-- Recap multi-game: keep default gap_fill as highest version; add second definition (morphology template).
UPDATE game_definitions
SET version = 2
WHERE id = '00000000-0000-0000-0000-000000000001' AND kind = 'gap_fill';

INSERT INTO game_definitions (
    id,
    kind,
    version,
    name,
    config,
    scoring_policy,
    timing_policy,
    active
) VALUES (
    '00000000-0000-0000-0000-000000000002',
    'gap_fill',
    1,
    'Gap fill — morphology distractors',
    '{"type":"gap_fill","max_passage_words":600,"distractors_per_gap":2,"allow_skip":false,"scoring_mode":"per_gap","max_llm_gap_slots":10,"max_llm_sentences":5,"max_learning_items_for_llm":100,"llm_template":"morphology_distractors"}'::jsonb,
    '{"type": "fixed_per_correct", "points": 10}'::jsonb,
    '{"per_step_limit_secs": null, "session_limit_secs": null, "auto_advance_on_timeout": false}'::jsonb,
    true
);
