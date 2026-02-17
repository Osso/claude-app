use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use tokio::sync::mpsc;

use super::process::{ProcessHandle, SpawnArgs, send_prompt, spawn_claude_process};
use super::protocol::{ClaudeOutput, ContentBlock};
use crate::state::{Message, Session, SessionId, SessionStatus};
use crate::worktree;

pub struct SessionManager {
    processes: HashMap<SessionId, ProcessHandle>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
        }
    }

    pub async fn new_session(
        &self,
        project_path: &Path,
        title: &str,
    ) -> Result<Session> {
        let id = SessionId::new();
        let worktree_name = format!("session-{id}");
        let worktree_path = worktree::create_worktree(project_path, &worktree_name)
            .await
            .context("create worktree for session")?;

        Ok(Session {
            id,
            title: title.to_string(),
            messages: Vec::new(),
            status: SessionStatus::Idle,
            worktree_path,
            project_path: project_path.to_path_buf(),
        })
    }

    pub fn send_prompt(
        &mut self,
        session_id: SessionId,
        prompt: &str,
        sessions: &mut HashMap<SessionId, Session>,
    ) -> Result<mpsc::Receiver<Message>> {
        let session = sessions
            .get_mut(&session_id)
            .context("session not found")?;

        let args = SpawnArgs {
            working_dir: session.worktree_path.clone(),
            system_prompt: String::new(),
            permission_mode: None,
            extra_args: Vec::new(),
            command_prefix: Vec::new(),
        };

        let mut process = spawn_claude_process(args)?;
        let stdin = process
            .take_stdin()
            .context("stdin already taken")?;

        let prompt_owned = prompt.to_string();
        tokio::spawn(async move {
            if let Err(e) = send_prompt(stdin, &prompt_owned).await {
                tracing::error!("failed to send prompt: {e}");
            }
        });

        let (handle, claude_rx) = process.into_parts();
        let (tx, rx) = mpsc::channel::<Message>(256);
        tokio::spawn(relay_output(claude_rx, tx));

        self.processes.insert(session_id, handle);
        session.status = SessionStatus::Running;

        Ok(rx)
    }

    pub fn abort_session(&mut self, session_id: SessionId) {
        if let Some(proc) = self.processes.get_mut(&session_id) {
            proc.abort();
        }
        self.processes.remove(&session_id);
    }

    pub async fn remove_session(
        &mut self,
        session_id: SessionId,
        sessions: &mut HashMap<SessionId, Session>,
    ) -> Result<()> {
        self.abort_session(session_id);

        let session = sessions
            .remove(&session_id)
            .context("session not found")?;

        worktree::remove_worktree(&session.project_path, &session.worktree_path)
            .await
            .context("remove worktree")?;

        Ok(())
    }
}

async fn relay_output(
    mut claude_rx: mpsc::Receiver<ClaudeOutput>,
    tx: mpsc::Sender<Message>,
) {
    while let Some(output) = claude_rx.recv().await {
        match convert_output(output) {
            Some(msg) => {
                if tx.send(msg).await.is_err() {
                    break;
                }
            }
            None => {}
        }
    }
}

pub fn convert_output(output: ClaudeOutput) -> Option<Message> {
    match output {
        ClaudeOutput::System(sys) => Some(Message::System {
            session_id: sys.session_id,
        }),
        ClaudeOutput::Assistant(assistant) => {
            let text = assistant
                .message
                .content
                .into_iter()
                .find_map(|block| match block {
                    ContentBlock::Text { text } => Some(text),
                    _ => None,
                })
                .unwrap_or_default();
            Some(Message::Assistant { text })
        }
        ClaudeOutput::ToolUse(tool) => Some(Message::ToolUse {
            id: tool.tool_use_id,
            name: tool.tool_name,
            input: tool.input,
        }),
        ClaudeOutput::ToolResult(result) => Some(Message::ToolResult {
            id: result.tool_use_id,
            output: result.output.unwrap_or_default(),
            is_error: result.is_error.unwrap_or(false),
        }),
        ClaudeOutput::Result(_) => None,
        ClaudeOutput::Error(err) => {
            let text = err
                .error
                .or(err.message)
                .unwrap_or_else(|| "unknown error".to_string());
            Some(Message::Error { text })
        }
        ClaudeOutput::Unknown => None,
    }
}
