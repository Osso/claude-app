pub mod agent;
pub mod parser;
pub mod roles;
pub mod routing;
pub mod types;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

use crate::claude::protocol::ClaudeOutput;
use crate::worktree;

use self::agent::{Agent, AgentConfig, AgentOutput};
use self::types::{AgentId, AgentMessage, AgentRole, MessageKind, RuntimeCommand};

const RELIEVE_COOLDOWN: Duration = Duration::from_secs(60);

/// Handle returned to the caller for controlling and observing a run.
pub struct RunHandle {
    /// Subscribe to all agent output for UI display
    ui_tx: broadcast::Sender<(AgentId, ClaudeOutput)>,
    /// Abort all agents
    abort_tx: mpsc::Sender<()>,
    /// Agent inbox senders for direct messaging
    agent_inboxes: HashMap<AgentId, mpsc::Sender<AgentMessage>>,
    /// Runtime task handle
    runtime_handle: JoinHandle<()>,
}

impl RunHandle {
    /// Subscribe to the UI output stream for all agents.
    pub fn subscribe(&self) -> broadcast::Receiver<(AgentId, ClaudeOutput)> {
        self.ui_tx.subscribe()
    }

    /// Signal the runtime to shut down all agents.
    pub fn abort(&self) {
        let _ = self.abort_tx.try_send(());
    }

    /// Return the IDs of all agents in this run.
    pub fn agent_ids(&self) -> Vec<AgentId> {
        self.agent_inboxes.keys().cloned().collect()
    }

    /// Send a user message to a specific agent.
    pub fn send_to_agent(&self, agent_id: &AgentId, content: String) -> bool {
        if let Some(tx) = self.agent_inboxes.get(agent_id) {
            let msg = AgentMessage::new(
                agent_id.clone(), // from (irrelevant for UserMessage)
                agent_id.clone(),
                MessageKind::UserMessage,
                content,
            );
            tx.try_send(msg).is_ok()
        } else {
            false
        }
    }
}

#[cfg(test)]
impl RunHandle {
    /// Create a test handle with fresh channels. Returns:
    /// - the RunHandle itself
    /// - abort_rx: mpsc::Receiver<()> -- recv to verify abort was called
    /// - agent_inboxes_rx: HashMap<AgentId, mpsc::Receiver<AgentMessage>> -- recv to verify messages
    pub fn new_test(
        agent_ids: Vec<AgentId>,
    ) -> (
        Self,
        mpsc::Receiver<()>,
        HashMap<AgentId, mpsc::Receiver<AgentMessage>>,
    ) {
        let (ui_tx, _) = broadcast::channel(16);
        let (abort_tx, abort_rx) = mpsc::channel(1);
        let mut agent_inboxes = HashMap::new();
        let mut inbox_receivers = HashMap::new();

        for id in agent_ids {
            let (tx, rx) = mpsc::channel(16);
            agent_inboxes.insert(id.clone(), tx);
            inbox_receivers.insert(id, rx);
        }

        // Spawn a no-op task for runtime_handle
        let runtime_handle = tokio::spawn(async {});

        let handle = Self {
            ui_tx,
            abort_tx,
            agent_inboxes,
            runtime_handle,
        };

        (handle, abort_rx, inbox_receivers)
    }
}

/// Mutable state tracked by the runtime
struct RuntimeState {
    developer_count: u8,
    manager_generation: u32,
    last_relieve: Option<Instant>,
}

/// Core orchestrator that spawns agents and routes messages between them.
pub struct OrchestratorRuntime {
    state: RuntimeState,
    project_path: PathBuf,
    /// Per-agent message inboxes
    agent_inboxes: HashMap<AgentId, mpsc::Sender<AgentMessage>>,
    /// Handles for spawned agent tasks
    agent_handles: HashMap<AgentId, JoinHandle<()>>,
    /// Shared sender for agent output → runtime routing
    outgoing_tx: mpsc::Sender<AgentOutput>,
    outgoing_rx: mpsc::Receiver<AgentOutput>,
    /// Broadcast channel for UI consumption
    ui_tx: broadcast::Sender<(AgentId, ClaudeOutput)>,
    /// Worktree paths for developers (for cleanup)
    developer_worktrees: HashMap<AgentId, PathBuf>,
    /// Abort signal
    abort_rx: mpsc::Receiver<()>,
}

