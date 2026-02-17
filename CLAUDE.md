# claude-app

Rust/Dioxus desktop app wrapping Claude Code CLI with multi-session chat and agent orchestrator.

## Claude CLI Permission Modes

When spawning Claude CLI with `--permission-mode`:

- `plan` — Forces the model into plan-only output. It cannot emit normal messages, only plans. Do NOT use for agents that need free-form output.
- `dontAsk` — Auto-approves all tool uses silently (does NOT deny them). Equivalent to "don't ask the user, just do it."
- `acceptEdits` — Auto-accepts file edits, asks for bash commands.
- `bypassPermissions` — Skips all permission checks entirely.
- `default` — Asks the user for permission (hangs in non-interactive `-p` mode).

For non-developer agents (manager, architect, scorer): rely on bwrap read-only sandbox as the hard security boundary, not permission modes. Use `bypassPermissions` since bwrap prevents writes anyway and the agent needs free-form output.

For developer agents: `acceptEdits` + bwrap with writable worktree.

## REST API

Axum-based REST API for programmatic access to sessions and orchestrator runs. Runs on a separate tokio runtime thread alongside the Dioxus UI.

### Config

- **Port**: `CLAUDE_APP_PORT` env var (default: 3100)
- **Secret**: `CLAUDE_APP_SECRET` env var (or auto-generated on startup and logged)

### Auth

All endpoints except `/api/auth` require JWT Bearer token.

```bash
TOKEN=$(curl -s localhost:3100/api/auth -d '{"secret":"..."}' | jq -r .token)
curl -H "Authorization: Bearer $TOKEN" localhost:3100/api/sessions
```

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/auth` | Get JWT token |
| GET | `/api/sessions` | List sessions |
| POST | `/api/sessions` | Create session |
| GET | `/api/sessions/:id` | Get session + messages |
| DELETE | `/api/sessions/:id` | Remove session |
| POST | `/api/sessions/:id/prompt` | Send prompt (SSE stream) |
| POST | `/api/sessions/:id/abort` | Abort running session |
| GET | `/api/runs` | List orchestrator runs |
| POST | `/api/runs` | Create run (spawns agents) |
| GET | `/api/runs/:id` | Get run status + agent sessions |
| POST | `/api/runs/:id/abort` | Abort run |
| POST | `/api/runs/:id/agents/:agent/message` | Send message to agent |
| GET | `/api/runs/:id/stream` | SSE stream of all agent output |

Agent names for messaging: `manager`, `architect`, `scorer`, `developer-0`, `developer-1`, `developer-2`.

### Source Layout

```
src/api/
  mod.rs        -- Router setup, CORS, middleware, server start
  state.rs      -- AppState (sessions, manager, runs, project_path, jwt_secret)
  auth.rs       -- JWT generation/validation, auth middleware
  types.rs      -- Request/response JSON types
  sessions.rs   -- Session CRUD + prompt SSE endpoint
  runs.rs       -- Orchestrator run endpoints + agent messaging
```
