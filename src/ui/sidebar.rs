use std::collections::HashMap;
use std::path::PathBuf;

use dioxus::prelude::*;

use crate::claude::{SessionManager, convert_output};
use crate::orchestrator::types::{AgentId, AgentRole};
use crate::orchestrator::OrchestratorRuntime;
use crate::state::{
    OrchestratorRun, RunId, RunStatus, Session, SessionId, SessionStatus,
};

#[component]
pub fn Sidebar() -> Element {
    let sessions: Signal<HashMap<SessionId, Session>> = use_context();
    let active_id: Signal<Option<SessionId>> = use_context();

    let sessions_read = sessions.read();
    let active = active_id();

    let mut entries: Vec<_> = sessions_read
        .iter()
        .map(|(id, s)| (*id, s.title.clone(), s.status.clone()))
        .collect();
    entries.sort_by(|a, b| a.1.cmp(&b.1));

    rsx! {
        div {
            class: "sidebar",
            div {
                class: "sidebar-section",
                NewSessionButton {}
            }
            SessionList { entries, active }
            OrchestratorSection {}
        }
    }
}

#[component]
fn NewSessionButton() -> Element {
    let mut sessions: Signal<HashMap<SessionId, Session>> = use_context();
    let mut active_id: Signal<Option<SessionId>> = use_context();
    let mut manager: Signal<SessionManager> = use_context();
    let project_path_signal: Signal<Option<PathBuf>> = use_context();

    rsx! {
        button {
            class: "btn btn-full",
            onclick: move |_| {
                spawn(async move {
                    let project_path = project_path_signal().unwrap_or_default();
                    let count = sessions.read().len() + 1;
                    let title = format!("Session {count}");
                    match manager.write().new_session(&project_path, &title).await {
                        Ok(session) => {
                            let id = session.id;
                            sessions.write().insert(id, session);
                            active_id.set(Some(id));
                        }
                        Err(e) => tracing::error!("Failed to create session: {e}"),
                    }
                });
            },
            "+ New Session"
        }
    }
}

#[component]
fn SessionList(
    entries: Vec<(SessionId, String, SessionStatus)>,
    active: Option<SessionId>,
) -> Element {
    let mut active_id: Signal<Option<SessionId>> = use_context();
    let mut sessions: Signal<HashMap<SessionId, Session>> = use_context();
    let mut manager: Signal<SessionManager> = use_context();

    rsx! {
        div {
            class: "sidebar-list",
            for (id, title, status) in entries {
                SessionEntry {
                    id,
                    title,
                    status,
                    is_active: active == Some(id),
                    on_click: move |_| active_id.set(Some(id)),
                    on_close: move |_| {
                        spawn(async move {
                            manager.write().abort_session(id);
                            sessions.write().remove(&id);
                            if active_id() == Some(id) {
                                let next = sessions.read().keys().next().copied();
                                active_id.set(next);
                            }
                        });
                    },
                }
            }
        }
    }
}

#[component]
fn SessionEntry(
    id: SessionId,
    title: String,
    status: SessionStatus,
    is_active: bool,
    on_click: EventHandler<()>,
    on_close: EventHandler<()>,
) -> Element {
    let active_class = if is_active { " active" } else { "" };

    rsx! {
        div {
            class: "session-item{active_class}",
            onclick: move |_| on_click.call(()),
            StatusBadge { status: status.clone() }
            span {
                class: "session-item-title",
                "{title}"
            }
            span {
                class: "session-close",
                onclick: move |evt| {
                    evt.stop_propagation();
                    on_close.call(());
                },
                "x"
            }
        }
    }
}

#[component]
fn StatusBadge(status: SessionStatus) -> Element {
    let (class, label) = match status {
        SessionStatus::Idle => ("badge badge-idle", "idle"),
        SessionStatus::Running => ("badge badge-running", "run"),
        SessionStatus::Error(_) => ("badge badge-error", "err"),
    };

    rsx! {
        span { class: class, "{label}" }
    }
}

// --- Orchestrator section ---

#[component]
fn OrchestratorSection() -> Element {
    let runs: Signal<Vec<OrchestratorRun>> = use_context();
    let runs_read = runs.read();

    rsx! {
        div {
            class: "orchestrator-section",
            div {
                class: "sidebar-section",
                NewRunButton {}
            }
            div {
                class: "orchestrator-runs",
                for (idx, run) in runs_read.iter().enumerate() {
                    RunEntry {
                        run_idx: idx,
                        run_id: run.id,
                        goal: run.goal.clone(),
                        status: run.status.clone(),
                        agent_sessions: run.agent_sessions.clone(),
                    }
                }
            }
        }
    }
}

