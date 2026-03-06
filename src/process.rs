use std::process::{Child, Command, Stdio};

pub fn spawn_orchestrator(dir: &str, task: &str) -> anyhow::Result<Child> {
    let child = Command::new("agent-orchestrator")
        .args(["run", dir, task])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    tracing::info!(
        pid = child.id(),
        dir,
        "Spawned agent-orchestrator"
    );

    Ok(child)
}
