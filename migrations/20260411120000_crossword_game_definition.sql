-- Crossword: story + grid + hints; single step; per-word scoring.
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
    '00000000-0000-0000-0000-000000000004',
    'crossword',
    1,
    'Crossword',
    '{"type":"crossword","max_learning_items_for_llm":100,"max_grid_rows":24,"max_grid_cols":24,"max_words":40,"max_hint_chars":400,"is_time_game":false,"game_time_seconds":0,"default_difficulty":3}'::jsonb,
    '{"type": "fixed_per_correct", "points": 10}'::jsonb,
    '{"per_step_limit_secs": null, "session_limit_secs": null, "auto_advance_on_timeout": false}'::jsonb,
    true
);