#[component]
fn NewRunButton() -> Element {
    let mut sessions: Signal<HashMap<SessionId, Session>> = use_context();
    let mut runs: Signal<Vec<OrchestratorRun>> = use_context();
    let mut active_id: Signal<Option<SessionId>> = use_context();
    let project_path_signal: Signal<Option<PathBuf>> = use_context();

    rsx! {
        button {
            class: "btn btn-full",
            onclick: move |_| {
                start_run(&mut sessions, &mut runs, &mut active_id, project_path_signal);
            },
            "+ New Run"
        }
    }
}

fn start_run(
    sessions: &mut Signal<HashMap<SessionId, Session>>,
    runs: &mut Signal<Vec<OrchestratorRun>>,
    active_id: &mut Signal<Option<SessionId>>,
    project_path_signal: Signal<Option<PathBuf>>,
) {
    let project_path = project_path_signal().unwrap_or_default();
    let run_id = RunId::new();

    // Known agent roles — create sessions synchronously
    let initial_agents = [
        AgentId::new_singleton(AgentRole::Manager),
        AgentId::new_singleton(AgentRole::Architect),
        AgentId::new_singleton(AgentRole::Scorer),
        AgentId::new_developer(0),
    ];

    let mut agent_sessions = HashMap::new();
    {
        let mut sessions_write = sessions.write();
        for agent_id in &initial_agents {
            let session_id = SessionId::new();
            let session = Session {
                id: session_id,
                title: format!("{}", agent_id),
                messages: Vec::new(),
                status: SessionStatus::Idle,
                worktree_path: project_path.clone(),
                project_path: project_path.clone(),
            };
            sessions_write.insert(session_id, session);
            agent_sessions.insert(agent_id.clone(), session_id);
        }
    }

    // Select manager's session
    if let Some(first_sid) = agent_sessions.get(&initial_agents[0]).copied() {
        active_id.set(Some(first_sid));
    }

    // Store run immediately (no handle yet)
    let run = OrchestratorRun {
        id: run_id,
        goal: String::new(),
        agent_sessions: agent_sessions.clone(),
        status: RunStatus::Running,
        run_handle: None,
    };
    runs.write().push(run);

    // Spawn orchestrator in background, wire up relay when ready
    let mut sessions = *sessions;
    let mut runs = *runs;
    spawn(async move {
        let run_handle = match OrchestratorRuntime::spawn_run(project_path).await {
            Ok(h) => h,
            Err(e) => {
                tracing::error!("Failed to start orchestrator run: {e:#}");
                mark_run_failed(&mut runs, &mut sessions, run_id, &agent_sessions, &e.to_string());
                return;
            }
        };

        let rx = run_handle.subscribe();

        // Store handle on the run
        {
            let mut runs_write = runs.write();
            if let Some(run) = runs_write.iter_mut().find(|r| r.id == run_id) {
                run.run_handle = Some(run_handle);
            }
        }

        spawn_output_relay(rx, agent_sessions, sessions, runs, run_id);
    });
}

