mod chat;
pub mod diff;
mod message;
mod prompt;
mod sidebar;

use std::collections::HashMap;
use std::path::PathBuf;

use dioxus::prelude::*;

use crate::claude::SessionManager;
use crate::state::{OrchestratorRun, Session, SessionId};

use self::chat::ChatFeed;
use self::sidebar::Sidebar;

#[component]
pub fn App() -> Element {
    let _sessions: Signal<HashMap<SessionId, Session>> =
        use_context_provider(|| Signal::new(HashMap::new()));
    let _active_session: Signal<Option<SessionId>> =
        use_context_provider(|| Signal::new(None));
    let _session_manager: Signal<SessionManager> =
        use_context_provider(|| Signal::new(SessionManager::new()));
    let _orchestrator_runs: Signal<Vec<OrchestratorRun>> =
        use_context_provider(|| Signal::new(Vec::new()));
    let project_path: Signal<Option<PathBuf>> =
        use_context_provider(|| Signal::new(None));

    rsx! {
        div {
            style: "display: flex; height: 100vh; width: 100vw; background: #1a1a2e; color: #e0e0e0; font-family: monospace; font-size: 14px;",
            if project_path().is_some() {
                Sidebar {}
                ChatFeed {}
            } else {
                ProjectPicker {}
            }
        }
    }
}

#[component]
fn ProjectPicker() -> Element {
    let mut project_path: Signal<Option<PathBuf>> = use_context();
    let mut input_value = use_signal(|| {
        std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    });

    rsx! {
        div {
            style: "flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 16px;",
            div {
                style: "font-size: 1.2em; color: #888;",
                "Select a project directory"
            }
            input {
                style: "width: 500px; max-width: 80%; padding: 10px; background: #222244; color: #e0e0e0; border: 1px solid #444; border-radius: 4px; font-family: monospace; font-size: 0.95em;",
                value: "{input_value}",
                oninput: move |evt| input_value.set(evt.value()),
                onkeydown: move |evt| {
                    if evt.key() == Key::Enter {
                        let path = PathBuf::from(input_value().trim().to_string());
                        if path.is_dir() {
                            project_path.set(Some(path));
                        }
                    }
                },
            }
            button {
                style: "padding: 10px 24px; background: #333355; color: #e0e0e0; border: none; border-radius: 4px; cursor: pointer; font-size: 0.95em;",
                onclick: move |_| {
                    let path = PathBuf::from(input_value().trim().to_string());
                    if path.is_dir() {
                        project_path.set(Some(path));
                    }
                },
                "Open"
            }
        }
    }
}
