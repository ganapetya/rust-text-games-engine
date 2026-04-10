-- User-facing names for recap / picker (align with RECAP_GAME_DEFINITION_LABELS defaults).

UPDATE game_definitions
SET name = 'Select Words'
WHERE id = '00000000-0000-0000-0000-000000000001' AND kind = 'gap_fill';

UPDATE game_definitions
SET name = 'Select Words (Morph)'
WHERE id = '00000000-0000-0000-0000-000000000002' AND kind = 'gap_fill';