fn spawn_output_relay(
    mut rx: tokio::sync::broadcast::Receiver<(AgentId, crate::claude::ClaudeOutput)>,
    agent_sessions: HashMap<AgentId, SessionId>,
    mut sessions: Signal<HashMap<SessionId, Session>>,
    mut runs: Signal<Vec<OrchestratorRun>>,
    run_id: RunId,
) {
    spawn(async move {
        loop {
            match rx.recv().await {
                Ok((agent_id, claude_output)) => {
                    let is_final = claude_output.is_final();
                    if let Some(msg) = convert_output(claude_output) {
                        if let Some(session_id) = agent_sessions.get(&agent_id) {
                            if let Some(session) =
                                sessions.write().get_mut(session_id)
                            {
                                session.messages.push(msg);
                            }
                        }
                    }
                    // Don't break on final — other agents may still produce output
                    let _ = is_final;
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("Orchestrator UI relay lagged by {n} messages");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }

        // Run finished — update status
        mark_run_completed(&mut runs, &mut sessions, run_id, &agent_sessions);
    });
}

fn mark_run_failed(
    runs: &mut Signal<Vec<OrchestratorRun>>,
    sessions: &mut Signal<HashMap<SessionId, Session>>,
    run_id: RunId,
    agent_sessions: &HashMap<AgentId, SessionId>,
    reason: &str,
) {
    let mut runs_write = runs.write();
    if let Some(run) = runs_write.iter_mut().find(|r| r.id == run_id) {
        run.status = RunStatus::Failed(reason.to_string());
    }
    drop(runs_write);

    let mut sessions_write = sessions.write();
    for session_id in agent_sessions.values() {
        if let Some(session) = sessions_write.get_mut(session_id) {
            session.status = SessionStatus::Error(reason.to_string());
        }
    }
}

fn mark_run_completed(
    runs: &mut Signal<Vec<OrchestratorRun>>,
    sessions: &mut Signal<HashMap<SessionId, Session>>,
    run_id: RunId,
    agent_sessions: &HashMap<AgentId, SessionId>,
) {
    let mut runs_write = runs.write();
    if let Some(run) = runs_write.iter_mut().find(|r| r.id == run_id) {
        if run.status == RunStatus::Running {
            run.status = RunStatus::Completed;
        }
    }
    drop(runs_write);

    let mut sessions_write = sessions.write();
    for session_id in agent_sessions.values() {
        if let Some(session) = sessions_write.get_mut(session_id) {
            if matches!(session.status, SessionStatus::Running) {
                session.status = SessionStatus::Idle;
            }
        }
    }
}

#[component]
fn RunEntry(
    run_idx: usize,
    run_id: RunId,
    goal: String,
    status: RunStatus,
    agent_sessions: HashMap<AgentId, SessionId>,
) -> Element {
    let mut expanded = use_signal(|| true);
    let mut runs: Signal<Vec<OrchestratorRun>> = use_context();
    let mut sessions: Signal<HashMap<SessionId, Session>> = use_context();

    let status_badge_class = match &status {
        RunStatus::Running => "badge badge-running",
        RunStatus::Completed => "badge badge-idle",
        RunStatus::Failed(_) => "badge badge-error",
    };
    let status_label = match &status {
        RunStatus::Running => "run",
        RunStatus::Completed => "done",
        RunStatus::Failed(_) => "fail",
    };

    let toggle_icon = if expanded() { "\u{25bc}" } else { "\u{25b6}" };
    let is_running = status == RunStatus::Running;

    rsx! {
        div {
            class: "run-entry",
            div {
                class: "run-header",
                onclick: move |_| expanded.set(!expanded()),
                span { class: "toggle-icon", "{toggle_icon}" }
                span { class: status_badge_class, "{status_label}" }
                span {
                    class: "flex-1 truncate",
                    "{goal}"
                }
                if is_running {
                    span {
                        class: "btn-danger-text",
                        onclick: move |evt| {
                            evt.stop_propagation();
                            abort_run(&mut runs, &mut sessions, run_idx);
                        },
                        "abort"
                    }
                }
            }
            if expanded() {
                RunAgentList { agent_sessions }
            }
        }
    }
}

#[component]
fn RunAgentList(agent_sessions: HashMap<AgentId, SessionId>) -> Element {
    let mut active_id: Signal<Option<SessionId>> = use_context();
    let mut sessions: Signal<HashMap<SessionId, Session>> = use_context();

    let mut agents: Vec<_> = agent_sessions.iter().collect();
    agents.sort_by_key(|(id, _)| format!("{id}"));

    rsx! {
        div {
            class: "collapsible-content",
            for (agent_id, session_id) in agents {
                AgentEntry {
                    agent_id: agent_id.clone(),
                    session_id: *session_id,
                    is_active: active_id() == Some(*session_id),
                    on_click: {
                        let sid = *session_id;
                        move |_| active_id.set(Some(sid))
                    },
                    on_reset: {
                        let sid = *session_id;
                        move |_| {
                            if let Some(session) = sessions.write().get_mut(&sid) {
                                session.messages.clear();
                            }
                        }
                    },
                }
            }
        }
    }
}

fn abort_run(
    runs: &mut Signal<Vec<OrchestratorRun>>,
    sessions: &mut Signal<HashMap<SessionId, Session>>,
    run_idx: usize,
) {
    let mut runs_write = runs.write();
    if let Some(run) = runs_write.get_mut(run_idx) {
        if let Some(handle) = &run.run_handle {
            handle.abort();
        }
        run.status = RunStatus::Failed("aborted".to_string());

        let agent_sids: Vec<_> = run.agent_sessions.values().copied().collect();
        drop(runs_write);

        let mut sessions_write = sessions.write();
        for sid in agent_sids {
            if let Some(session) = sessions_write.get_mut(&sid) {
                session.status = SessionStatus::Error("run aborted".to_string());
            }
        }
    }
}

#[component]
fn AgentEntry(
    agent_id: AgentId,
    session_id: SessionId,
    is_active: bool,
    on_click: EventHandler<()>,
    on_reset: EventHandler<()>,
) -> Element {
    let active_class = if is_active { " active" } else { "" };
    let label = format!("{agent_id}");
    let is_developer = label.starts_with("developer");

    rsx! {
        div {
            class: "agent-entry{active_class}",
            onclick: move |_| on_click.call(()),
            span {
                class: "agent-entry-label",
                "{label}"
            }
            if is_developer {
                span {
                    class: "btn-danger-text",
                    onclick: move |evt| {
                        evt.stop_propagation();
                        on_reset.call(());
                    },
                    "reset"
                }
            }
        }
    }
}
