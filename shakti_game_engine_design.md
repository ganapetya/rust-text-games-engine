# shakti-game-engine — Design Document

## 1. Purpose

`shakti-game-engine` is a Rust backend service that powers language-learning games built from user learning sessions.

The engine will:

- run in a Docker container
- persist state in PostgreSQL
- expose an HTTP API for a web UI
- support one-player games initially
- fetch and transform learning material (for example: hard words with context)
- define game rules as explicit state-machine transitions
- support multiple game types with different rule sets
- score and estimate results
- support time-limited steps
- optionally use LLM-based preprocessing before content is shown to the user

A second important purpose of the project is **Rust skill sharpening**. The design therefore favors:

- explicit domain modeling
- trait-based abstractions
- strong typing
- separation between pure domain logic and infrastructure
- testable state transitions
- incremental complexity

---

## 2. Product Goal

The engine is not just a CRUD service for games. It is a **game orchestration backend** that:

1. receives or fetches learning material
2. builds a concrete game session from that material
3. executes game rules step by step
4. evaluates user answers
5. emits updated game state, score, and results

Think of it as:

**Learning Content -> Game Generator -> Runtime State Machine -> Scoring / Results**

---

## 3. High-Level Architecture

## Main architectural style

Use a layered modular monolith first.

This is the best fit because:

- you are learning Rust
- the system is not yet huge
- game logic needs fast iteration
- modular monolith keeps boundaries clean without distributed complexity

Later, if needed, LLM orchestration or content ingestion can be extracted.

## Layers

### A. API Layer
Responsible for:

- HTTP routing
- request/response DTOs
- auth integration later
- validation at transport boundary

Suggested stack:

- `axum` for HTTP
- `serde` for JSON
- `utoipa` or `aide` for OpenAPI generation

### B. Application Layer
Responsible for use cases:

- initiate game
- fetch current state
- submit answer
- advance step
- handle timeouts
- finish game
- score game
- trigger content preparation

This layer coordinates repositories, clocks, scheduler, LLM service, and domain logic.

### C. Domain Layer
Responsible for core business logic:

- game definitions
- state machine
- transition validation
- scoring rules
- timing rules
- step evaluation
- game session lifecycle

This must be as pure as possible.

### D. Infrastructure Layer
Responsible for:

- PostgreSQL repositories
- HTTP clients
- LLM provider adapters
- background scheduler / timeout checker
- configuration
- tracing / logging

---

## 4. Core Design Choice: Game = Definition + Session + State Machine

Separate **game type definition** from **game session runtime**.

### 4.1 Game Definition
A game definition describes:

- game kind (`gap_fill`, `morph_gap_fill`, etc.)
- allowed state transitions
- scoring strategy
- timing strategy
- content preparation strategy
- answer validation strategy

This is mostly static.

### 4.2 Game Session
A game session is a concrete run for one user.

It includes:

- selected content
- generated steps
- current state
- current score
- start time / end time
- per-step deadlines
- answer history
- result summary

### 4.3 State Machine
Each game session evolves through explicit states.

Example:

- `Draft`
- `Prepared`
- `InProgress`
- `WaitingForAnswer`
- `StepEvaluated`
- `Completed`
- `TimedOut`
- `Cancelled`

Transitions are explicit and validated.

This is a good Rust-learning opportunity because it maps nicely to enums and typed transition logic.

---

## 5. Recommended Tech Stack

## Core

- Rust stable
- `axum` — HTTP API
- `tokio` — async runtime
- `sqlx` — PostgreSQL access with compile-time checked queries
- `serde` / `serde_json` — serialization
- `uuid` — identifiers
- `time` or `chrono` — timestamps
- `thiserror` — domain and engine-layer errors
- `anyhow` — only at binary/integration boundaries if needed
- `tracing` and `tracing-subscriber` — structured logs
- `tower` / `tower-http` — middleware

## Nice additions

