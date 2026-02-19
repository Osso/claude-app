use std::collections::HashMap;
use std::path::{Path, PathBuf};

use dioxus::prelude::*;

use crate::claude::SessionManager;
use crate::state::{OrchestratorRun, Session, SessionId};

const MAX_RECENT: usize = 20;

fn storage_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".claude-sessions")
        .join("recent-projects.json")
}

fn load_recent() -> Vec<PathBuf> {
    std::fs::read_to_string(storage_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_recent(project: &Path) {
    let mut list = load_recent();
    list.retain(|p| p != project);
    list.insert(0, project.to_path_buf());
    list.truncate(MAX_RECENT);

    let path = storage_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, serde_json::to_string(&list).unwrap_or_default());
}

/// Scan ~/Projects/ (two levels deep) for git repos as initial suggestions.
fn discover_projects() -> Vec<PathBuf> {
    let projects_dir = dirs::home_dir().unwrap_or_default().join("Projects");
    let mut found = Vec::new();

    let Ok(entries) = std::fs::read_dir(&projects_dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.join(".git").exists() {
            found.push(path.clone());
        }
        // One level deeper (e.g. ~/Projects/cli/*)
        if let Ok(sub) = std::fs::read_dir(&path) {
            for sub_entry in sub.flatten() {
                let sub_path = sub_entry.path();
                if sub_path.is_dir() && sub_path.join(".git").exists() {
                    found.push(sub_path);
                }
            }
        }
    }
    found.sort();
    found
}

fn dir_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

fn open_project(
    path: PathBuf,
    mut project_path: Signal<Option<PathBuf>>,
    mut sessions: Signal<HashMap<SessionId, Session>>,
    mut active_id: Signal<Option<SessionId>>,
    mut manager: Signal<SessionManager>,
    mut runs: Signal<Vec<OrchestratorRun>>,
) {
    let ids: Vec<_> = sessions.read().keys().copied().collect();
    for id in ids {
        manager.write().abort_session(id);
    }
    sessions.write().clear();
    active_id.set(None);

    {
        let runs_read = runs.read();
        for run in runs_read.iter() {
            if let Some(handle) = &run.run_handle {
                handle.abort();
            }
        }
    }
    runs.write().clear();

    save_recent(&path);
    let path_clone = path.clone();
    project_path.set(Some(path));

    // Load persisted sessions
    let loaded = crate::persist::load_sessions(&path_clone);
    if !loaded.is_empty() {
        let first_id = loaded.keys().next().copied();
        *sessions.write() = loaded;
        active_id.set(first_id);
    }
}

// -- Project Picker (full-page, initial state) --

/// Full-page project picker shown when no project is selected.
#[component]
pub fn ProjectPicker() -> Element {
    let project_path: Signal<Option<PathBuf>> = use_context();
    let sessions: Signal<HashMap<SessionId, Session>> = use_context();
    let active_id: Signal<Option<SessionId>> = use_context();
    let manager: Signal<SessionManager> = use_context();
    let runs: Signal<Vec<OrchestratorRun>> = use_context();

    let recent = load_recent();
    let (projects, section_label) = if recent.is_empty() {
        (discover_projects(), "projects")
    } else {
        (recent, "recent")
    };

    let on_open = move |p: PathBuf| {
        if p.is_dir() {
            open_project(p, project_path, sessions, active_id, manager, runs);
        }
    };

    rsx! {
        div {
            class: "project-picker",
            div { class: "project-picker-title", "open project" }
            if !projects.is_empty() {
                div {
                    class: "project-picker-recent",
                    div { class: "project-picker-section", "{section_label}" }
                    for path in projects {
                        PickerItem {
                            path: path.clone(),
                            on_select: move |p: PathBuf| on_open(p),
                        }
                    }
                }
            }
            PickerPathInput {
                on_open: move |p: PathBuf| on_open(p),
            }
        }
    }
}

#[component]
fn PickerItem(path: PathBuf, on_select: EventHandler<PathBuf>) -> Element {
    let name = dir_name(&path);
    let display = path.to_string_lossy().to_string();

    rsx! {
        div {
            class: "project-picker-item",
            onclick: {
                let path = path.clone();
                move |_| on_select.call(path.clone())
            },
            div { class: "project-picker-item-name", "{name}" }
            div { class: "project-picker-item-path", "{display}" }
        }
    }
}

#[component]
fn PickerPathInput(on_open: EventHandler<PathBuf>) -> Element {
    let mut input_value = use_signal(|| {
        std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    });

    rsx! {
        div {
            class: "project-picker-manual",
            div { class: "project-picker-section", "path" }
            div {
                class: "project-picker-input-row",
                input {
                    class: "project-picker-input",
                    value: "{input_value}",
                    placeholder: "/path/to/project",
                    oninput: move |evt| input_value.set(evt.value()),
                    onkeydown: move |evt| {
                        if evt.key() == Key::Enter {
                            on_open.call(PathBuf::from(
                                input_value().trim().to_string(),
                            ));
                        }
                    },
                }
                button {
                    class: "btn btn-primary",
                    onclick: move |_| {
                        on_open.call(PathBuf::from(
                            input_value().trim().to_string(),
                        ));
                    },
                    "open"
                }
            }
        }
    }
}

// -- Project Switcher (sidebar dropdown) --

/// Sidebar project switcher with dropdown for recent projects.
#[component]
pub fn ProjectSwitcher() -> Element {
    let project_path: Signal<Option<PathBuf>> = use_context();
    let sessions: Signal<HashMap<SessionId, Session>> = use_context();
    let active_id: Signal<Option<SessionId>> = use_context();
    let manager: Signal<SessionManager> = use_context();
    let runs: Signal<Vec<OrchestratorRun>> = use_context();

    let mut expanded = use_signal(|| false);
    let current = project_path().unwrap_or_default();
    let name = dir_name(&current);
    let dir = current.to_string_lossy().to_string();
    let toggle = if expanded() { "\u{25b4}" } else { "\u{25be}" };

    rsx! {
        div {
            class: "project-switcher",
            div {
                class: "project-switcher-header",
                onclick: move |_| expanded.set(!expanded()),
                span { class: "project-switcher-toggle", "{toggle}" }
                div {
                    class: "project-switcher-info",
                    div { class: "project-switcher-name", "{name}" }
                    div { class: "project-switcher-path", "{dir}" }
                }
            }
            if expanded() {
                SwitcherDropdown {
                    current: current.clone(),
                    on_select: move |p: PathBuf| {
                        open_project(
                            p, project_path, sessions,
                            active_id, manager, runs,
                        );
                        expanded.set(false);
                    },
                    on_close: move |_| expanded.set(false),
                }
            }
        }
    }
}

#[component]
fn SwitcherDropdown(
    current: PathBuf,
    on_select: EventHandler<PathBuf>,
    on_close: EventHandler<()>,
) -> Element {
    let filter_text = use_signal(String::new);
    let needle = filter_text().to_lowercase();
    let filtered: Vec<_> = load_recent()
        .into_iter()
        .filter(|p| {
            needle.is_empty()
                || p.to_string_lossy().to_lowercase().contains(&needle)
        })
        .collect();

    rsx! {
        div {
            class: "project-switcher-dropdown",
            SwitcherFilter { filter_text, on_select, on_close }
            div {
                class: "project-switcher-list",
                for path in filtered {
                    SwitcherItem {
                        path: path.clone(),
                        is_current: path == current,
                        on_select: move |p: PathBuf| on_select.call(p),
                    }
                }
            }
        }
    }
}

#[component]
fn SwitcherFilter(
    mut filter_text: Signal<String>,
    on_select: EventHandler<PathBuf>,
    on_close: EventHandler<()>,
) -> Element {
    rsx! {
        input {
            class: "project-switcher-filter",
            value: "{filter_text}",
            placeholder: "filter or type path\u{2026}",
            oninput: move |evt| filter_text.set(evt.value()),
            onkeydown: move |evt| {
                match evt.key() {
                    Key::Escape => on_close.call(()),
                    Key::Enter => {
                        let path = PathBuf::from(
                            filter_text().trim().to_string(),
                        );
                        if path.is_dir() {
                            on_select.call(path);
                        }
                    }
                    _ => {}
                }
            },
        }
    }
}

#[component]
fn SwitcherItem(
    path: PathBuf,
    is_current: bool,
    on_select: EventHandler<PathBuf>,
) -> Element {
    let name = dir_name(&path);
    let display = path.to_string_lossy().to_string();
    let class = if is_current {
        "project-switcher-item current"
    } else {
        "project-switcher-item"
    };

    rsx! {
        div {
            class,
            onclick: {
                let path = path.clone();
                move |_| {
                    if !is_current {
                        on_select.call(path.clone());
                    }
                }
            },
            div { class: "project-switcher-item-name", "{name}" }
            div { class: "project-switcher-item-path", "{display}" }
        }
    }
}
