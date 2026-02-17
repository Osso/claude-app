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
/// Developers auto-accept edits. Others use dontAsk mode which auto-denies
/// tool uses that aren't pre-approved (read-only tools work without approval).
pub fn permission_mode(role: AgentRole) -> &'static str {
    match role {
        AgentRole::Developer => "acceptEdits",
        AgentRole::Manager | AgentRole::Architect | AgentRole::Scorer => "dontAsk",
    }
}
