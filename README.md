# shakti-game-engine

Rust service that runs language-learning **gap-fill** game sessions: state machine, scoring, PostgreSQL persistence, and JSON HTTP API. It is **standalone** (own database and Docker Compose) and can be wired into the wider Shakti stack later.

## Run locally (Rust)

1. Start PostgreSQL (or use Docker Compose for only the DB):

   ```bash
   docker compose up -d shakti-game-db
   ```

2. Export env and migrate (the binary runs migrations on startup):

   ```bash
   export DATABASE_URL=postgresql://game:game@127.0.0.1:5435/shakti_game
   export RUST_LOG=info
   cargo run -p shakti-game-engine
   ```

## Run with Docker Compose (app + Postgres)

```bash
docker compose up --build
```

- API: `http://127.0.0.1:8010`
- Health: `GET /health`, readiness + DB: `GET /ready`

### OpenAI (real LLM for gap-fill)

**CI / automated integration tests** use **`GAME_ENGINE_LLM_MODE=mock`** (deterministic passage stitched from DB or inline inputs). **Manual play UI** checks against a **real model** use **`openai`** plus a key: the model is instructed to write an **original** passage in the session language (not a copy of your source texts).

1. Create a single-line file **`openai.key.secret`** in this repo root (the file is gitignored) containing your API key, or set **`OPENAI_API_KEY`** in the environment.
2. Optional: override file path with **`OPENAI_KEY_FILE`** (default `openai.key.secret`, relative to the process working directory — `/app` in the container).
3. Set **`OPENAI_MODEL`** if needed (default in compose: `gpt-5.2`).
4. **`GAME_ENGINE_LLM_MODE`**: `mock` (default in compose), `openai` for live calls, or omit when running the binary locally so that if a key is present the engine picks OpenAI automatically.
5. For Docker, `docker-compose.yml` bind-mounts `./openai.key.secret` → `/app/openai.key.secret`. Create that file in the repo root before `GAME_ENGINE_LLM_MODE=openai docker compose up --build`. If the file is missing, compose may fail to start the engine container; use mock mode or add a placeholder line for local-only dev.

The app logs structured **`openai_key_source`** as `none`, `env`, or `file` at startup (never logs the key).

### Inline LLM inputs (play UI / API)

`contentRequest` may include **`llmSourceTexts`** (non-empty strings → skip DB learning rows; requires **`language`**) and **`llmHardWords`** (non-empty list → skip DB vocabulary for the prompt). See the **shakti-game-play-ui** client.

### Developer gap solution (play UI)

When **`GAME_ENGINE_DEV_EXPOSE_GAP_SOLUTION`** is `1`, `true`, or `yes`, session/step JSON includes **`devGapSolution`** (array of strings, gap index order) on gap-fill steps so **shakti-game-play-ui** can show a developer hint panel. **Keep this off in production** (`false` or unset); `docker-compose.yml` defaults it to `true` for local testing only.

### Troubleshooting: `VersionMismatch(20250401000001)` (or similar)

SQLx stores a **checksum** per applied migration in `_sqlx_migrations`. This error means the migration **file on disk** (baked into the binary) no longer matches the checksum recorded when that version first ran—usually because an old `migrations/*.sql` was **edited after it was already applied**, while the **Postgres Docker volume** still holds the old checksum.

**Local dev (safe to wipe data):** reset the database volume and start again:

```bash
docker compose down -v
docker compose up --build
```

**Rule of thumb:** never change the contents of a migration that has already run in an environment you care about; add a **new** migration file instead. For shared/prod databases, coordinate checksum fixes with your DB team (avoid ad-hoc edits to `_sqlx_migrations`).

## Seeded dev user

Migrations seed sample `learning_items` for user id:

`11111111-1111-1111-1111-111111111111`

## Browser play UI (optional)

A small **standalone** Vite + TypeScript client for manual testing lives in the sibling project **`shakti-game-play-ui`** (same parent directory as this repo). Run the engine (`docker compose up`), then `npm run dev` in that UI; it proxies `/api` to `http://127.0.0.1:8010` by default. See that project’s README for env vars and production builds.

## Example API flow

Optional request header `X-Trace-Id` selects the correlation id; if omitted, the server generates one. **Every response** echoes the effective id in the **`X-Trace-Id` response header** (not in JSON bodies). For browser clients, expose that header via CORS (`Access-Control-Expose-Headers`) if the UI needs to read it.

Create session (camelCase JSON):

```bash
curl -s -X POST http://127.0.0.1:8010/api/v1/game-sessions \
  -H 'Content-Type: application/json' \
  -H 'X-Trace-Id: my-trace-1' \
  -d '{
    "userId": "11111111-1111-1111-1111-111111111111",
    "gameKind": "gap_fill",
    "contentRequest": { "source": "hard_words", "limit": 5, "language": "no" },
    "options": { "stepTimeLimitSecs": 60 }
  }'
```

Start session (replace `SESSION_ID`):

```bash
curl -s -X POST http://127.0.0.1:8010/api/v1/game-sessions/SESSION_ID/start \
  -H 'Content-Type: application/json' \
  -d '{"userId":"11111111-1111-1111-1111-111111111111"}'
```

Get session:

```bash
curl -s 'http://127.0.0.1:8010/api/v1/game-sessions/SESSION_ID?userId=11111111-1111-1111-1111-111111111111'
```

Submit answer for passage gap-fill (replace `STEP_ID` from session view; `selections` in gap order, ordinal 0…n−1):

```bash
curl -s -X POST http://127.0.0.1:8010/api/v1/game-sessions/SESSION_ID/steps/STEP_ID/answer \
  -H 'Content-Type: application/json' \
  -d '{"userId":"11111111-1111-1111-1111-111111111111","answer":{"type":"gap_fill_slots","selections":["gikk","svart","regner","leser","hjem"]}}'
```

Result (after session finished):

```bash
curl -s 'http://127.0.0.1:8010/api/v1/game-sessions/SESSION_ID/result?userId=11111111-1111-1111-1111-111111111111'
```

## Project layout

- `crates/domain` — pure model, gap-fill engine, unit tests
- `crates/engine` — use cases and repository ports (package `shakti-game-engine-core`)
- `crates/infrastructure` — SQLx PostgreSQL adapters
- `crates/api` — Axum routes; `X-Trace-Id` on requests and mirrored on all responses; span field `trace_id` for structured logs
- `crates/app` — HTTP server binary (`shakti-game-engine`), config, migrations on startup; avoids name clash with `crates/engine`
- `migrations/` — SQL schema + seed data

Design reference: [shakti_game_engine_design.md](./shakti_game_engine_design.md).

## Shakti monorepo compose (optional)

When this repository sits next to `shakti-deployment`, you can enable the optional Compose profile `game-engine` so the service joins `shakti-network` and uses a dedicated `shakti_game` database on `shakti-db`. See `shakti-deployment/docker-compose.yml`.
