# claude-app

Rust/Dioxus desktop app wrapping Claude Code CLI with multi-session chat and agent orchestrator.

## Architecture

Two runtimes: Dioxus desktop UI (main thread) and Axum REST API (background tokio runtime on port 3100). Shared state via `Arc<AppState>` with `RwLock`/`Mutex`.

```
src/
  main.rs              -- Entry point: spawns API thread, launches Dioxus
  api/
    mod.rs             -- build_router(), start_server(), route definitions
    state.rs           -- AppState (sessions, manager, runs, project_path, jwt_secret)
    auth.rs            -- JWT login + auth middleware
    types.rs           -- Request/response JSON types
    sessions.rs        -- Session CRUD + prompt SSE
    runs.rs            -- Run CRUD + agent messaging + output SSE
    tests.rs           -- Handler-level tests (tower::oneshot)
  claude/
    mod.rs             -- Module exports
    protocol.rs        -- stream-json types (ClaudeInput, ClaudeOutput, ContentBlock)
    process.rs         -- Spawn Claude CLI, stdin/stdout relay
    session.rs         -- SessionManager: process lifecycle, message relay
  orchestrator/
    mod.rs             -- OrchestratorRuntime, RunHandle, agent spawn/kill
    types.rs           -- AgentId, AgentRole, AgentMessage, MessageKind, RuntimeCommand
    agent.rs           -- Agent loop: inbox → prompt → Claude process → parse → route
    roles.rs           -- System prompts and permission modes per role
    routing.rs         -- Section prefix → target agent routing table
    parser.rs          -- Extract structured sections from agent output
  worktree/
    mod.rs             -- Git worktree create/remove/reset
  sandbox/
    mod.rs             -- bwrap command builders (read-only, developer)
  state/
    mod.rs             -- Session, SessionId, Message, SessionStatus
    orchestrator.rs    -- RunId, RunStatus, OrchestratorRun
  ui/
    mod.rs             -- App root, ProjectPicker, signal context providers
    sidebar.rs         -- Session list, orchestrator section
    chat.rs            -- ChatFeed, MessageList
    prompt.rs          -- PromptInput (Enter/Shift+Enter)
    message.rs         -- Message rendering per variant
    diff.rs            -- Syntax-highlighted diff blocks
```

## Orchestrator

Four agent roles communicate via in-process mpsc channels:

| Role | Sandbox | Permission Mode | Purpose |
|------|---------|----------------|---------|
| Manager | read-only bwrap | bypassPermissions | Task decomposition |
| Architect | read-only bwrap | bypassPermissions | Task validation |
| Developer | bwrap + writable worktree | acceptEdits | Implementation |
| Scorer | read-only bwrap | bypassPermissions | Progress monitoring |

Message routing (section prefix → action):
- `TASK:` → Architect (TaskAssignment)
- `APPROVED:devN` → Developer-N (TaskAssignment)
- `REJECTED:` → Manager (ArchitectReview)
- `COMPLETE:` / `BLOCKED:` → Manager (TaskComplete/TaskGiveUp)
- `CREW:N` → Runtime: SetCrewSize(1-3)
- `RELIEVE:reason` → Runtime: fire and replace manager

## Claude CLI Permission Modes

- `plan` — Plan-only output, no normal messages. Don't use for agents needing free-form output.
- `dontAsk` — Auto-approves all tool uses silently ("don't ask, just do it").
- `acceptEdits` — Auto-accepts file edits, asks for bash.
- `bypassPermissions` — Skips all permission checks.
- `default` — Asks user (hangs in non-interactive `-p` mode).

## Worktrees and Sandboxing

All sessions and developers get isolated git worktrees under `~/.claude-sessions/worktrees/<project-hash>/`. Developer agents run in bwrap with only their worktree writable. Non-developer agents get read-only bwrap.

## REST API

All endpoints except `/api/auth` require JWT Bearer token.

```bash
TOKEN=$(curl -s localhost:3100/api/auth -d '{"secret":"..."}' | jq -r .token)
curl -H "Authorization: Bearer $TOKEN" localhost:3100/api/sessions
```

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/auth` | Get JWT token |
| GET | `/api/sessions` | List sessions |
| POST | `/api/sessions` | Create session |
| GET | `/api/sessions/{id}` | Get session + messages |
| DELETE | `/api/sessions/{id}` | Remove session |
| POST | `/api/sessions/{id}/prompt` | Send prompt (SSE stream) |
| POST | `/api/sessions/{id}/abort` | Abort running session |
| GET | `/api/runs` | List runs |
| POST | `/api/runs` | Create run (spawns agents) |
| GET | `/api/runs/{id}` | Get run detail |
| POST | `/api/runs/{id}/abort` | Abort run |
| POST | `/api/runs/{id}/agents/{agent}/message` | Message agent |
| GET | `/api/runs/{id}/stream` | SSE stream of agent output |

Agent names: `manager`, `architect`, `scorer`, `developer-0` through `developer-2`.

## Environment Variables

- `CLAUDE_APP_PORT` — API port (default: 3100)
- `CLAUDE_APP_SECRET` — JWT secret (auto-generated and logged if unset)
- `RUST_LOG` — Tracing filter

## Testing

```bash
cargo test -p claude-app
```

API tests use `tower::ServiceExt::oneshot()` with a `TestApp` harness that pre-populates `AppState` directly — no git worktrees or Claude CLI needed. `RunHandle::new_test(agent_ids)` creates handles with fresh channels for verification.

Tests live in `src/api/tests.rs` (handler-level) and inline `#[cfg(test)]` modules in `routing.rs`, `parser.rs`, `sandbox/mod.rs`, and `runs.rs`.

## Dependencies

pdfium is NOT used here. Key deps: dioxus 0.6 (desktop UI), axum 0.8 (API), tokio (async), jsonwebtoken 9 (auth), syntect 5 (syntax highlighting), tokio-stream (SSE).
