use std::path::Path;

use anyhow::{bail, Result};

/// Build the bwrap command prefix for sandboxing a developer agent.
///
/// The returned Vec is meant to be passed as `SpawnArgs::command_prefix`,
/// which `process.rs` prepends before the `claude` binary.
pub fn bwrap_command_prefix(worktree_path: &Path) -> Vec<String> {
    let worktree = worktree_path.to_string_lossy();
    [
        "bwrap",
        "--ro-bind", "/", "/",
        "--dev", "/dev",
        "--proc", "/proc",
        "--tmpfs", "/tmp",
        "--bind", &worktree, &worktree,
        "--unshare-pid",
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
