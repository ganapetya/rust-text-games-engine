-- shakti-game-engine v0.1 schema

CREATE TABLE game_definitions (
    id UUID PRIMARY KEY,
    kind TEXT NOT NULL,
    version INT NOT NULL,
    name TEXT NOT NULL,
    config JSONB NOT NULL,
    scoring_policy JSONB NOT NULL,
    timing_policy JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    active BOOLEAN NOT NULL DEFAULT true
);

CREATE TABLE game_sessions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL,
    definition_id UUID NOT NULL REFERENCES game_definitions(id),
    state TEXT NOT NULL,
    current_step_index INT NOT NULL DEFAULT 0,
    score JSONB NOT NULL,
    started_at TIMESTAMPTZ NULL,
    completed_at TIMESTAMPTZ NULL,
    expires_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_game_sessions_user ON game_sessions(user_id);

CREATE TABLE game_steps (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES game_sessions(id) ON DELETE CASCADE,
    ordinal INT NOT NULL,
    state TEXT NOT NULL,
    prompt JSONB NOT NULL,
    expected_answer JSONB NOT NULL,
    user_answer JSONB NULL,
    evaluation JSONB NULL,
    deadline_at TIMESTAMPTZ NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(session_id, ordinal)
);

CREATE INDEX idx_game_steps_session ON game_steps(session_id);

CREATE TABLE learning_items (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL,
    source_text TEXT NOT NULL,
    context_text TEXT NULL,
    hard_fragment TEXT NOT NULL,
    lemma TEXT NULL,
    language TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_learning_items_user ON learning_items(user_id);
CREATE INDEX idx_learning_items_lang ON learning_items(user_id, language);

CREATE TABLE session_events (
    id UUID PRIMARY KEY,
    session_id UUID NOT NULL REFERENCES game_sessions(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_session_events_session ON session_events(session_id);
