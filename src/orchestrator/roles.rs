use super::types::AgentRole;

/// System prompt for each agent role, embedded from prompts/*.md at compile time.
pub fn system_prompt(role: AgentRole) -> &'static str {
    match role {
        AgentRole::Manager => include_str!("../../prompts/manager.md"),
        AgentRole::Architect => include_str!("../../prompts/architect.md"),
        AgentRole::Developer => include_str!("../../prompts/developer.md"),
        AgentRole::Scorer => include_str!("../../prompts/scorer.md"),
    }
}

/// Permission mode for the Claude CLI `--permission-mode` flag.
/// All agents use bypassPermissions — bwrap sandbox is the real security
/// boundary. `acceptEdits` blocks writes outside the "project root", but
/// Claude resolves project root via .git which points back to the original
/// repo path, not the worktree. bypassPermissions skips Claude's permission
/// checks entirely, letting bwrap enforce write restrictions at the OS level.
pub fn permission_mode(_role: AgentRole) -> &'static str {
    "bypassPermissions"
}