- `validator` — DTO validation
- `utoipa` — OpenAPI
- `enum_dispatch` or plain trait objects/enums depending on taste
- `bon` or builders if you want ergonomic construction, but optional

## Why `sqlx`

For Rust learning, `sqlx` is excellent because it keeps SQL visible and teaches you explicit persistence, instead of hiding too much behind an ORM.

---

## 6. Project Structure

Recommended workspace layout:

```text
shakti-game-engine/
  Cargo.toml
  .env
  docker-compose.yml
  Dockerfile
  migrations/
  crates/
    app/
      src/
        main.rs
        lib.rs
        config.rs
        startup.rs
    api/
      src/
        lib.rs
        routes/
          health.rs
          games.rs
          sessions.rs
          admin.rs
        dto/
        error.rs
    engine/
      src/
        lib.rs
        services/
          create_game_session.rs
          get_game_session.rs
          submit_answer.rs
          advance_session.rs
          timeout_sessions.rs
          prepare_content.rs
        ports/
          repositories.rs
          llm.rs
          scheduler.rs
          content_provider.rs
          clock.rs
    domain/
      src/
        lib.rs
        common/
          ids.rs
          types.rs
          score.rs
          timer.rs
          errors.rs
        content/
          learning_item.rs
          prepared_content.rs
        game/
          game_kind.rs
          game_definition.rs
          game_step.rs
          game_session.rs
          game_state.rs
          transition.rs
          answer.rs
          result.rs
          rules.rs
        evaluation/
          evaluator.rs
          scoring.rs
        engines/
          gap_fill/
            mod.rs
            definition.rs
            generator.rs
            evaluator.rs
          morph_gap_fill/
            mod.rs
            definition.rs
            generator.rs
            evaluator.rs
    infrastructure/
      src/
        lib.rs
        db/
          mod.rs
          repositories/
        llm/
          mod.rs
          openai.rs
        scheduler/
          mod.rs
          poller.rs
        content/
          mod.rs
          session_content_provider.rs
        clock/
          system_clock.rs
```

### Why workspace?

Because it forces clean boundaries and helps you learn Rust modularity. But keep it pragmatic. If workspace feels too heavy in the first week, start with one crate and evolve into the above.

---

## 7. Domain Model

## 7.1 Important concepts

### User
For now only reference user id.

```rust
pub struct UserId(pub Uuid);
```

### LearningItem
This is the raw material fetched from user learning sessions.

Examples:

- hard word
- original sentence
- translated sentence
- lemma
- morphology metadata
- source language
- target language
- user difficulty mark

```rust
pub struct LearningItem {
    pub id: LearningItemId,
    pub user_id: UserId,
    pub source_text: String,
    pub context_text: Option<String>,
    pub hard_fragment: String,
    pub lemma: Option<String>,
    pub morphology: Option<MorphologyInfo>,
    pub language: LanguageCode,
    pub metadata: serde_json::Value,
}
```

### PreparedContent
A transformed version ready for a game.

```rust
pub struct PreparedContent {
    pub items: Vec<PreparedItem>,
    pub provenance: ContentProvenance,
    pub transformation_log: Vec<TransformationRecord>,
}
```

This allows LLM-based or rule-based modifications while keeping traceability.

### GameKind
An enum of supported game families.

```rust
pub enum GameKind {
    GapFill,
    MorphGapFill,
    // later:
    // MatchPairs,
    // BuildSentence,
    // FlashRecall,
}
```

### GameDefinition
A game type plus config.

```rust
pub struct GameDefinition {
    pub id: GameDefinitionId,
    pub kind: GameKind,
    pub version: i32,
    pub name: String,
    pub config: GameConfig,
    pub scoring_policy: ScoringPolicy,
    pub timing_policy: TimingPolicy,
}
```

Use versioning from day one. It will save you trouble later when rules evolve.

### GameSession
A concrete playable session.

