use dioxus::prelude::*;

use super::message::MessageView;
use super::prompt::PromptInput;
use crate::state::ChatMessage;

type Selection = Option<(String, String)>;

#[component]
pub fn ChatPanel() -> Element {
    let selected = use_context::<Signal<Selection>>();
    let messages = use_context::<Signal<Vec<ChatMessage>>>();

    let sel = selected.read().clone();
    let Some((ref _project, ref agent)) = sel else {
        return rsx! {
            div { class: "chat-area",
                div { class: "chat-empty", "Select an agent to view conversation" }
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
            PromptInput {
                disabled: false,
                on_submit: move |prompt: String| {
                    send_to_agent(selected, prompt);
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

fn send_to_agent(selected: Signal<Selection>, prompt: String) {
    let sel = selected.read().clone();
    let Some((ref _project, ref agent)) = sel else {
        return;
    };
    let agent = agent.clone();
    spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            crate::ipc::send_message(&agent, &prompt)
        })
        .await;
        match result {
            Ok(Ok(crate::ipc::ControlResponse::Ok)) => {}
            Ok(Ok(crate::ipc::ControlResponse::Error { message })) => {
                tracing::warn!("IPC send error: {message}");
            }
            Ok(Err(e)) => tracing::warn!("IPC call failed: {e}"),
            Err(e) => tracing::warn!("spawn_blocking failed: {e}"),
            _ => {}
        }
    });
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
