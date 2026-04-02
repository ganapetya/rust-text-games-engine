-- Existing databases created before user_facing_step_prompt column name.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'game_steps'
          AND column_name = 'prompt'
    ) THEN
        ALTER TABLE game_steps RENAME COLUMN prompt TO user_facing_step_prompt;
    END IF;
END $$;