impl OrchestratorRuntime {
    /// Start an orchestrator run. Creates worktrees, spawns agents.
    /// Returns a RunHandle for the caller. The user sends the goal via send_to_agent.
    pub async fn spawn_run(
        project_path: PathBuf,
    ) -> Result<RunHandle> {
        // Resolve symlinks — bwrap can't bind to symlink destinations
        let project_path = project_path.canonicalize()
            .context("canonicalize project path")?;

        let (ui_tx, _) = broadcast::channel(256);
        let (outgoing_tx, outgoing_rx) = mpsc::channel(64);
        let (abort_tx, abort_rx) = mpsc::channel(1);

        let mut runtime = Self {
            state: RuntimeState {
                developer_count: 1,
                manager_generation: 0,
                last_relieve: None,
            },
            project_path: project_path.clone(),
            agent_inboxes: HashMap::new(),
            agent_handles: HashMap::new(),
            outgoing_tx,
            outgoing_rx,
            ui_tx: ui_tx.clone(),
            developer_worktrees: HashMap::new(),
            abort_rx,
        };

        runtime.spawn_singleton(AgentRole::Manager).await?;
        runtime.spawn_singleton(AgentRole::Architect).await?;
        runtime.spawn_singleton(AgentRole::Scorer).await?;
        runtime.spawn_developer(0).await?;

        // Clone inbox senders for RunHandle before runtime consumes them
        let handle_inboxes: HashMap<AgentId, mpsc::Sender<AgentMessage>> =
            runtime.agent_inboxes.clone();

        let runtime_handle = tokio::spawn(async move {
            if let Err(e) = runtime.run_loop().await {
                tracing::error!("Orchestrator runtime error: {}", e);
            }
        });

        Ok(RunHandle {
            ui_tx,
            abort_tx,
            agent_inboxes: handle_inboxes,
            runtime_handle,
        })
    }

    /// Main loop: route agent outputs and handle commands.
    async fn run_loop(&mut self) -> Result<()> {
        loop {
            tokio::select! {
                output = self.outgoing_rx.recv() => {
                    match output {
                        Some(AgentOutput::Message(msg)) => {
                            self.deliver_message(msg);
                        }
                        Some(AgentOutput::Command(cmd)) => {
                            self.handle_command(cmd).await;
                        }
                        None => {
                            tracing::info!("All agent output senders dropped, shutting down");
                            break;
                        }
                    }
                }
                _ = self.abort_rx.recv() => {
                    tracing::info!("Abort signal received");
                    break;
                }
            }
        }

        self.shutdown().await;
        Ok(())
    }

    /// Deliver a message to the target agent's inbox.
    fn deliver_message(&self, msg: AgentMessage) {
        if let Some(tx) = self.agent_inboxes.get(&msg.to) {
            if tx.try_send(msg).is_err() {
                tracing::warn!("Failed to deliver message (inbox full or closed)");
            }
        } else {
            tracing::warn!("No inbox for agent {}", msg.to);
        }
    }

    async fn handle_command(&mut self, cmd: RuntimeCommand) {
        match cmd {
            RuntimeCommand::SetCrewSize { count } => {
                self.handle_crew_size(count).await;
            }
            RuntimeCommand::RelieveManager { reason } => {
                self.handle_relieve_manager(&reason).await;
            }
        }
    }

    /// Spawn a singleton agent (Manager, Architect, Scorer).
    async fn spawn_singleton(&mut self, role: AgentRole) -> Result<()> {
        let id = AgentId::new_singleton(role);
        let config = AgentConfig {
            working_dir: self.project_path.clone(),
            worktree_bind: None,
        };
        self.spawn_agent(id, config).await
    }

    /// Spawn a developer agent with its own worktree.
    async fn spawn_developer(&mut self, index: u8) -> Result<()> {
        let id = AgentId::new_developer(index);
        let worktree_name = format!("orch-dev-{index}");
        let worktree_path = worktree::create_worktree(&self.project_path, &worktree_name)
            .await
            .context("create developer worktree")?;

        self.developer_worktrees.insert(id.clone(), worktree_path.clone());

        let config = AgentConfig {
            // Use project_path as working_dir — bwrap mounts worktree there
            working_dir: self.project_path.clone(),
            worktree_bind: Some((worktree_path, self.project_path.clone())),
        };
        self.spawn_agent(id, config).await
    }

