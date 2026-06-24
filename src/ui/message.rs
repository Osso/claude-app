#![cfg_attr(coverage_nightly, coverage(off))]

use dioxus::prelude::*;

use super::diff::render_assistant_text;
use crate::state::ChatMessage;

#[component]
pub fn MessageView(message: ChatMessage) -> Element {
    match message {
        ChatMessage::User { text, timestamp } => rsx! {
            UserMessage { text, timestamp }
        },
        ChatMessage::Assistant {
            text,
            timestamp,
            usage,
        } => rsx! {
            AssistantMessage { text, timestamp, usage }
        },
    }
}

#[component]
fn UserMessage(text: String, timestamp: String) -> Element {
    let timestamp_label = formatted_timestamp(&timestamp);
    let timestamp_node = timestamp_label.map(|label| {
        rsx! {
            span { class: "message-timestamp text-xs text-inactive", "{label}" }
        }
    });

    rsx! {
        div { class: "message message-user",
            {timestamp_node}
            "{text}"
        }
    }
}

#[component]
fn AssistantMessage(
    text: String,
    timestamp: String,
    usage: Option<crate::state::TokenUsage>,
) -> Element {
    let html = render_assistant_text(&text);
    let timestamp_label = formatted_timestamp(&timestamp);
    let usage_label = usage
        .as_ref()
        .map(|token_usage| format!("{}in/{}out", token_usage.input, token_usage.output));
    let timestamp_node = timestamp_label.map(|label| {
        rsx! {
            span { class: "message-timestamp text-xs text-inactive", "{label}" }
        }
    });
    let usage_node = usage_label.map(|label| {
        rsx! {
            span { class: "message-tokens text-xs text-inactive",
                "{label}"
            }
        }
    });

    rsx! {
        div { class: "message message-assistant",
            div { class: "message-meta",
                {timestamp_node}
                {usage_node}
            }
            div { dangerous_inner_html: html }
        }
    }
}

/// Extract HH:MM from ISO-8601 timestamp
fn format_time(ts: &str) -> String {
    // "2026-03-03T14:30:00Z" → "14:30"
    if let Some(t_pos) = ts.find('T') {
        let time_part = &ts[t_pos + 1..];
        if time_part.len() >= 5 {
            return time_part[..5].to_string();
        }
    }
    ts.to_string()
}

fn formatted_timestamp(timestamp: &str) -> Option<String> {
    if timestamp.is_empty() {
        return None;
    }
    Some(format_time(timestamp))
}
