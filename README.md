# claude-app

Desktop application for multi-session Claude Code chat with an agent orchestrator. Built with Rust, Dioxus, and Axum.

## What It Does

- **Multi-session chat** — Run multiple Claude Code sessions side-by-side, each with its own git worktree
- **Agent orchestrator** — Coordinate multiple Claude instances (manager, architect, developer, scorer) to work on tasks collaboratively
- **REST API** — Programmatic access to sessions and orchestrator runs via HTTP
- **Sandboxed execution** — Developer agents run in bwrap containers with only their worktree writable

## Requirements

- Rust (edition 2024, rustc 1.85+)
- [Claude Code CLI](https://docs.anthropic.com/en/docs/claude-code) installed and authenticated
- [bwrap](https://github.com/containers/bubblewrap) for agent sandboxing
- Linux (bwrap and Dioxus desktop depend on it)

## Building

```bash
cargo build --release
```

## Running

```bash
cargo run --release
```

The app opens a desktop window. Pick a project directory, then create sessions or orchestrator runs from the sidebar.

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `CLAUDE_APP_PORT` | `3100` | REST API port |
| `CLAUDE_APP_SECRET` | auto-generated | JWT secret for API auth (logged on startup if generated) |
| `RUST_LOG` | — | Tracing filter (e.g. `info`, `claude_app=debug`) |

## REST API

The API server starts automatically alongside the UI on port 3100.

```bash
# Authenticate
TOKEN=$(curl -s localhost:3100/api/auth \
  -H 'content-type: application/json' \
  -d '{"secret":"<your-secret>"}' | jq -r .token)

# List sessions
curl -H "Authorization: Bearer $TOKEN" localhost:3100/api/sessions

# Create an orchestrator run
curl -X POST -H "Authorization: Bearer $TOKEN" localhost:3100/api/runs

# Send a message to an agent
curl -X POST -H "Authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' \
  -d '{"text":"Implement the login page"}' \
  localhost:3100/api/runs/<run-id>/agents/manager/message
```

### Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/auth` | Get JWT token |
| GET | `/api/sessions` | List sessions |
| POST | `/api/sessions` | Create session (spawns worktree) |
| GET | `/api/sessions/{id}` | Get session with messages |
| DELETE | `/api/sessions/{id}` | Remove session and worktree |
| POST | `/api/sessions/{id}/prompt` | Send prompt (SSE response) |
| POST | `/api/sessions/{id}/abort` | Abort running session |
| GET | `/api/runs` | List orchestrator runs |
| POST | `/api/runs` | Create run (spawns agents) |
| GET | `/api/runs/{id}` | Get run detail |
| POST | `/api/runs/{id}/abort` | Abort all agents |
| POST | `/api/runs/{id}/agents/{agent}/message` | Message an agent |
| GET | `/api/runs/{id}/stream` | SSE stream of agent output |

Agent names: `manager`, `architect`, `scorer`, `developer-0`, `developer-1`, `developer-2`.

## Orchestrator

The orchestrator spawns four types of agents that communicate via structured message sections:

| Role | Job | Sandbox |
|------|-----|---------|
| **Manager** | Decomposes goals into tasks, assigns to developers | Read-only bwrap |
| **Architect** | Reviews task plans before developers execute | Read-only bwrap |
| **Developer** | Implements tasks in isolated git worktrees (1-3 instances) | Writable worktree in bwrap |
| **Scorer** | Monitors progress, can fire and replace the manager | Read-only bwrap |

The manager can request 1-3 developers via `CREW:N`. The scorer can fire a stuck manager via `RELIEVE:reason`, which spawns a replacement briefed on current state.

## Project Structure

```
src/
  main.rs           Entry point: spawns API thread, launches Dioxus UI
  api/              REST API (Axum): auth, sessions, runs, SSE streaming
  claude/           Claude CLI process management: spawn, I/O, protocol
  orchestrator/     Agent orchestration: spawn, routing, parsing, commands
  worktree/         Git worktree lifecycle
  sandbox/          bwrap command builders
  state/            Shared state types (Session, Run, Message)
  ui/               Dioxus components (sidebar, chat, messages, diff)
prompts/            System prompts per agent role
assets/             CSS
```

## Testing

```bash
cargo test -p claude-app
```

API tests use `tower::ServiceExt::oneshot()` with pre-populated state — no git worktrees or Claude CLI needed.
