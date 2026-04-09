-- Explicit LLM / content-fetch tunables on default gap_fill definition (serde defaults already match in code).
-- Do not change older migration files for this: sqlx stores a checksum per version; edits break existing DBs (VersionMismatch).

UPDATE game_definitions
SET config = config || '{
  "max_llm_gap_slots": 10,
  "max_llm_sentences": 5,
  "max_learning_items_for_llm": 100
}'::jsonb
WHERE id = '00000000-0000-0000-0000-000000000001' AND kind = 'gap_fill';