```rust
pub struct GameSession {
    pub id: GameSessionId,
    pub user_id: UserId,
    pub definition_id: GameDefinitionId,
    pub state: GameSessionState,
    pub steps: Vec<GameStep>,
    pub current_step_index: usize,
    pub score: Score,
    pub started_at: Option<OffsetDateTime>,
    pub completed_at: Option<OffsetDateTime>,
    pub expires_at: Option<OffsetDateTime>,
}
```

### GameStep
A single interactive unit.

```rust
pub struct GameStep {
    pub id: GameStepId,
    pub prompt: StepPrompt,
    pub expected_answer: ExpectedAnswer,
    pub user_answer: Option<UserAnswer>,
    pub evaluation: Option<StepEvaluation>,
    pub deadline_at: Option<OffsetDateTime>,
    pub state: StepState,
}
```

### Session State

```rust
pub enum GameSessionState {
    Draft,
    Prepared,
    InProgress,
    Completed,
    TimedOut,
    Cancelled,
}
```

### Step State

```rust
pub enum StepState {
    Pending,
    Active,
    Answered,
    Evaluated,
    TimedOut,
    Skipped,
}
```

---

## 8. State Machine Design

Model transitions explicitly.

## Session transitions

```text
Draft -> Prepared
Prepared -> InProgress
InProgress -> Completed
InProgress -> TimedOut
InProgress -> Cancelled
```

## Step transitions

```text
Pending -> Active
Active -> Answered
Answered -> Evaluated
Active -> TimedOut
Active -> Skipped
```

## Rust modeling options

### Option A — Simple and practical
Keep state as enums and validate transitions through domain methods.

```rust
impl GameSession {
    pub fn start(&mut self, now: OffsetDateTime) -> Result<(), DomainError> { ... }
    pub fn submit_answer(&mut self, command: SubmitAnswerCommand, now: OffsetDateTime) -> Result<StepEvaluation, DomainError> { ... }
    pub fn timeout_current_step(&mut self, now: OffsetDateTime) -> Result<(), DomainError> { ... }
}
```

### Option B — Fully typed states
Represent states as different types.

This is elegant but heavier.
For this project, start with **Option A**. It is much more development-friendly.

---

## 9. Supporting Multiple Games

This is one of the most important design areas.

Use a **plugin-like internal architecture** based on traits.

## Core abstraction

```rust
pub trait GameEngine: Send + Sync {
    fn kind(&self) -> GameKind;

    fn prepare_content(
        &self,
        input: &[LearningItem],
        config: &GameConfig,
    ) -> Result<PreparedContent, DomainError>;

    fn generate_steps(
        &self,
        content: &PreparedContent,
        config: &GameConfig,
    ) -> Result<Vec<GameStep>, DomainError>;

    fn evaluate_answer(
        &self,
        step: &GameStep,
        answer: &UserAnswer,
        now: OffsetDateTime,
        config: &GameConfig,
    ) -> Result<StepEvaluation, DomainError>;

    fn finalize(
        &self,
        session: &GameSession,
        config: &GameConfig,
    ) -> Result<GameResult, DomainError>;
}
```

Then register engines in a registry:

```rust
pub struct GameEngineRegistry {
    engines: HashMap<GameKind, Arc<dyn GameEngine>>,
}
```

This gives you extensibility without overengineering.

## Why this is good for Rust learning

It teaches:

- traits
- trait objects
- `Arc<dyn Trait>`
- modular boundaries
- clean engine/domain separation

---

## 10. Example Game Designs

## 10.1 Game A — Basic Gap Fill

### Input
A sentence with one removed word plus candidate words.

Example:

Text:
`Han ___ til butikken i går.`

Candidates:
- gikk
- går
- gått

Correct answer:
- gikk

### Domain rules

- exactly one or more gaps per step, but start with one gap only
- user selects candidate
- evaluation compares answer to expected
- partial scoring possible later

### Step prompt

```rust
pub struct GapFillPrompt {
    pub text_with_gap: String,
    pub choices: Vec<String>,
}
```

