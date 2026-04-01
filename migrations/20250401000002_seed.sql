-- Default gap_fill definition and sample learning items for a dev user UUID

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
    '00000000-0000-0000-0000-000000000001',
    'gap_fill',
    1,
    'Default gap fill',
    '{"steps_count": 5, "distractors_per_step": 2, "allow_skip": false}'::jsonb,
    '{"type": "fixed_per_correct", "points": 10}'::jsonb,
    '{"per_step_limit_secs": null, "session_limit_secs": null, "auto_advance_on_timeout": false}'::jsonb,
    true
);

-- Dev user: 11111111-1111-1111-1111-111111111111
INSERT INTO learning_items (id, user_id, source_text, context_text, hard_fragment, lemma, language, metadata)
VALUES
(
    'a0000000-0000-0000-0000-000000000001',
    '11111111-1111-1111-1111-111111111111',
    'Han gikk til butikken.',
    'Han gikk til butikken i går.',
    'gikk',
    'gå',
    'no',
    '{}'
),
(
    'a0000000-0000-0000-0000-000000000002',
    '11111111-1111-1111-1111-111111111111',
    'Jeg ser en katt.',
    'Jeg ser en svart katt.',
    'svart',
    NULL,
    'no',
    '{}'
),
(
    'a0000000-0000-0000-0000-000000000003',
    '11111111-1111-1111-1111-111111111111',
    'Det regner.',
    NULL,
    'regner',
    NULL,
    'no',
    '{}'
),
(
    'a0000000-0000-0000-0000-000000000004',
    '11111111-1111-1111-1111-111111111111',
    'Hun leser bok.',
    'Hun leser en god bok.',
    'leser',
    'lese',
    'no',
    '{}'
),
(
    'a0000000-0000-0000-0000-000000000005',
    '11111111-1111-1111-1111-111111111111',
    'Vi skal hjem.',
    NULL,
    'hjem',
    NULL,
    'no',
    '{}'
);
