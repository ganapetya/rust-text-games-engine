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

## Seeded dev user

Migrations seed sample `learning_items` for user id:

`11111111-1111-1111-1111-111111111111`

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

Submit answer (replace `STEP_ID` from session view):

```bash
curl -s -X POST http://127.0.0.1:8010/api/v1/game-sessions/SESSION_ID/steps/STEP_ID/answer \
  -H 'Content-Type: application/json' \
  -d '{"userId":"11111111-1111-1111-1111-111111111111","answer":{"type":"text","value":"gikk"}}'
```

Advance to next step:

```bash
curl -s -X POST http://127.0.0.1:8010/api/v1/game-sessions/SESSION_ID/advance \
  -H 'Content-Type: application/json' \
  -d '{"userId":"11111111-1111-1111-1111-111111111111"}'
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
