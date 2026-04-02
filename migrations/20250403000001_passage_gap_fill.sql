-- Passage-based gap-fill: registered vocabulary + session deferred start + persisted LLM context

CREATE TABLE user_hard_words (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL,
    language TEXT NOT NULL,
    word TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, language, word)
);

CREATE INDEX idx_user_hard_words_user_lang ON user_hard_words (user_id, language);

ALTER TABLE game_sessions
    ADD COLUMN IF NOT EXISTS base_context JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS deferred_payload JSONB NULL;

COMMENT ON COLUMN game_sessions.base_context IS 'Validated LLM passage payload (full_text, hard word spans, fake words, etc.)';
COMMENT ON COLUMN game_sessions.deferred_payload IS 'JSON: content_request + session_options from create_session; consumed on start_session';
