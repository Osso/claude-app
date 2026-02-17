use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tokio::process::Command;

const WORKTREES_DIR: &str = ".claude-sessions/worktrees";

#[derive(Debug)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub head: String,
}

fn project_hash(repo_path: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    repo_path.hash(&mut hasher);
    let hash = hasher.finish();
    format!("{hash:016x}")[..8].to_string()
}

fn worktree_base(repo_path: &Path) -> PathBuf {
    let home = dirs::home_dir().expect("home directory must exist");
    home.join(WORKTREES_DIR).join(project_hash(repo_path))
}

pub async fn create_worktree(repo_path: &Path, name: &str) -> Result<PathBuf> {
    let repo_path = repo_path
        .canonicalize()
        .context("canonicalize repo path")?;
    let worktree_path = worktree_base(&repo_path).join(name);
    let branch_name = format!("claude-sessions/{name}");

    tokio::fs::create_dir_all(worktree_path.parent().unwrap())
        .await
        .context("create worktree parent directories")?;

    let output = Command::new("git")
        .arg("-C")
        .arg(&repo_path)
        .arg("worktree")
        .arg("add")
        .arg(&worktree_path)
        .arg("-b")
        .arg(&branch_name)
        .output()
        .await
        .context("spawn git worktree add")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git worktree add failed: {stderr}");
    }

    Ok(worktree_path)
}

pub async fn remove_worktree(repo_path: &Path, worktree_path: &Path) -> Result<()> {
    let branch_name = detect_branch(worktree_path).await;

    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("worktree")
        .arg("remove")
        .arg(worktree_path)
        .arg("--force")
        .output()
        .await
        .context("spawn git worktree remove")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git worktree remove failed: {stderr}");
    }

    if let Some(branch) = branch_name {
        delete_branch(repo_path, &branch).await?;
    }

    Ok(())
}

async fn detect_branch(worktree_path: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(worktree_path)
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if branch == "HEAD" {
        None
    } else {
        Some(branch)
    }
}

async fn delete_branch(repo_path: &Path, branch: &str) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("branch")
        .arg("-D")
        .arg(branch)
        .output()
        .await
        .context("spawn git branch -D")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git branch -D {branch} failed: {stderr}");
    }

    Ok(())
}

pub async fn reset_worktree(worktree_path: &Path) -> Result<()> {
    let output = Command::new("git")
        .arg("checkout")
        .arg(".")
        .current_dir(worktree_path)
        .output()
        .await
        .context("spawn git checkout .")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git checkout . failed: {stderr}");
    }

    let output = Command::new("git")
        .arg("clean")
        .arg("-fd")
        .current_dir(worktree_path)
        .output()
        .await
        .context("spawn git clean -fd")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git clean -fd failed: {stderr}");
    }

    Ok(())
}

pub async fn list_worktrees(repo_path: &Path) -> Result<Vec<WorktreeInfo>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("worktree")
        .arg("list")
        .arg("--porcelain")
        .output()
        .await
        .context("spawn git worktree list")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git worktree list failed: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_porcelain_worktrees(&stdout))
}

fn parse_porcelain_worktrees(output: &str) -> Vec<WorktreeInfo> {
    let mut worktrees = Vec::new();
    let mut path: Option<PathBuf> = None;
    let mut head: Option<String> = None;
    let mut branch: Option<String> = None;

    for line in output.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            path = Some(PathBuf::from(p));
        } else if let Some(h) = line.strip_prefix("HEAD ") {
            head = Some(h.to_string());
        } else if let Some(b) = line.strip_prefix("branch ") {
            branch = Some(b.strip_prefix("refs/heads/").unwrap_or(b).to_string());
        } else if line.is_empty() {
            if let (Some(p), Some(h)) = (path.take(), head.take()) {
                worktrees.push(WorktreeInfo {
                    path: p,
                    branch: branch.take(),
                    head: h,
                });
            }
            branch = None;
        }
    }

    // Handle last entry if output doesn't end with blank line
    if let (Some(p), Some(h)) = (path, head) {
        worktrees.push(WorktreeInfo {
            path: p,
            branch: branch.take(),
            head: h,
        });
    }

    worktrees
}
