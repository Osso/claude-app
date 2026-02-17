use std::path::Path;

use anyhow::{bail, Result};

/// Resolve the Claude config directory (~/.claude) that must be writable
/// for the CLI to function (session state, lock files).
fn claude_config_dir() -> String {
    dirs::home_dir()
        .map(|h| h.join(".claude"))
        .unwrap_or_else(|| "/tmp/.claude".into())
        .to_string_lossy()
        .into_owned()
}

/// Build the bwrap command prefix for sandboxing a developer agent.
/// Developer gets read-write access to the worktree only.
///
/// Note: --proc /proc is omitted because Bun (Claude CLI runtime) hangs
/// when bwrap mounts a synthetic procfs. Host /proc is visible but harmless
/// since the sandbox goal is filesystem write protection, not PID isolation.
pub fn bwrap_command_prefix(worktree_path: &Path) -> Vec<String> {
    let worktree = worktree_path.to_string_lossy();
    let claude_dir = claude_config_dir();
    [
        "bwrap",
        "--ro-bind", "/", "/",
        "--dev", "/dev",
        "--tmpfs", "/tmp",
        "--bind", &worktree, &worktree,
        // Claude CLI needs write access to ~/.claude for session state
        "--bind", &claude_dir, &claude_dir,
        "--die-with-parent",
        "--",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Build a read-only bwrap sandbox for non-developer agents.
/// No writable paths except /tmp and ~/.claude (needed for Claude's session state).
pub fn bwrap_readonly_prefix() -> Vec<String> {
    let claude_dir = claude_config_dir();
    [
        "bwrap",
        "--ro-bind", "/", "/",
        "--dev", "/dev",
        "--tmpfs", "/tmp",
        // Claude CLI needs write access to ~/.claude for session state
        "--bind", &claude_dir, &claude_dir,
        "--die-with-parent",
        "--",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Check whether the `bwrap` binary is available in PATH.
pub fn is_bwrap_available() -> bool {
    std::process::Command::new("bwrap")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Validate that the worktree path is suitable for sandboxing.
pub fn validate_sandbox(worktree_path: &Path) -> Result<()> {
    if !worktree_path.is_absolute() {
        bail!("worktree path must be absolute: {}", worktree_path.display());
    }
    if !worktree_path.exists() {
        bail!("worktree path does not exist: {}", worktree_path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn prefix_contains_bwrap_and_worktree() {
        let path = PathBuf::from("/home/user/worktrees/session-1");
        let prefix = bwrap_command_prefix(&path);

        assert_eq!(prefix[0], "bwrap");
        assert!(prefix.contains(&"--ro-bind".to_string()));
        assert!(prefix.contains(&"/home/user/worktrees/session-1".to_string()));
        // Must include writable ~/.claude for Claude CLI session state
        let bind_count = prefix.iter().filter(|s| s.as_str() == "--bind").count();
        assert!(bind_count >= 2, "need --bind for worktree and .claude");
        // No --proc (Bun hangs with synthetic procfs)
        assert!(!prefix.contains(&"--proc".to_string()));
        assert_eq!(prefix.last().unwrap(), "--");
    }

    #[test]
    fn validate_rejects_relative_path() {
        let result = validate_sandbox(Path::new("relative/path"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must be absolute"));
    }

    #[test]
    fn validate_rejects_nonexistent_path() {
        let result = validate_sandbox(Path::new("/nonexistent/path/abc123"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn validate_accepts_existing_absolute_path() {
        let result = validate_sandbox(Path::new("/tmp"));
        assert!(result.is_ok());
    }
}