### Expected answer

```rust
pub enum ExpectedAnswer {
    ExactText(String),
    Structured(StructuredExpectedAnswer),
}
```

## 10.2 Game B — Morphological Gap Fill

### Input
Same as above, but candidate may require transformation before submission.

Example:

Sentence:
`Jeg ser ___ katt.`

Base candidate:
`svart`

Expected filled form may depend on grammar.

This means answer evaluation may involve:

- exact expected surface form
- acceptable variants
- morphology-aware normalization

### Extra design requirement

Keep the distinction between:

- **base lexical item**
- **surface form expected in context**

```rust
pub struct MorphPrompt {
    pub text_with_gap: String,
    pub base_candidates: Vec<BaseCandidate>,
    pub grammatical_hints: Vec<String>,
}
```

### Evaluation strategy

Possible modes:

- strict string equality
- normalized equality
- morphological equivalence via rule engine or LLM-assisted evaluator

Start with strict + normalization. Add advanced morphology later.

---

## 11. Content Pipeline

Your content pipeline should be explicit.

## Stages

1. fetch learning items
2. filter/select items
3. optionally enrich items
4. optionally call LLM for transformation
5. create prepared content
6. generate game steps

## Suggested abstractions

```rust
pub trait ContentProvider {
    async fn fetch_learning_items(
        &self,
        user_id: UserId,
        request: ContentRequest,
    ) -> Result<Vec<LearningItem>, AppError>;
}
```

This provider may initially fetch from your own PostgreSQL tables or from another internal service.

## LLM augmentation pipeline

Use a separate abstraction:

```rust
pub trait ContentTransformer {
    async fn transform(
        &self,
        items: Vec<LearningItem>,
        template: PromptTemplate,
        context: TransformContext,
    ) -> Result<Vec<LearningItem>, AppError>;
}
```

Later you can chain transformers:

- rule-based normalizer
- LLM sentence regenerator
- distractor generator
- morphology analyzer

---

## 12. Prompt Template / LLM Area

Since you want prompt templates and LLM calls, do not scatter prompts in code.

Create a dedicated module and DB table.

## Prompt template entity

```rust
pub struct PromptTemplate {
    pub id: PromptTemplateId,
    pub name: String,
    pub version: i32,
    pub purpose: PromptPurpose,
    pub template_text: String,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    pub active: bool,
}
```

## Good uses of LLM in this engine

- generate distractors for gap-fill
- rewrite context sentence while preserving meaning
- produce morphology-aware tasks
- classify difficulty
- normalize or enrich hard words

## Important rule

LLM should be used for **preparation**, not for core runtime state correctness.

Why:

- core game runtime must be deterministic
- evaluation must remain understandable and testable
- LLM failures should not break session progression

So:

- prepare content with LLM before game starts
- runtime answer evaluation should mostly be rule-based
- only optionally consult LLM in offline/assistive mode

---

## 13. Scheduling / Time-Limited Steps

You want time-limited steps. There are two main approaches.

## Option A — Passive deadlines
Store deadlines in DB and check them when user interacts.

Pros:

- simple
- robust
- easy to implement

Cons:

- timeout only discovered on next action or polling

## Option B — Background poller
Run a Tokio task that periodically finds overdue active steps and marks them timed out.

Pros:

- more real-time
- UI can reflect actual timeout even without user input

Cons:

- more moving parts

## Recommendation

Implement both in phases:

### Phase 1
Passive deadline checking in submit/advance endpoints.

### Phase 2
Background timeout poller every few seconds.

### Timer model

```rust
pub struct TimingPolicy {
    pub per_step_limit_secs: Option<u32>,
    pub session_limit_secs: Option<u32>,
    pub auto_advance_on_timeout: bool,
}
```

### Scheduler port

```rust
pub trait TimeoutScheduler {
    async fn check_and_apply_timeouts(&self) -> Result<u64, AppError>;
}
```

---

## 14. Scoring Design

