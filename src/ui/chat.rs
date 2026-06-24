#![cfg_attr(coverage_nightly, coverage(off))]

use dioxus::prelude::*;

use super::message::MessageView;
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
                div { class: "empty-state",
                    "Select an agent to view messages"
                }
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
        }
    }
}

#[component]
fn AgentHeader(name: String, total_input: u64, total_output: u64) -> Element {
    let tokens_label = token_summary_label(total_input, total_output);
    rsx! {
        div { class: "agent-header",
            span { class: "agent-header-name", "{name}" }
            TokenSummary { label: tokens_label }
        }
    }
}

#[component]
fn MessageList(messages: Vec<ChatMessage>) -> Element {
    let message_nodes = messages
        .into_iter()
        .enumerate()
        .map(|(i, msg)| rsx! { MessageView { key: "{i}", message: msg } });

    rsx! {
        div {
            id: "chat-messages",
            class: "message-list",
            {message_nodes}
        }
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

fn token_summary_label(total_input: u64, total_output: u64) -> Option<String> {
    if total_input == 0 && total_output == 0 {
        return None;
    }
    Some(format!(
        "{}in / {}out",
        format_tokens(total_input),
        format_tokens(total_output)
    ))
}

#[component]
fn TokenSummary(label: Option<String>) -> Element {
    let Some(label) = label else {
        return rsx! {};
    };
    rsx! {
        span { class: "agent-header-tokens text-sm text-subtle",
            "{label}"
        }
    }
}
