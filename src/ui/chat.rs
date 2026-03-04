use dioxus::prelude::*;

use super::message::MessageView;
use super::prompt::PromptInput;
use crate::state::ChatMessage;

type Selection = Option<(String, String)>;
type ErrorMessage = Option<String>;

#[component]
pub fn ChatPanel() -> Element {
    let selected = use_context::<Signal<Selection>>();
    let messages = use_context::<Signal<Vec<ChatMessage>>>();
    let error = use_context::<Signal<ErrorMessage>>();

    let sel = selected.read().clone();
    let Some((ref _project, ref agent)) = sel else {
        return rsx! {
            div { class: "chat-area",
                super::launch::LaunchForm {}
            }
        };
    };

    let agent_name = agent.clone();
    let msgs = messages.read().clone();
    let msg_count = msgs.len();

    // Cumulative token totals
    let (total_input, total_output) = cumulative_tokens(&msgs);

    use_effect(move || {
        let _ = msg_count;
        document::eval(
            "let el = document.getElementById('chat-messages'); if (el) el.scrollTop = el.scrollHeight;",
        );
    });

    rsx! {
        div { class: "chat-area",
            AgentHeader { name: agent_name, total_input, total_output }
            MessageList { messages: msgs }
            ErrorBanner {}
            PromptInput {
                disabled: false,
                on_submit: move |prompt: String| {
                    send_to_agent(selected, error, prompt);
                }
            }
        }
    }
}

#[component]
fn AgentHeader(name: String, total_input: u64, total_output: u64) -> Element {
    rsx! {
        div { class: "agent-header",
            span { class: "agent-header-name", "{name}" }
            if total_input > 0 || total_output > 0 {
                span { class: "agent-header-tokens text-sm text-subtle",
                    "{format_tokens(total_input)}in / {format_tokens(total_output)}out"
                }
            }
        }
    }
}

#[component]
fn ErrorBanner() -> Element {
    let mut error = use_context::<Signal<ErrorMessage>>();
    let err = error.read().clone();
    let Some(err_msg) = err else {
        return rsx! {};
    };
    rsx! {
        div { class: "ipc-error",
            span { "{err_msg}" }
            button {
                class: "ipc-error-dismiss",
                onclick: move |_| error.set(None),
                "×"
            }
        }
    }
}

#[component]
fn MessageList(messages: Vec<ChatMessage>) -> Element {
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

fn send_to_agent(selected: Signal<Selection>, mut error: Signal<ErrorMessage>, prompt: String) {
    let sel = selected.read().clone();
    let Some((ref _project, ref agent)) = sel else {
        return;
    };
    let agent = agent.clone();
    error.set(None);
    spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            crate::ipc::send_message(&agent, &prompt)
        })
        .await;
        match result {
            Ok(Ok(crate::ipc::ControlResponse::Ok)) => {}
            other => error.set(Some(format_ipc_error(other))),
        }
    });
}

fn format_ipc_error(result: Result<Result<crate::ipc::ControlResponse, peercred_ipc::IpcError>, tokio::task::JoinError>) -> String {
    match result {
        Ok(Ok(crate::ipc::ControlResponse::Error { message })) => message,
        Ok(Err(e)) => format!("Socket error: {e}"),
        Err(e) => format!("Internal error: {e}"),
        _ => "Unknown error".to_string(),
    }
}

fn cumulative_tokens(messages: &[ChatMessage]) -> (u64, u64) {
    let mut input = 0u64;
    let mut output = 0u64;
    for msg in messages {
        if let ChatMessage::Assistant { usage: Some(u), .. } = msg {
            input += u.input;
            output += u.output;
        }
    }
    (input, output)
}

fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M ", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k ", n as f64 / 1_000.0)
    } else {
        format!("{n} ")
    }
}
