# claude-app

Dioxus desktop frontend for agent-orchestrator. Watches JSONL log files and sends messages to agents via Unix socket IPC.

## Architecture

Single Dioxus desktop runtime (no API server). Discovers projects by scanning `~/.local/share/agent-orchestrator/` for directories with `logs/*.jsonl` files. Live-updates via `notify` file watcher. Sends messages to agents via `peercred-ipc` control socket.

```
src/
  main.rs              -- Entry point: launches Dioxus desktop
  state.rs             -- Project, ChatMessage, TokenUsage, load_projects(), parse_jsonl_from_offset()
  watcher.rs           -- notify v7 file watcher (ProjectsChanged, JsonlChanged events)
  ipc.rs               -- peercred-ipc Client wrapper (send_message, start_task, get_status)
  ui/
    mod.rs             -- App root, signals, watcher bridge, selection effects
    sidebar.rs         -- Project→Agent tree navigation
    chat.rs            -- ChatPanel, AgentHeader (token bar), MessageList, IPC send
    prompt.rs          -- PromptInput (Enter/Shift+Enter)
    message.rs         -- Render ChatMessage (user/assistant with timestamps + usage)
    diff.rs            -- Syntax-highlighted diff blocks and code blocks (syntect)
```

## Data Flow

1. `watcher.rs` watches `~/.local/share/agent-orchestrator/` recursively via inotify
2. Events bridged to Dioxus via std mpsc → tokio mpsc → 200ms poll loop
3. `ProjectsChanged` → re-scan project directories
4. `JsonlChanged(path)` → incremental JSONL parse from last offset
5. `session_reset` entries clear message history
6. User prompt → `ipc::send_message()` via `spawn_blocking` → peercred-ipc `Client::call()`

## Control Socket Protocol

Communicates with agent-orchestrator via `/tmp/claude/orchestrator/control.sock` (peercred-ipc msgpack):

```rust
enum ControlRequest {
    SendMessage { to: String, content: String },
    StartTask { task: String },
    Abort,
    Status,
}

enum ControlResponse {
    Ok,
    Error { message: String },
    Status { agents: Vec<AgentStatus>, project: String },
}
```

## JSONL Log Format

Files at `~/.local/share/agent-orchestrator/{project}/logs/{agent}.jsonl`:

```json
{"type":"user","text":"...","timestamp":"2026-03-03T14:30:00Z"}
{"type":"assistant","text":"...","timestamp":"...","usage":{"input":100,"output":200,"cache_read":0,"cache_creation":0}}
{"type":"session_reset"}
```

## Testing

```bash
cargo test -p claude-app
```

Tests are inline `#[cfg(test)]` in `diff.rs` (syntax highlighting).

## Dependencies

Key deps: dioxus 0.6 (desktop UI), tokio (async), notify 7 (file watching), llm-sdk (LogEntry/LogUsage types), peercred-ipc (Unix socket IPC), syntect 5 (syntax highlighting), serde/serde_json, tracing, dirs.

## Environment Variables

- `RUST_LOG` — Tracing filter
