-- Tagged gap_fill config (passage mode) + sample registered hard words for integration tests

UPDATE game_definitions
SET config = '{
  "type": "gap_fill",
  "max_passage_words": 600,
  "distractors_per_gap": 2,
  "allow_skip": false,
  "scoring_mode": "per_gap"
}'::jsonb
WHERE id = '00000000-0000-0000-0000-000000000001';

INSERT INTO user_hard_words (id, user_id, language, word)
VALUES
(
    'b0000000-0000-0000-0000-000000000001',
    '11111111-1111-1111-1111-111111111111',
    'no',
    'gikk'
),
(
    'b0000000-0000-0000-0000-000000000002',
    '11111111-1111-1111-1111-111111111111',
    'no',
    'svart'
),
(
    'b0000000-0000-0000-0000-000000000003',
    '11111111-1111-1111-1111-111111111111',
    'no',
    'regner'
),
(
    'b0000000-0000-0000-0000-000000000004',
    '11111111-1111-1111-1111-111111111111',
    'no',
    'leser'
),
(
    'b0000000-0000-0000-0000-000000000005',
    '11111111-1111-1111-1111-111111111111',
    'no',
    'hjem'
)
ON CONFLICT DO NOTHING;
