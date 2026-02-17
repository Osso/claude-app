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
