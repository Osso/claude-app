# Switch Orchestrator to MCP Tool Calls

## Context

Agent communication currently uses text prefix parsing (`TASK:`, `APPROVED:`, `COMPLETE:`, etc.) from Claude's free-form output. This is fragile — depends on Claude formatting correctly, needs synthetic fallbacks when parsing fails, and routing logic is interleaved with text manipulation.

Switching to MCP tool calls gives structured, schema-validated communication. Claude calls tools like `send_task()` or `task_complete()` during execution, and the MCP server routes them to the right agent. No parsing ambiguity, no fallback hacks.

## Approach

**Stdio MCP binary** (`src/bin/orchestrator-mcp.rs`) in the same crate. Spawned by each agent's Claude CLI as an MCP server subprocess. The binary speaks MCP JSON-RPC on stdin/stdout and forwards tool calls to the existing Axum API via HTTP.

Each agent's Claude CLI gets: `--mcp-config '{"orchestrator":{"command":"orchestrator-mcp","args":["--agent-id","manager",...]}}' --strict-mcp-config`

## Files to Modify

### 1. `Cargo.toml` — add deps

Add `reqwest` (HTTP client for MCP binary) and `clap` (arg parsing for MCP binary):

```toml
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }
clap = { version = "4", features = ["derive"] }
```

Add binary target:

```toml
[[bin]]
name = "orchestrator-mcp"
path = "src/bin/orchestrator-mcp.rs"

[[bin]]
name = "claude-app"
path = "src/main.rs"
```

### 2. `src/bin/orchestrator-mcp.rs` — new file (~200 lines)

Small synchronous binary:
- Parse CLI args: `--agent-id`, `--run-id`, `--api-url`, `--token`
- Read JSON-RPC from stdin line by line, write responses to stdout
- Handle `initialize` → return capabilities
- Handle `tools/list` → return tools based on agent role (derived from agent-id)
- Handle `tools/call` → POST to `{api_url}/api/runs/{run_id}/mcp/tool-call` with `{agent_id, tool_name, arguments}`, return result text

Uses `reqwest::blocking::Client` (synchronous — the MCP loop is stdin/stdout, no async needed). Token passed as `Authorization: Bearer {token}` header.

**Tools per role:**

| Role | Tool | Parameters | Routes to |
|------|------|-----------|-----------|
| Manager | `send_task` | `description: string` | Architect (TaskAssignment) |
| Manager | `set_crew_size` | `count: integer (1-3)` | RuntimeCommand::SetCrewSize |
| Manager | `approve_task` | `developer: string, description: string` | Developer (TaskAssignment) |
| Architect | `approve_task` | `developer: string, description: string` | Developer (TaskAssignment) |
| Architect | `reject_task` | `reason: string` | Manager (ArchitectReview) |
| Architect | `interrupt_developer` | `developer: string, reason: string` | Developer (Interrupt) |
| Developer | `task_complete` | `summary: string` | Manager (TaskComplete) |
| Developer | `task_blocked` | `reason: string` | Manager (TaskGiveUp) |
| Scorer | `evaluate` | `assessment: string` | Logged only |
| Scorer | `relieve_manager` | `reason: string` | RuntimeCommand::RelieveManager |

### 3. `src/api/state.rs` — add `api_token` and `api_port`

```rust
pub struct AppState {
    // ... existing ...
    pub api_token: String,  // pre-signed JWT for internal MCP calls
    pub api_port: u16,
}
```

### 4. `src/api/mod.rs` — sign token at startup, store port

In `start_server`, sign a long-lived internal JWT token (30-day expiry) using the same `jwt_secret`, store in AppState alongside `api_port`.

### 5. `src/api/runs.rs` — add MCP tool-call endpoint + dispatch logic

New handler `mcp_tool_call`:
- `POST /api/runs/{id}/mcp/tool-call`
- Body: `{ agent_id: String, tool_name: String, arguments: Value }`
- Response: `{ result: String }`
- Validates role + tool combination via match statement
- Delivers messages directly via `RunHandle::send_message()` (new method)
- Sends runtime commands via `RunHandle::send_runtime_command()` (new method)

New `RunHandle` methods:
- `send_message(msg: AgentMessage) -> bool` — looks up target inbox, calls `try_send`
- `send_runtime_command(cmd: RuntimeCommand) -> bool` — sends to runtime via new channel

### 6. `src/orchestrator/mod.rs` — add runtime command channel, simplify run_loop

