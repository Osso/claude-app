use dioxus::prelude::*;
use serde_json::Value;

use crate::state::Message;

#[component]
pub fn MessageView(message: Message) -> Element {
    match message {
        Message::User { text } => rsx! { UserMessage { text } },
        Message::Assistant { text } => rsx! { AssistantMessage { text } },
        Message::ToolUse { id: _, name, input } => {
            let preview = tool_preview(&name, &input);
            let input_str = serde_json::to_string_pretty(&input).unwrap_or_default();
            rsx! { ToolUseMessage { name, preview, input: input_str } }
        }
        Message::ToolResult { id: _, output, is_error } => {
            rsx! { ToolResultMessage { output, is_error } }
        }
        Message::System { session_id } => rsx! { SystemMessage { session_id } },
        Message::Error { text } => rsx! { ErrorMessage { text } },
    }
}

fn tool_preview(name: &str, input: &Value) -> String {
    let s = match name {
        "Bash" => input.get("command").and_then(Value::as_str),
        "Read" | "Write" => input.get("file_path").and_then(Value::as_str),
        "Edit" => input.get("file_path").and_then(Value::as_str),
        "Grep" | "Glob" => input.get("pattern").and_then(Value::as_str),
        "WebFetch" => input.get("url").and_then(Value::as_str),
        "WebSearch" => input.get("query").and_then(Value::as_str),
        _ => None,
    };
    s.unwrap_or("").to_string()
}

#[component]
fn UserMessage(text: String) -> Element {
    rsx! {
        div {
            class: "message message-user",
            "{text}"
        }
    }
}

#[component]
fn AssistantMessage(text: String) -> Element {
    rsx! {
        div {
            class: "message message-assistant",
            "{text}"
        }
    }
}

#[component]
fn ToolUseMessage(name: String, preview: String, input: String) -> Element {
    let mut expanded = use_signal(|| false);
    let arrow = if expanded() { "\u{25bc}" } else { "\u{25b6}" };

    rsx! {
        div {
            class: "message-tool",
            div {
                class: "message-tool-header",
                onclick: move |_| expanded.set(!expanded()),
                span { class: "toggle-icon", "{arrow}" }
                span { class: "tool-name", "{name}" }
                if !expanded() && !preview.is_empty() {
                    span { class: "tool-preview", "{preview}" }
                }
            }
            if expanded() {
                div {
                    class: "message-tool-body",
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
    let header_class = if is_error {
        "message-tool-header message-tool-error"
    } else {
        "message-tool-header"
    };
    let arrow = if expanded() { "\u{25bc}" } else { "\u{25b6}" };

    rsx! {
        div {
            class: "message-tool",
            div {
                class: header_class,
                onclick: move |_| expanded.set(!expanded()),
                span { class: "toggle-icon", "{arrow}" }
                span { "{header}" }
            }
            if expanded() {
                div {
                    class: "message-tool-body",
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
            class: "message-system",
            "{text}"
        }
    }
}

#[component]
fn ErrorMessage(text: String) -> Element {
    rsx! {
        div {
            class: "message message-error",
            "{text}"
        }
    }
}
