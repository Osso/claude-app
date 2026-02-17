mod chat;
pub mod diff;
mod message;
mod projects;
mod prompt;
mod sidebar;

use std::collections::HashMap;
use std::path::PathBuf;

use dioxus::prelude::*;

use crate::claude::SessionManager;
use crate::state::{OrchestratorRun, Session, SessionId};

use self::chat::ChatFeed;
use self::projects::ProjectPicker;
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
            class: "app",
            div { class: "drag-region" }
            div {
                class: "app-body",
                if project_path().is_some() {
                    Sidebar {}
                    ChatFeed {}
                } else {
                    ProjectPicker {}
                }
            }
        }
    }
}