Add `ext_cmd_tx/ext_cmd_rx` channel pair:
- `ext_cmd_tx: mpsc::Sender<RuntimeCommand>` stored in `RunHandle`
- `ext_cmd_rx: mpsc::Receiver<RuntimeCommand>` in `OrchestratorRuntime`
- `run_loop` adds `ext_cmd_rx` as a `select!` arm alongside `abort_rx`

Remove `outgoing_tx/outgoing_rx` — agents no longer send output through the runtime. The run_loop simplifies to just handling external commands and abort.

Update `spawn_run` signature:
```rust
pub async fn spawn_run(project_path: PathBuf, api_url: String, api_token: String) -> Result<RunHandle>
```

Pass `api_url` and `api_token` down to agent spawn so MCP config can be built.

Update `spawn_agent` — stop passing `outgoing_tx`. Pass MCP config string instead.

Update `new_test` — add dummy `ext_cmd_tx` channel, remove outgoing channel.

### 7. `src/orchestrator/agent.rs` — remove parsing, add MCP config

Remove:
- `use super::parser::extract_sections`
- `use super::routing::{ParsedOutput, route_sections}`
- `outgoing_tx` field from `Agent` struct and constructor
- `dispatch_parsed` function
- Synthetic TaskComplete fallback
- All text collection (`all_text`) in `consume_output` — keep only session_id tracking

Add `mcp_config: Option<String>` to `AgentConfig`.

Simplify `process_prompt`:
```rust
async fn process_prompt(&mut self, prompt: &str) -> Result<()> {
    // ... build command_prefix, extra_args same as before ...
    let args = SpawnArgs { ..., mcp_config: self.config.mcp_config.clone() };
    let mut process = spawn_claude_process(args)?;
    send_prompt(process.take_stdin().unwrap(), prompt).await?;
    self.consume_output(&mut process.rx).await;
    process.abort();
    Ok(())
}
```

### 8. `src/claude/process.rs` — add `mcp_config` to SpawnArgs

Add field:
```rust
pub struct SpawnArgs {
    // ... existing ...
    pub mcp_config: Option<String>,
}
```

In `build_and_spawn`, after existing args:
```rust
if let Some(config) = &args.mcp_config {
    cmd.args(["--mcp-config", config, "--strict-mcp-config"]);
}
```

### 9. Delete `src/orchestrator/parser.rs` and `src/orchestrator/routing.rs`

Remove module declarations from `src/orchestrator/mod.rs`.

### 10. Update prompts (`prompts/*.md`)

Replace all section-prefix instructions with MCP tool descriptions. Each prompt gets a `## Communication` section listing available tools and workflow. Add explicit instruction: "Never output text prefixes like TASK:, APPROVED:, COMPLETE:. Use only the MCP tools."

### 11. `src/api/types.rs` — add request/response types

```rust
#[derive(Deserialize)]
pub struct McpToolCallRequest {
    pub agent_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
}

#[derive(Serialize)]
pub struct McpToolCallResponse {
    pub result: String,
}
```

### 12. Register route in `src/api/mod.rs`

```rust
.route("/runs/{id}/mcp/tool-call", post(runs::mcp_tool_call))
```

## Key Design Decisions

- **`--strict-mcp-config`**: Prevents agents from using user's globally configured MCP servers, which may not work inside bwrap. Only affects MCP servers, not Claude's built-in tools.
- **Sync MCP binary**: The stdin/stdout loop is inherently synchronous. `reqwest::blocking` keeps it simple. No tokio runtime needed.
- **HTTP bridge to Axum**: The MCP binary calls the existing API. This means routing logic lives in one place (runs.rs), is testable via HTTP, and the binary stays thin.
- **All-at-once migration**: Text parsing and MCP can't coexist — prompts can't describe both interaction models without confusing the LLM.
- **No `AgentOutput` enum**: With MCP handling routing, agents don't produce output that needs runtime processing. The outgoing channel is replaced by the external command channel for `SetCrewSize`/`RelieveManager`.

## Finding the MCP Binary

`std::env::current_exe().parent()` gives the directory containing `claude-app`. The MCP binary `orchestrator-mcp` lives in the same directory (both are `[[bin]]` targets). Inside bwrap with `--ro-bind / /`, the binary path is accessible.

## Verification

1. `cargo build` — both binaries compile
2. `cargo test` — existing API tests pass (update `new_test()` for new RunHandle fields)
3. New unit tests for `dispatch_tool_call` — verify each role/tool combo routes correctly
4. Manual: start API, create run, verify MCP binary spawns with agents and tools are callable
5. End-to-end: send goal to manager, verify task flows through architect → developer → completion
