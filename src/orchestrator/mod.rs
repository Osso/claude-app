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
    /// Track which agents are alive
    agent_ids: Vec<AgentId>,
    /// Runtime task handle
    runtime_handle: JoinHandle<()>,
}

impl RunHandle {
    /// Get a list of active agent IDs at the time of creation.
    pub fn agent_ids(&self) -> &[AgentId] {
        &self.agent_ids
    }

    /// Subscribe to the UI output stream for all agents.
    pub fn subscribe(&self) -> broadcast::Receiver<(AgentId, ClaudeOutput)> {
        self.ui_tx.subscribe()
    }

    /// Signal the runtime to shut down all agents.
    pub fn abort(&self) {
        let _ = self.abort_tx.try_send(());
    }

    /// Check if the runtime task is still running.
    pub fn is_running(&self) -> bool {
        !self.runtime_handle.is_finished()
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
    /// Start an orchestrator run. Creates worktrees, spawns agents, sends the goal.
    /// Returns a RunHandle for the caller.
    pub async fn spawn_run(
        goal: String,
        project_path: PathBuf,
    ) -> Result<RunHandle> {
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

        let agent_ids: Vec<AgentId> = runtime.agent_inboxes.keys().cloned().collect();

        // Send initial goal to manager
        let goal_msg = AgentMessage::new(
            AgentId::new_singleton(AgentRole::Manager), // from self (bootstrap)
            AgentId::new_singleton(AgentRole::Manager),
            MessageKind::Info,
            goal,
        );
        runtime.deliver_message(goal_msg);

        let runtime_handle = tokio::spawn(async move {
            if let Err(e) = runtime.run_loop().await {
                tracing::error!("Orchestrator runtime error: {}", e);
            }
        });

        Ok(RunHandle {
            ui_tx,
            abort_tx,
            agent_ids,
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
            worktree_path: None,
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
            working_dir: worktree_path.clone(),
            worktree_path: Some(worktree_path),
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
