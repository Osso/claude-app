use std::collections::HashMap;

use dioxus::prelude::*;

use super::message::MessageView;
use super::prompt::PromptInput;
use crate::claude::SessionManager;
use crate::state::{Message, OrchestratorRun, Session, SessionId, SessionStatus};

#[component]
pub fn ChatFeed() -> Element {
    let mut sessions: Signal<HashMap<SessionId, Session>> = use_context();
    let active_id: Signal<Option<SessionId>> = use_context();
    let mut manager: Signal<SessionManager> = use_context();
    let runs: Signal<Vec<OrchestratorRun>> = use_context();

    let active = active_id();
    let sessions_read = sessions.read();

    let Some(session_id) = active else {
        return rsx! {
            div { class: "chat-empty", "No session selected" }
        };
    };

    let Some(session) = sessions_read.get(&session_id) else {
        return rsx! {
            div { class: "chat-empty", "Session not found" }
        };
    };

    let messages = session.messages.clone();
    let msg_count = messages.len();

    use_effect(move || {
        let _ = msg_count;
        document::eval(
            "let el = document.getElementById('chat-messages'); if (el) el.scrollTop = el.scrollHeight;",
        );
    });

    rsx! {
        div {
            class: "chat-area",
            MessageList { messages }
            PromptInput {
                disabled: false,
                on_submit: move |prompt: String| {
                    submit_prompt(session_id, prompt, &mut sessions, &mut manager, &runs);
                }
            }
        }
    }
}

#[component]
fn MessageList(messages: Vec<Message>) -> Element {
    rsx! {
        div {
            id: "chat-messages",
            class: "message-list",
            for (i, msg) in messages.into_iter().enumerate() {
                MessageView { key: "{i}", message: msg }
            }
        }
    }
}

fn submit_prompt(
    session_id: SessionId,
    prompt: String,
    sessions: &mut Signal<HashMap<SessionId, Session>>,
    manager: &mut Signal<SessionManager>,
    runs: &Signal<Vec<OrchestratorRun>>,
) {
    // Add user message immediately
    if let Some(session) = sessions.write().get_mut(&session_id) {
        session.messages.push(Message::User { text: prompt.clone() });
    }

    // Check if this session belongs to an orchestrator run
    match try_send_to_agent(runs, session_id, &prompt) {
        Some(true) => return,
        Some(false) => {
            if let Some(session) = sessions.write().get_mut(&session_id) {
                session.messages.push(Message::Error {
                    text: "Failed to send message to agent".to_string(),
                });
            }
            return;
        }
        None => {} // not an orchestrator session, fall through
    }

    // Regular session — send to claude via SessionManager
    let result = manager
        .write()
        .send_prompt(session_id, &prompt, &mut sessions.write());

    let rx = match result {
        Ok(rx) => rx,
        Err(e) => {
            tracing::error!("Failed to send prompt: {e}");
            if let Some(session) = sessions.write().get_mut(&session_id) {
                session.status = SessionStatus::Error(e.to_string());
            }
            return;
        }
    };

    let sessions = *sessions;
    spawn(async move {
        drain_messages(rx, session_id, sessions).await;
    });
}

/// Try to send a user message to an orchestrator agent.
/// Returns None if session is not part of a run, Some(true) if sent, Some(false) if send failed.
fn try_send_to_agent(
    runs: &Signal<Vec<OrchestratorRun>>,
    session_id: SessionId,
    content: &str,
) -> Option<bool> {
    let runs_read = runs.read();
    for run in runs_read.iter() {
        if let Some(run_handle) = &run.run_handle {
            for (agent_id, sid) in &run.agent_sessions {
                if *sid == session_id {
                    return Some(run_handle.send_to_agent(agent_id, content.to_string()));
                }
            }
        }
    }
    None
}

async fn drain_messages(
    mut rx: tokio::sync::mpsc::Receiver<Message>,
    session_id: SessionId,
    mut sessions: Signal<HashMap<SessionId, Session>>,
) {
    while let Some(msg) = rx.recv().await {
        if let Some(session) = sessions.write().get_mut(&session_id) {
            session.messages.push(msg);
        }
    }
    // Stream ended — set back to idle
    if let Some(session) = sessions.write().get_mut(&session_id) {
        if matches!(session.status, SessionStatus::Running) {
            session.status = SessionStatus::Idle;
        }
    }
}
