use std::path::PathBuf;

use anyhow::{Context, Result};
use tokio::sync::{broadcast, mpsc};

use crate::claude::process::{spawn_claude_process, send_prompt, SpawnArgs};
use crate::claude::protocol::{ClaudeOutput, ContentBlock};
use crate::sandbox::{bwrap_command_prefix, bwrap_readonly_prefix};

use super::parser::extract_sections;
use super::roles;
use super::routing::{ParsedOutput, route_sections};
use super::types::{AgentId, AgentMessage, MessageKind, RuntimeCommand};

/// Outgoing item from an agent: either a routed message or a runtime command.
pub enum AgentOutput {
    Message(AgentMessage),
    Command(RuntimeCommand),
}

/// Configuration for spawning an agent
pub struct AgentConfig {
    pub working_dir: PathBuf,
    /// For developers: bwrap worktree path. None for non-sandboxed agents.
    pub worktree_path: Option<PathBuf>,
}

/// A running agent that receives messages, sends them to Claude, and routes output.
pub struct Agent {
    id: AgentId,
    config: AgentConfig,
    /// Claude CLI session ID for conversation continuity.
    session_id: Option<String>,
    message_rx: mpsc::Receiver<AgentMessage>,
    outgoing_tx: mpsc::Sender<AgentOutput>,
    ui_tx: broadcast::Sender<(AgentId, ClaudeOutput)>,
}

impl Agent {
    pub fn new(
        id: AgentId,
        config: AgentConfig,
        message_rx: mpsc::Receiver<AgentMessage>,
        outgoing_tx: mpsc::Sender<AgentOutput>,
        ui_tx: broadcast::Sender<(AgentId, ClaudeOutput)>,
    ) -> Self {
        Self {
            id,
            config,
            session_id: None,
            message_rx,
            outgoing_tx,
            ui_tx,
        }
    }

    /// Run the agent loop: wait for messages, send to Claude, parse and route output.
    pub async fn run(mut self) -> Result<()> {
        tracing::info!("Agent {} starting", self.id);

        while let Some(msg) = self.message_rx.recv().await {
            tracing::info!(
                "Agent {} received {:?} from {}",
                self.id, msg.kind, msg.from,
            );

            let prompt = format_prompt(&msg);

            if let Err(e) = self.process_prompt(&prompt).await {
                tracing::error!("Agent {} process error: {}", self.id, e);
            }
        }

        tracing::info!("Agent {} shutting down (channel closed)", self.id);
        Ok(())
    }

    async fn process_prompt(&mut self, prompt: &str) -> Result<()> {
        let command_prefix = match &self.config.worktree_path {
            Some(path) => bwrap_command_prefix(path),
            None => bwrap_readonly_prefix(),
        };

        let extra_args = match &self.session_id {
            Some(sid) => vec!["--resume".into(), sid.clone()],
            None => Vec::new(),
        };

        let args = SpawnArgs {
            working_dir: self.config.working_dir.clone(),
            system_prompt: roles::system_prompt(self.id.role).to_string(),
            permission_mode: Some(roles::permission_mode(self.id.role).to_string()),
            extra_args,
            command_prefix,
        };

        let mut process = spawn_claude_process(args).context("spawn claude process")?;
        let stdin = process.take_stdin().context("get stdin")?;
        send_prompt(stdin, prompt).await.context("send prompt")?;

        let all_text = self.consume_output(&mut process.rx).await;
        // Kill immediately — Claude CLI hangs after final output (MCP server cleanup)
        process.abort();

        let sections = extract_sections(&all_text);
        let parsed = route_sections(&self.id, sections);
        dispatch_parsed(&self.outgoing_tx, parsed).await;

        Ok(())
    }

    /// Read all output from the process, forward to UI, and collect assistant text.
    async fn consume_output(&mut self, rx: &mut mpsc::Receiver<ClaudeOutput>) -> String {
        let mut all_text = String::new();

        while let Some(output) = rx.recv().await {
            let _ = self.ui_tx.send((self.id.clone(), output.clone()));

            match &output {
                ClaudeOutput::System(sys) => {
                    if let Some(sid) = &sys.session_id {
                        self.session_id = Some(sid.clone());
                    }
                }
                ClaudeOutput::Assistant(asst) => {
                    for block in &asst.message.content {
                        if let ContentBlock::Text { text } = block {
                            all_text.push_str(text);
                            all_text.push('\n');
                        }
                    }
                }
                _ => {}
            }

            if output.is_final() {
                break;
            }
        }

        all_text
    }
}

/// Dispatch parsed outputs to the outgoing channel
async fn dispatch_parsed(tx: &mpsc::Sender<AgentOutput>, items: Vec<ParsedOutput>) {
    for item in items {
        let output = match item {
            ParsedOutput::Message(msg) => {
                tracing::info!("Routing message to {}: {:?}", msg.to, msg.kind);
                AgentOutput::Message(msg)
            }
            ParsedOutput::Command(cmd) => {
                tracing::info!("Runtime command: {:?}", cmd);
                AgentOutput::Command(cmd)
            }
        };
        if tx.send(output).await.is_err() {
            tracing::warn!("Outgoing channel closed");
            break;
        }
    }
}

/// Format incoming message as a prompt for the AI
fn format_prompt(msg: &AgentMessage) -> String {
    let context = match msg.kind {
        MessageKind::TaskAssignment => "NEW TASK",
        MessageKind::TaskComplete => "TASK COMPLETE",
        MessageKind::TaskGiveUp => "TASK BLOCKED",
        MessageKind::Interrupt => "INTERRUPT",
        MessageKind::ArchitectReview => "ARCHITECT REVIEW",
        MessageKind::Info => "INFO",
        MessageKind::Evaluation => "EVALUATION",
        MessageKind::Observation => "OBSERVATION",
        MessageKind::UserMessage => {
            return format!("USER: {}", msg.content);
        }
    };

    format!("{} from {}: {}", context, msg.from, msg.content)
}
