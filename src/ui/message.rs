use dioxus::prelude::*;

use crate::state::Message;

const USER_BG: &str = "#2a2a3a";
const ERROR_COLOR: &str = "#ff6b6b";
const SYSTEM_COLOR: &str = "#888888";
const TOOL_HEADER_BG: &str = "#252535";
const MSG_PADDING: &str = "8px 12px";

#[component]
pub fn MessageView(message: Message) -> Element {
    match message {
        Message::User { text } => rsx! { UserMessage { text } },
        Message::Assistant { text } => rsx! { AssistantMessage { text } },
        Message::ToolUse { id: _, name, input } => {
            let input_str = serde_json::to_string_pretty(&input).unwrap_or_default();
            rsx! { ToolUseMessage { name, input: input_str } }
        }
        Message::ToolResult { id: _, output, is_error } => {
            rsx! { ToolResultMessage { output, is_error } }
        }
        Message::System { session_id } => rsx! { SystemMessage { session_id } },
        Message::Error { text } => rsx! { ErrorMessage { text } },
    }
}

#[component]
fn UserMessage(text: String) -> Element {
    rsx! {
        div {
            style: "background: {USER_BG}; padding: {MSG_PADDING}; border-radius: 4px; margin: 4px 0; white-space: pre-wrap;",
            "{text}"
        }
    }
}

#[component]
fn AssistantMessage(text: String) -> Element {
    rsx! {
        div {
            style: "padding: {MSG_PADDING}; margin: 4px 0; font-family: monospace; white-space: pre-wrap;",
            "{text}"
        }
    }
}

#[component]
fn ToolUseMessage(name: String, input: String) -> Element {
    let mut expanded = use_signal(|| false);
    let arrow = if expanded() { "\u{25bc}" } else { "\u{25b6}" };

    rsx! {
        div {
            style: "margin: 4px 0; border-radius: 4px; overflow: hidden;",
            div {
                style: "background: {TOOL_HEADER_BG}; padding: 4px 12px; cursor: pointer; font-size: 0.9em; user-select: none;",
                onclick: move |_| expanded.set(!expanded()),
                "[{name}] {arrow}"
            }
            if expanded() {
                div {
                    style: "padding: 8px 12px; font-family: monospace; font-size: 0.85em; white-space: pre-wrap; background: #1e1e2e;",
                    "{input}"
                }
            }
        }
    }
}

#[component]
fn ToolResultMessage(output: String, is_error: bool) -> Element {
    let mut expanded = use_signal(|| false);
    let header = if is_error { "Error" } else { "Result" };
    let header_color = if is_error { ERROR_COLOR } else { "#aaaaaa" };
    let arrow = if expanded() { "\u{25bc}" } else { "\u{25b6}" };

    rsx! {
        div {
            style: "margin: 4px 0; border-radius: 4px; overflow: hidden;",
            div {
                style: "background: {TOOL_HEADER_BG}; padding: 4px 12px; cursor: pointer; font-size: 0.9em; color: {header_color}; user-select: none;",
                onclick: move |_| expanded.set(!expanded()),
                "{header} {arrow}"
            }
            if expanded() {
                div {
                    style: "padding: 8px 12px; font-family: monospace; font-size: 0.85em; white-space: pre-wrap; background: #1e1e2e;",
                    "{output}"
                }
            }
        }
    }
}

#[component]
fn SystemMessage(session_id: Option<String>) -> Element {
    let text = match session_id {
        Some(id) => format!("Session started: {id}"),
        None => "System message".to_string(),
    };

    rsx! {
        div {
            style: "padding: 2px 12px; color: {SYSTEM_COLOR}; font-size: 0.85em;",
            "{text}"
        }
    }
}

#[component]
fn ErrorMessage(text: String) -> Element {
    rsx! {
        div {
            style: "padding: {MSG_PADDING}; color: {ERROR_COLOR}; font-weight: bold;",
            "{text}"
        }
    }
}
