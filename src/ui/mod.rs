pub mod chat;
pub mod diff;
pub mod launch;
pub mod message;
pub mod prompt;
pub mod sidebar;

use dioxus::prelude::*;

use crate::state::{self, ChatMessage, Project};
use crate::watcher;

/// Selected agent: (project_name, agent_name)
type Selection = Option<(String, String)>;


fn load_agent_messages(project: &str, agent: &str, offset: u64) -> (Vec<ChatMessage>, u64) {
    match state::jsonl_path_for(project, agent) {
        Some(path) => {
            let (msgs, off, _) = state::parse_jsonl_from_offset(&path, offset);
            (msgs, off)
        }
        None => (Vec::new(), 0),
    }
}

fn handle_jsonl_changed(
    path: std::path::PathBuf,
    selected: Signal<Selection>,
    mut messages: Signal<Vec<ChatMessage>>,
    mut offset: Signal<u64>,
) {
    let sel = selected.read().clone();
    let Some((ref project, ref agent)) = sel else {
        return;
    };
    let expected = state::jsonl_path_for(project, agent);
    if expected.as_ref() != Some(&path) {
        return;
    }
    let cur_offset = *offset.read();
    let (new_msgs, new_off, had_reset) = state::parse_jsonl_from_offset(&path, cur_offset);
    if had_reset {
        messages.set(new_msgs);
    } else if !new_msgs.is_empty() {
        messages.write().extend(new_msgs);
    }
    offset.set(new_off);
}

fn setup_selection_effect(
    selected: Signal<Selection>,
    mut messages: Signal<Vec<ChatMessage>>,
    mut offset: Signal<u64>,
) {
    use_effect(move || {
        let sel = selected.read().clone();
        messages.set(Vec::new());
        offset.set(0);
        let Some((ref project, ref agent)) = sel else {
            return;
        };
        let (msgs, new_off) = load_agent_messages(project, agent, 0);
        messages.set(msgs);
        offset.set(new_off);
    });
}

fn setup_watcher_future(
    mut projects: Signal<Vec<Project>>,
    messages: Signal<Vec<ChatMessage>>,
    offset: Signal<u64>,
    selected: Signal<Selection>,
) {
    use_future(move || async move {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        std::thread::spawn(move || {
            let std_rx = watcher::start_watcher();
            while let Ok(event) = std_rx.recv() {
                if tx.send(event).is_err() {
                    break;
                }
            }
        });
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            while let Ok(event) = rx.try_recv() {
                match event {
                    watcher::WatchEvent::ProjectsChanged => {
                        projects.set(state::load_projects());
                    }
                    watcher::WatchEvent::JsonlChanged(path) => {
                        handle_jsonl_changed(path, selected, messages, offset);
                    }
                }
            }
        }
    });
}

#[component]
pub fn App() -> Element {
    let mut projects = use_context_provider(|| Signal::new(Vec::<Project>::new()));
    let selected = use_context_provider(|| Signal::new(Option::<(String, String)>::None));
    let messages = use_context_provider(|| Signal::new(Vec::<ChatMessage>::new()));
    let offset = use_context_provider(|| Signal::new(0u64));
    let _error = use_context_provider(|| Signal::new(Option::<String>::None));

    use_effect(move || {
        projects.set(state::load_projects());
    });

    setup_selection_effect(selected, messages, offset);
    setup_watcher_future(projects, messages, offset, selected);

    rsx! {
        div { class: "app",
            div { class: "drag-region" }
            div { class: "app-body",
                sidebar::Sidebar {}
                chat::ChatPanel {}
            }
        }
    }
}
