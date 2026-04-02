# Game session and step states

This document describes **all** `GameSessionState` and `StepState` values in **shakti-game-engine**, how they appear in running code today, and the **implemented** transition rules.

Source of truth for the enums:

- `crates/domain/src/game_session.rs` — `GameSessionState`
- `crates/domain/src/game_step.rs` — `StepState`
- `crates/domain/src/game_session.rs` — transition logic (`start`, `check_session_expired`, `timeout_current_step`, `record_evaluation`, `advance`)

API / persistence wire these enums as **snake_case** in JSON where `rename_all = "snake_case"` applies (see domain types).

---

## `GameSessionState`

| Variant | Role today |
|--------|------------|
 **`Prepared`** | Default when `GameSession::new` runs after steps are built. This is what session creation persists. |
| **`InProgress`** | Set by `GameSession::start(now)` — only allowed from **Prepared**. Also activates the first step and sets per-step deadline. |
| **`Completed`** | Set when the last step is finished via evaluation, advance-after-eval, or step-timeout on the last step. Sets `completed_at`. |
| **`TimedOut`** | Set by `check_session_expired` when the session is **InProgress** and `now >= expires_at` (session limit from the game definition). Sets `completed_at` and may set the current step to **TimedOut** if it was **Active** or **Pending**. |
| **`Draft`** | Exists on the enum and in DB mapping — **no domain or service assigns it** in the current codebase. |
| **`Cancelled`** | Exists on the enum and in DB mapping; `record_evaluation` refuses work if the session is **Cancelled**. **No domain or service assigns it** today. |

### Session state machine (implemented transitions)

```mermaid
stateDiagram-v2
  [*] --> Prepared: GameSession::new
  Prepared --> InProgress: start() when session is Prepared
  InProgress --> Completed: last step finished (evaluate / advance / step-timeout path)
  InProgress --> TimedOut: check_session_expired, now >= expires_at
  note right of Draft: Enum only today — no transition into it in code
  note right of Cancelled: Enum only today — no transition into it in code
```

### App layer entry points

| Flow | Behavior |
|------|----------|
| **Start** (`start_game_session`) | Load session; require **Prepared**; call `session.start(now)`; persist. |
| **Get** (`get_game_session`) | Call `check_session_expired`; if session became **TimedOut**, persist. |
| **Submit answer** (`submit_answer`) | `check_session_expired`; reject if finished; on per-step timeout, `timeout_current_step` and persist; else evaluate and `record_evaluation`. |
| **Advance** (`advance_session`) | `check_session_expired`; then `session.advance(now)`; persist. |

---

## `StepState`

| Variant | Role today |
|--------|------------|
| **`Pending`** | Every step when created in `GapFillEngine::generate_steps`. Non-current steps stay **Pending** until the session moves to them. |
| **`Active`** | The current step after `start`, after `advance` to the next step, or after `timeout_current_step` advances past a non-last step. Requires session **InProgress**. |
| **`Evaluated`** | Set by `record_evaluation` when the session is **InProgress**, index matches current step, step is **Active**, and the step is not past its per-step deadline. |
| **`TimedOut`** | Set by `timeout_current_step` from **Active**, or by `check_session_expired` for the current step when the **session** hits `expires_at` while the step is **Active** or **Pending**. |
| **`Answered`** | On the enum and in persistence — **never assigned** in domain code (submission records evaluation in one step: **Active → Evaluated**). |
| **`Skipped`** | `advance` treats **Skipped** like a finished step, but **nothing sets Skipped** yet (reserved for skip flows, e.g. `allow_skip`). |

### Step state machine (implemented transitions)

```mermaid
stateDiagram-v2
  Pending --> Active: session start OR advance to this step
  Active --> Evaluated: record_evaluation (submit_answer path)
  Active --> TimedOut: timeout_current_step OR session-level timeout on current step
  Pending --> TimedOut: session-level timeout while step is still Pending at current index
  note right of Answered: Not used by current code paths
  note right of Skipped: advance() accepts it as finished; no producer yet
```

---

## Coupling and rules

- **Per-step deadline:** `is_step_timed_out` applies when the step is **Active** and `now > deadline_at`. Submitting an answer in that window leads to `timeout_current_step` instead of evaluation.
- **After evaluation:** The session stays **InProgress** until `advance`, unless `record_evaluation` was on the **last** step — then the session becomes **Completed** immediately.
- **`advance`:** Requires **InProgress** and current step **Evaluated** or **Skipped**; then either **Completed** (last step) or increment `current_step_index` and set the next step **Active** with a new deadline.

---

## Operation quick reference

| Operation | Session must be | Current step | Main effect |
|-----------|-----------------|--------------|-------------|
| `start` | **Prepared** | N/A (first becomes current) | **InProgress**; step[0] **Active** |
| `record_evaluation` | **InProgress** | index = current; **Active**; not per-step timed out | **Evaluated**; **Completed** if last |
| `advance` | **InProgress** | **Evaluated** or **Skipped** | Next **Active** or **Completed** |
| `timeout_current_step` | **InProgress** | **Active** | **TimedOut**; **Completed** or next **Active** |
| `check_session_expired` | **InProgress** | optional | If past `expires_at`: **TimedOut**; current step may become **TimedOut** |

---

## Summary

The type system includes **Draft**, **Cancelled**, **Answered**, and **Skipped** for forward compatibility and persistence. The **live** state machine today is:

- **Session:** **Prepared → InProgress → (Completed | TimedOut)**.
- **Step (current):** **Pending → Active → (Evaluated | TimedOut)**, with **advance** moving from **Evaluated** (or future **Skipped**) to the next **Active** step.

When adding features (draft sessions, cancel, skip, or a distinct “answered but not scored” phase), extend the domain methods that already centralize transitions in `GameSession` so invalid jumps stay impossible by construction.
