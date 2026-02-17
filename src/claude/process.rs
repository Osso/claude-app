use std::path::PathBuf;
use std::process::Stdio;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin};
use tokio::sync::mpsc;

use super::protocol::{ClaudeInput, ClaudeOutput};

pub struct SpawnArgs {
    pub working_dir: PathBuf,
    pub system_prompt: String,
    pub permission_mode: Option<String>,
    pub extra_args: Vec<String>,
    /// Command prefix (e.g. bwrap args) inserted before "claude".
    pub command_prefix: Vec<String>,
}

pub struct ClaudeProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    pub rx: mpsc::Receiver<ClaudeOutput>,
}

/// Handle for a running Claude process (abort only, output already consumed).
pub struct ProcessHandle {
    child: Child,
}

impl ProcessHandle {
    pub fn abort(&mut self) {
        let _ = self.child.start_kill();
    }
}

impl ClaudeProcess {
    pub fn abort(&mut self) {
        let _ = self.child.start_kill();
    }

    pub fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.stdin.take()
    }

    /// Split into a handle (for abort) and the output receiver.
    pub fn into_parts(self) -> (ProcessHandle, mpsc::Receiver<ClaudeOutput>) {
        (ProcessHandle { child: self.child }, self.rx)
    }

    pub async fn wait(&mut self) -> Result<()> {
        self.child.wait().await.context("Failed to wait for claude process")?;
        Ok(())
    }
}

pub fn spawn_claude_process(args: SpawnArgs) -> Result<ClaudeProcess> {
    let mut child = build_and_spawn(&args)?;

    let stdin = child.stdin.take().context("Failed to get stdin")?;
    let stdout = child.stdout.take().context("Failed to get stdout")?;
    let stderr = child.stderr.take().context("Failed to get stderr")?;

    let (tx, rx) = mpsc::channel::<ClaudeOutput>(256);
    spawn_stderr_logger(stderr);
    spawn_stdout_reader(stdout, tx);

    Ok(ClaudeProcess {
        child,
        stdin: Some(stdin),
        rx,
    })
}

fn build_and_spawn(args: &SpawnArgs) -> Result<Child> {
    let (program, prefix_args) = if args.command_prefix.is_empty() {
        ("claude".to_string(), Vec::new())
    } else {
        (
            args.command_prefix[0].clone(),
            args.command_prefix[1..].to_vec(),
        )
    };

    let mut cmd = tokio::process::Command::new(&program);

    for arg in &prefix_args {
        cmd.arg(arg);
    }

    if !args.command_prefix.is_empty() {
        cmd.arg("claude");
    }

    cmd.args([
        "-p",
        "--input-format", "stream-json",
        "--output-format", "stream-json",
        "--verbose",
        "--system-prompt", &args.system_prompt,
    ]);

    if let Some(mode) = &args.permission_mode {
        cmd.args(["--permission-mode", mode]);
    }

    for arg in &args.extra_args {
        cmd.arg(arg);
    }

    // Prevent "nested session" error if parent has CLAUDECODE set
    cmd.env_remove("CLAUDECODE");

    cmd.current_dir(&args.working_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());


    cmd.spawn()
        .with_context(|| format!("Failed to spawn claude process via '{program}'"))
}

pub async fn send_prompt(mut stdin: ChildStdin, prompt: &str) -> Result<()> {
    let input = ClaudeInput::user(prompt);
    let json = serde_json::to_string(&input)?;
    stdin.write_all(json.as_bytes()).await?;
    stdin.write_all(b"\n").await?;
    stdin.flush().await?;
    drop(stdin);
    Ok(())
}

fn spawn_stderr_logger(stderr: tokio::process::ChildStderr) {
    tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            tracing::warn!("[claude stderr] {}", line);
        }
    });
}

fn spawn_stdout_reader(stdout: tokio::process::ChildStdout, tx: mpsc::Sender<ClaudeOutput>) {
    tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<ClaudeOutput>(&line) {
                Ok(output) => {
                    let is_final = output.is_final();
                    if tx.send(output).await.is_err() || is_final {
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to parse Claude output: {} - line: {}", e, line);
                }
            }
        }
    });
}