Scoring should be strategy-based.

## Base score model

```rust
pub struct Score {
    pub total_points: i32,
    pub earned_points: i32,
    pub accuracy: f32,
}
```

## Per-step evaluation

```rust
pub struct StepEvaluation {
    pub is_correct: bool,
    pub awarded_points: i32,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub explanation: Option<String>,
    pub evaluation_mode: EvaluationMode,
}
```

## Scoring policy

```rust
pub enum ScoringPolicy {
    FixedPerCorrect { points: i32 },
    WeightedByDifficulty,
    TimeAdjusted,
    Composite(CompositeScoringPolicy),
}
```

### Recommendation

Start with:

- fixed points per correct answer
- optional penalty for timeout

Later add:

- faster answer bonus
- difficulty weighting
- streak bonus

---

## 15. PostgreSQL Schema

Keep the schema normalized enough, but not overly fragmented.

## Core tables

### `game_definitions`
Stores game configuration templates.

Fields:

- id UUID PK
- kind TEXT
- version INT
- name TEXT
- config JSONB
- scoring_policy JSONB
- timing_policy JSONB
- created_at TIMESTAMPTZ
- active BOOLEAN

### `game_sessions`
Stores one concrete session.

Fields:

- id UUID PK
- user_id UUID
- definition_id UUID
- state TEXT
- current_step_index INT
- score JSONB
- started_at TIMESTAMPTZ NULL
- completed_at TIMESTAMPTZ NULL
- expires_at TIMESTAMPTZ NULL
- created_at TIMESTAMPTZ
- updated_at TIMESTAMPTZ

### `game_steps`
Stores step runtime state.

Fields:

- id UUID PK
- session_id UUID FK
- ordinal INT
- state TEXT
- prompt JSONB
- expected_answer JSONB
- user_answer JSONB NULL
- evaluation JSONB NULL
- deadline_at TIMESTAMPTZ NULL
- created_at TIMESTAMPTZ
- updated_at TIMESTAMPTZ

### `learning_items`
If you store fetched learning content locally.

Fields:

- id UUID PK
- user_id UUID
- source_text TEXT
- context_text TEXT NULL
- hard_fragment TEXT
- lemma TEXT NULL
- morphology JSONB NULL
- language TEXT
- metadata JSONB
- created_at TIMESTAMPTZ

### `prompt_templates`
Stores prompt templates.

Fields:

- id UUID PK
- name TEXT
- version INT
- purpose TEXT
- template_text TEXT
- input_schema JSONB
- output_schema JSONB
- active BOOLEAN
- created_at TIMESTAMPTZ

### `llm_calls`
For observability and debugging.

Fields:

- id UUID PK
- template_id UUID NULL
- session_id UUID NULL
- request_payload JSONB
- response_payload JSONB
- status TEXT
- latency_ms INT
- created_at TIMESTAMPTZ

### `session_events`
Optional but strongly recommended.

This is very useful.

Fields:

- id UUID PK
- session_id UUID
- event_type TEXT
- payload JSONB
- created_at TIMESTAMPTZ

Examples of events:

- session_created
- session_started
- step_activated
- answer_submitted
- step_evaluated
- step_timed_out
- session_completed

This gives you auditability and debugging without going full event sourcing.

---

## 16. Repository Design

Use ports in the engine layer and implementations in infrastructure.

```rust
#[async_trait::async_trait]
pub trait GameSessionRepository {
    async fn insert(&self, session: &GameSession) -> Result<(), AppError>;
    async fn get(&self, id: GameSessionId) -> Result<GameSession, AppError>;
    async fn update(&self, session: &GameSession) -> Result<(), AppError>;
    async fn find_overdue_sessions(&self, now: OffsetDateTime) -> Result<Vec<GameSession>, AppError>;
}
```

Also:

- `GameDefinitionRepository`
- `LearningItemRepository`
- `PromptTemplateRepository`
- `SessionEventRepository`

---