    /// Spawn an agent as a tokio task and register its inbox.
    async fn spawn_agent(&mut self, id: AgentId, config: AgentConfig) -> Result<()> {
        let (inbox_tx, inbox_rx) = mpsc::channel(16);
        let outgoing_tx = self.outgoing_tx.clone();
        let ui_tx = self.ui_tx.clone();

        let agent = Agent::new(
            id.clone(),
            config,
            inbox_rx,
            outgoing_tx,
            ui_tx,
        );

        let handle = tokio::spawn(async move {
            if let Err(e) = agent.run().await {
                tracing::error!("Agent error: {}", e);
            }
        });

        self.agent_inboxes.insert(id.clone(), inbox_tx);
        self.agent_handles.insert(id, handle);
        Ok(())
    }

    /// Adjust developer count: spawn or kill developers to match target.
    async fn handle_crew_size(&mut self, count: u8) {
        let count = count.clamp(1, 3);
        let current = self.state.developer_count;

        if count == current {
            return;
        }

        tracing::info!("CREW resize: {} -> {}", current, count);

        if count > current {
            for i in current..count {
                if let Err(e) = self.spawn_developer(i).await {
                    tracing::error!("Failed to spawn developer-{}: {}", i, e);
                }
            }
        } else {
            for i in count..current {
                self.kill_developer(i).await;
            }
        }

        self.state.developer_count = count;
    }

    /// Kill a developer agent and clean up its worktree.
    async fn kill_developer(&mut self, index: u8) {
        let id = AgentId::new_developer(index);
        self.agent_inboxes.remove(&id);
        if let Some(handle) = self.agent_handles.remove(&id) {
            handle.abort();
        }
        if let Some(wt_path) = self.developer_worktrees.remove(&id) {
            if let Err(e) = worktree::remove_worktree(&self.project_path, &wt_path).await {
                tracing::warn!("Failed to remove worktree for {}: {}", id, e);
            }
        }
    }

    /// Replace the current manager with a fresh instance briefed on state.
    async fn handle_relieve_manager(&mut self, reason: &str) {
        if let Some(last) = self.state.last_relieve {
            if last.elapsed() < RELIEVE_COOLDOWN {
                tracing::warn!(
                    "RELIEVE rejected: cooldown ({:.0}s remaining)",
                    (RELIEVE_COOLDOWN - last.elapsed()).as_secs_f64()
                );
                return;
            }
        }

        tracing::warn!(
            "RELIEVE: firing manager gen {} -- {}",
            self.state.manager_generation,
            reason,
        );

        // Kill current manager
        let mgr_id = AgentId::new_singleton(AgentRole::Manager);
        self.agent_inboxes.remove(&mgr_id);
        if let Some(handle) = self.agent_handles.remove(&mgr_id) {
            handle.abort();
        }

        self.state.manager_generation += 1;
        self.state.last_relieve = Some(Instant::now());

        // Spawn replacement
        if let Err(e) = self.spawn_singleton(AgentRole::Manager).await {
            tracing::error!("Failed to spawn replacement manager: {}", e);
            return;
        }

        // Brief the new manager
        let briefing = format!(
            "## State Briefing (you are replacing the previous manager)\n\n\
             **Reason for replacement:** {}\n\
             **Manager generation:** {}\n\
             **Active developers:** {}\n",
            reason, self.state.manager_generation, self.state.developer_count,
        );

        let brief_msg = AgentMessage::new(
            AgentId::new_singleton(AgentRole::Scorer),
            AgentId::new_singleton(AgentRole::Manager),
            MessageKind::Info,
            briefing,
        );
        self.deliver_message(brief_msg);
    }

    /// Shut down all agents and clean up worktrees.
    async fn shutdown(&mut self) {
        tracing::info!("Shutting down {} agents", self.agent_handles.len());

        // Drop all inboxes to signal agents to stop
        self.agent_inboxes.clear();

        for (id, handle) in self.agent_handles.drain() {
            tracing::info!("Stopping {}", id);
            handle.abort();
        }

        // Clean up developer worktrees
        for (id, wt_path) in self.developer_worktrees.drain() {
            if let Err(e) = worktree::remove_worktree(&self.project_path, &wt_path).await {
                tracing::warn!("Failed to remove worktree for {}: {}", id, e);
            }
        }
    }
}
