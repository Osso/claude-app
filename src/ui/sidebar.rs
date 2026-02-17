use std::collections::HashMap;
use std::path::PathBuf;

use dioxus::prelude::*;

use crate::claude::{SessionManager, convert_output};
use crate::orchestrator::types::AgentId;
use crate::orchestrator::OrchestratorRuntime;
use crate::state::{
    OrchestratorRun, RunId, RunStatus, Session, SessionId, SessionStatus,
};

const SIDEBAR_BG: &str = "#16162a";
const ACTIVE_BG: &str = "#2a2a4a";

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
            style: "width: 250px; min-width: 250px; background: {SIDEBAR_BG}; display: flex; flex-direction: column; border-right: 1px solid #333;",
            NewSessionButton {}
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

    rsx! {
        div {
            style: "padding: 12px; border-bottom: 1px solid #333;",
            button {
                style: "width: 100%; padding: 8px; background: #333355; color: #e0e0e0; border: none; border-radius: 4px; cursor: pointer; font-size: 0.9em;",
                onclick: move |_| {
                    spawn(async move {
                        let project_path: Signal<Option<PathBuf>> = use_context();
                        let project_path = project_path().unwrap_or_default();
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
            style: "flex: 1; overflow-y: auto; padding: 4px 0;",
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
    let bg = if is_active { ACTIVE_BG } else { "transparent" };
    let text_weight = if is_active { "bold" } else { "normal" };

    rsx! {
        div {
            style: "padding: 8px 12px; cursor: pointer; background: {bg}; display: flex; align-items: center; gap: 8px;",
            onclick: move |_| on_click.call(()),
            StatusDot { status: status.clone() }
            span {
                style: "flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: {text_weight};",
                "{title}"
            }
            span {
                style: "color: #666; font-size: 0.8em; padding: 2px 4px; border-radius: 2px;",
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
fn StatusDot(status: SessionStatus) -> Element {
    let color = match status {
        SessionStatus::Idle => "#4caf50",
        SessionStatus::Running => "#ffb74d",
        SessionStatus::Error(_) => "#ff6b6b",
    };

    rsx! {
        span {
            style: "width: 8px; height: 8px; border-radius: 50%; background: {color}; flex-shrink: 0;",
        }
    }
}

// --- Orchestrator section ---

#[component]
fn OrchestratorSection() -> Element {
    let runs: Signal<Vec<OrchestratorRun>> = use_context();
    let runs_read = runs.read();

    rsx! {
        div {
            style: "border-top: 1px solid #333; display: flex; flex-direction: column;",
            NewRunButton {}
            div {
                style: "overflow-y: auto; max-height: 300px;",
                for (idx, run) in runs_read.iter().enumerate() {
                    RunEntry { run_idx: idx, run_id: run.id, goal: run.goal.clone(), status: run.status.clone(), agent_sessions: run.agent_sessions.clone() }
                }
            }
        }
    }
}

#[component]
fn NewRunButton() -> Element {
    let mut show_input = use_signal(|| false);

    rsx! {
        div {
            style: "padding: 12px;",
            if show_input() {
                GoalInputForm { on_close: move |_| show_input.set(false) }
            } else {
                button {
                    style: "width: 100%; padding: 8px; background: #333355; color: #e0e0e0; border: none; border-radius: 4px; cursor: pointer; font-size: 0.9em;",
                    onclick: move |_| show_input.set(true),
                    "+ New Run"
                }
            }
        }
    }
}

#[component]
fn GoalInputForm(on_close: EventHandler<()>) -> Element {
    let mut goal_input = use_signal(|| String::new());
    let mut sessions: Signal<HashMap<SessionId, Session>> = use_context();
    let mut runs: Signal<Vec<OrchestratorRun>> = use_context();
    let mut active_id: Signal<Option<SessionId>> = use_context();

    let mut submit = move || {
        let goal = goal_input().trim().to_string();
        if !goal.is_empty() {
            start_run(goal, &mut sessions, &mut runs, &mut active_id);
            goal_input.set(String::new());
            on_close.call(());
        }
    };

    rsx! {
        div {
            style: "display: flex; flex-direction: column; gap: 6px;",
            input {
                style: "width: 100%; padding: 6px; background: #222244; color: #e0e0e0; border: 1px solid #444; border-radius: 4px; font-size: 0.85em; box-sizing: border-box;",
                placeholder: "Goal for orchestrator...",
                value: "{goal_input}",
                oninput: move |evt| goal_input.set(evt.value()),
                onkeydown: move |evt| {
                    if evt.key() == Key::Enter {
                        submit();
                    } else if evt.key() == Key::Escape {
                        goal_input.set(String::new());
                        on_close.call(());
                    }
                },
            }
            div {
                style: "display: flex; gap: 4px;",
                button {
                    style: "flex: 1; padding: 6px; background: #333355; color: #e0e0e0; border: none; border-radius: 4px; cursor: pointer; font-size: 0.85em;",
                    onclick: move |_| submit(),
                    "Start"
                }
                button {
                    style: "padding: 6px 10px; background: transparent; color: #666; border: 1px solid #444; border-radius: 4px; cursor: pointer; font-size: 0.85em;",
                    onclick: move |_| {
                        goal_input.set(String::new());
                        on_close.call(());
                    },
                    "Cancel"
                }
            }
        }
    }
}

fn start_run(
    goal: String,
    sessions: &mut Signal<HashMap<SessionId, Session>>,
    runs: &mut Signal<Vec<OrchestratorRun>>,
    active_id: &mut Signal<Option<SessionId>>,
) {
    let mut sessions = *sessions;
    let mut runs = *runs;
    let mut active_id = *active_id;

    spawn(async move {
        let project_path_signal: Signal<Option<PathBuf>> = use_context();
        let project_path = project_path_signal().unwrap_or_default();

        let run_handle = match OrchestratorRuntime::spawn_run(
            goal.clone(),
            project_path.clone(),
        )
        .await
        {
            Ok(h) => h,
            Err(e) => {
                tracing::error!("Failed to start orchestrator run: {e}");
                return;
            }
        };

        let run_id = RunId::new();
        let mut agent_sessions = HashMap::new();

        // Create a session per agent
        for agent_id in run_handle.agent_ids() {
            let session_id = SessionId::new();
            let session = Session {
                id: session_id,
                title: format!("{}", agent_id),
                messages: Vec::new(),
                status: SessionStatus::Running,
                worktree_path: project_path.clone(),
                project_path: project_path.clone(),
            };
            sessions.write().insert(session_id, session);
            agent_sessions.insert(agent_id.clone(), session_id);
        }

        // Select the first agent's session
        if let Some(first_sid) = agent_sessions.values().next().copied() {
            active_id.set(Some(first_sid));
        }

        // Subscribe before storing the run
        let rx = run_handle.subscribe();

        let run = OrchestratorRun {
            id: run_id,
            goal,
            agent_sessions: agent_sessions.clone(),
            status: RunStatus::Running,
            run_handle: Some(run_handle),
        };
        runs.write().push(run);

        // Background task: relay orchestrator output to sessions
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

    let status_color = match &status {
        RunStatus::Running => "#4caf50",
        RunStatus::Completed => "#888",
        RunStatus::Failed(_) => "#ff6b6b",
    };

    let toggle_icon = if expanded() { "v" } else { ">" };
    let is_running = status == RunStatus::Running;

    rsx! {
        div {
            style: "border-bottom: 1px solid #2a2a3a;",
            div {
                style: "padding: 8px 12px; cursor: pointer; display: flex; align-items: center; gap: 6px;",
                onclick: move |_| expanded.set(!expanded()),
                span { style: "color: #666; font-size: 0.8em; width: 12px;", "{toggle_icon}" }
                span {
                    style: "width: 8px; height: 8px; border-radius: 50%; background: {status_color}; flex-shrink: 0;",
                }
                span {
                    style: "flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 0.85em;",
                    "{goal}"
                }
                if is_running {
                    span {
                        style: "color: #ff6b6b; font-size: 0.8em; padding: 2px 6px; cursor: pointer;",
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
            style: "padding-left: 20px;",
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
    let bg = if is_active { ACTIVE_BG } else { "transparent" };
    let label = format!("{agent_id}");
    let is_developer = label.starts_with("developer");

    rsx! {
        div {
            style: "padding: 5px 8px; cursor: pointer; background: {bg}; display: flex; align-items: center; gap: 6px; font-size: 0.85em;",
            onclick: move |_| on_click.call(()),
            span {
                style: "flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                "{label}"
            }
            if is_developer {
                span {
                    style: "color: #666; font-size: 0.8em; padding: 1px 4px; cursor: pointer;",
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