## 17. HTTP API Design

Keep the API explicit and session-oriented.

## Endpoints

### Health

```text
GET /health
```

### Create a game session

```text
POST /api/v1/game-sessions
```

Request:

```json
{
  "userId": "uuid",
  "gameKind": "gap_fill",
  "definitionId": "optional-uuid",
  "contentRequest": {
    "source": "hard_words",
    "limit": 10,
    "language": "no"
  },
  "options": {
    "stepTimeLimitSecs": 30,
    "llmPreparationEnabled": true
  }
}
```

Response:

```json
{
  "sessionId": "uuid",
  "state": "prepared",
  "currentStepIndex": 0,
  "stepsCount": 10
}
```

### Start session

```text
POST /api/v1/game-sessions/{session_id}/start
```

### Get session state

```text
GET /api/v1/game-sessions/{session_id}
```

Returns current session view including current step.

### Submit answer

```text
POST /api/v1/game-sessions/{session_id}/steps/{step_id}/answer
```

Request:

```json
{
  "answer": {
    "type": "text",
    "value": "gikk"
  }
}
```

Response:

```json
{
  "stepId": "uuid",
  "correct": true,
  "awardedPoints": 10,
  "nextState": "evaluated",
  "currentScore": {
    "earnedPoints": 20,
    "totalPoints": 30,
    "accuracy": 0.6667
  }
}
```

### Advance to next step

```text
POST /api/v1/game-sessions/{session_id}/advance
```

### Get result

```text
GET /api/v1/game-sessions/{session_id}/result
```

### Admin prompt templates

```text
POST /api/v1/admin/prompt-templates
GET /api/v1/admin/prompt-templates
```

### Optional timeout trigger endpoint
Useful in development.

```text
POST /api/v1/admin/timeouts/check
```

---

## 18. Example Runtime Flow

## Game creation flow

1. API receives create session request
2. engine service resolves `GameDefinition`
3. fetches learning items via `ContentProvider`
4. optionally transforms content via LLM/template
5. engine generates `PreparedContent`
6. engine generates `GameStep`s
7. session saved in DB as `Prepared`
8. response returned

## Answer flow

1. API receives answer
2. app service loads session
3. checks timeout before applying answer
4. obtains relevant game engine by `GameKind`
5. engine evaluates answer
6. domain updates current step and score
7. session persisted
8. if last step completed, finalize result
9. return updated session/score

---

## 19. Error Handling Strategy

Since you want to learn Rust properly, keep errors layered.

## Domain errors

For invariant violations and invalid transitions.

```rust
#[derive(thiserror::Error, Debug)]
pub enum DomainError {
    #[error("invalid session transition from {from:?} to {to:?}")]
    InvalidSessionTransition { from: GameSessionState, to: GameSessionState },

    #[error("step already answered")]
    StepAlreadyAnswered,

    #[error("step timed out")]
    StepTimedOut,

    #[error("unsupported game kind")]
    UnsupportedGameKind,
}
```

## Application errors

For use-case orchestration issues.

```rust
#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error(transparent)]
    Domain(#[from] DomainError),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("repository error: {0}")]
    Repository(String),

    #[error("external service error: {0}")]
    External(String),
}
```

## API errors
Map into HTTP status codes.

This is excellent Rust practice.

---

## 20. Docker / Deployment Design

## Dockerfile
Use multi-stage build.

```dockerfile
FROM rust:1.87 as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p app

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/app /usr/local/bin/shakti-game-engine
CMD ["shakti-game-engine"]
```

## docker-compose.yml
Include:

- app
- postgres

Environment:

- `DATABASE_URL`
- `RUST_LOG`
- `APP_PORT`
- `LLM_BASE_URL`
- `LLM_API_KEY`

---

## 21. Testing Strategy

This project is great for learning Rust testing seriously.

## Types of tests

### A. Pure domain unit tests
Test state transitions, scoring, answer evaluation.

Examples:

- cannot submit answer when session not started
- timeout prevents answer acceptance
- correct answer increments score
- morph evaluator normalizes acceptable answers

### B. Repository integration tests
Use test Postgres.

### C. API integration tests
Boot axum app and test endpoints.

### D. Golden tests for LLM prompt parsing
Useful when prompt outputs must match schema.

## Recommendation

Write many domain tests first. That is the best Rust learning path.

---

## 22. Observability

Add observability from the start.

## Logging
Use `tracing` fields:

- session_id
- user_id
- game_kind
- step_id
- transition
- latency_ms

## Metrics to add later

- sessions_created_total
- sessions_completed_total
- step_timeouts_total
- answer_submissions_total
- llm_calls_total
- llm_call_latency_ms
- session_duration_seconds

---

## 23. Security / Safety Notes

Even if auth is not first priority, design for it.

## Minimum

- every session tied to user id
- never expose another user’s sessions
- validate ownership on reads and writes
- sanitize prompt data sent to LLM
- keep LLM prompts and outputs logged carefully

---

## 24. Recommended First Increment

Do not start with everything.

## Version 0.1 scope

Implement only:

- one game kind: `gap_fill`
- one player
- PostgreSQL persistence
- create session
- start session
- submit answer
- advance session
- scoring
- passive timeout checks
- no external content fetch yet; seed learning items in DB
- optional stubbed LLM interface, but no real provider required

This will already teach you a lot.

## Version 0.2

Add:

- `morph_gap_fill`
- background timeout poller
- content provider abstraction backed by real session data
- prompt template persistence
- real LLM integration for distractor generation

## Version 0.3

Add:

- admin endpoints for definitions/templates
- better result analytics
- event log/audit trail
- more game types

---

## 25. Concrete Rust Design Recommendations

Because this project is partly for sharpening Rust skills:

### Use enums aggressively
They are perfect for:

- states
- game kinds
- answer types
- evaluation modes
- errors

### Keep domain methods mostly synchronous and pure
This is important.

Do not make your domain entities depend on async repository access.

### Use traits only at boundaries
Examples:

- repositories
- LLM provider
- content provider
- scheduler
- clock
- game engines

### Do not overuse trait objects internally
For some domain cases, plain enums are simpler and more idiomatic.

### Prefer explicit SQL over heavy ORMs
This is better for learning.

### Keep JSONB where flexibility helps
Use JSONB for:

- prompt payloads
- scoring/timing configs
- step prompt details
- evaluation payloads

But do not store everything as JSONB. Use real columns for key queryable fields.

---

## 26. Suggested Internal Contracts

## Create session use case

```rust
pub struct CreateGameSessionCommand {
    pub user_id: UserId,
    pub game_kind: GameKind,
    pub definition_id: Option<GameDefinitionId>,
    pub content_request: ContentRequest,
    pub options: SessionOptions,
}
```

## Submit answer use case

```rust
pub struct SubmitAnswerCommand {
    pub session_id: GameSessionId,
    pub step_id: GameStepId,
    pub answer: UserAnswer,
}
```

## Game engine registry usage

```rust
let engine = registry.get(definition.kind())?;
let evaluation = engine.evaluate_answer(step, &answer, now, &definition.config)?;
```

---

## 27. Example GameConfig Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameConfig {
    pub steps_count: usize,
    pub distractors_per_step: usize,
    pub allow_skip: bool,
    pub normalization: NormalizationPolicy,
    pub hints_enabled: bool,
}
```

For morph game:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MorphGameConfig {
    pub base: GameConfig,
    pub accept_alternative_forms: bool,
    pub require_explicit_transformation: bool,
}
```

You can either embed variants in one enum config or keep one generic JSON config and parse by game kind.

Recommendation: start with one typed enum:

```rust
pub enum GameConfig {
    GapFill(GapFillConfig),
    MorphGapFill(MorphGapFillConfig),
}
```

That is more type-safe and very Rust-like.

---

