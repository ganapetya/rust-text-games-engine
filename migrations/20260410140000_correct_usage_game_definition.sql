-- Choose correct usage: one LLM batch → one step per hard word (A/B/C sentences).
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
    '00000000-0000-0000-0000-000000000003',
    'correct_usage',
    1,
    'Choose correct usage',
    '{"type":"correct_usage","max_learning_items_for_llm":100,"max_sentence_words":15}'::jsonb,
    '{"type": "fixed_per_correct", "points": 10}'::jsonb,
    '{"per_step_limit_secs": null, "session_limit_secs": null, "auto_advance_on_timeout": false}'::jsonb,
    true
);