## 28. Important Trade-Offs

## Should game rules be fully DB-driven?
Not at first.

Do not try to encode all state transitions and evaluation logic in database rows. That becomes too abstract too early.

Recommended split:

- structural metadata in DB: definitions, versions, timing/scoring config, prompt templates
- executable game logic in Rust code

This is much more maintainable.

## Should LLM be part of answer evaluation?
Prefer no for now.

Use it for content generation/preparation, not authoritative runtime scoring.

## Should you use event sourcing?
No, not first.

Use regular state persistence + session_events table.

---

## 29. Development Order

This is the order I recommend for actual implementation in Cursor.

### Step 1
Scaffold Axum app, config, health endpoint, Postgres connection.

### Step 2
Create domain types for:

- ids
- session state
- step state
- score
- game kind
- game session
- game step

### Step 3
Implement `gap_fill` domain engine with pure unit tests.

### Step 4
Create SQL migrations for:

- game_definitions
- game_sessions
- game_steps
- session_events

### Step 5
Implement repositories using `sqlx`.

### Step 6
Implement create/start/get/answer/advance endpoints.

### Step 7
Add passive timeout handling.

### Step 8
Add content provider abstraction and seed learning items.

### Step 9
Add prompt template model and stub LLM provider.

### Step 10
Add `morph_gap_fill`.

---

## 30. Minimal Initial API Contract for Cursor Implementation

Here is the smallest sensible scope to build first:

### Features

- create one-player gap-fill session from seeded learning items
- start session
- fetch current session
- submit one text answer
- advance to next step
- compute score
- mark timeout if deadline passed

### Non-features for first iteration

- multiplayer
- websocket updates
- real auth
- real LLM integration
- dynamic DB-defined rules engine
- advanced morphology evaluator

This is enough to produce a real working engine.

---

## 31. Final Recommendation

The best design for `shakti-game-engine` is:

- **modular monolith** in Rust
- **Axum + SQLx + PostgreSQL**
- **trait-based game engine registry** for multiple game types
- **pure domain state machine** for session and step transitions
- **explicit scoring and timing policies**
- **LLM only in preparation/enrichment pipeline**
- **PostgreSQL persistence with session/step tables plus event log**
- **incremental rollout starting with one deterministic gap-fill game**

This design is strong enough to become a real product backend, but still small and explicit enough to be an excellent Rust learning platform.

---

## 32. Copy-Paste Build Brief for Cursor

You can give Cursor the following:

> Build a Rust service called `shakti-game-engine` as a modular monolith using Axum, Tokio, SQLx, Serde, Uuid, Thiserror, and Tracing. The service runs in Docker, uses PostgreSQL, and exposes an HTTP API for a web UI.
>
> Core domain:
> - one-player game sessions
> - multiple game kinds via trait-based engine registry
> - game session + game step state machines
> - scoring and timing policies
> - deterministic runtime evaluation
> - optional LLM-based content preparation before play
>
> First implementation scope:
> - health endpoint
> - create/start/get/answer/advance game session
> - one game kind: gap_fill
> - PostgreSQL persistence for game_definitions, game_sessions, game_steps, session_events
> - passive step timeout validation
> - clean layered structure: api / engine / domain / infrastructure
> - strong unit tests for domain transitions and answer evaluation
>
> Design rules:
> - domain layer must stay mostly pure and synchronous
> - repositories and providers are traits in engine layer
> - SQLx repositories in infrastructure
> - use enums for states, errors, game kinds, answer types
> - keep prompts/LLM integration in separate modules
> - prepare the code structure so `morph_gap_fill` can be added later

---

## 33. Next Document You Should Create After This

After this design, the next best artifact is a **technical implementation spec** with:

- exact Rust module skeletons
- first SQL migrations
- API request/response structs
- domain enum/type definitions
- first `gap_fill` engine implementation skeleton
- test list

That would be the ideal handoff for Cursor to start coding with minimal ambiguity.

