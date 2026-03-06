use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;

#[derive(Debug)]
pub enum WatchEvent {
    ProjectsChanged,
    JsonlChanged(PathBuf),
}

pub fn start_watcher() -> std_mpsc::Receiver<WatchEvent> {
    let (tx, rx) = std_mpsc::channel::<WatchEvent>();

    std::thread::spawn(move || {
        let (notify_tx, notify_rx) = std_mpsc::channel::<notify::Result<Event>>();

        let mut watcher = match RecommendedWatcher::new(notify_tx, Config::default()) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("Failed to create file watcher: {e}");
                return;
            }
        };

        watch_data_root(&mut watcher);

        loop {
            match notify_rx.recv() {
                Ok(Ok(event)) => {
                    if let Some(watch_event) = classify_event(event) {
                        if tx.send(watch_event).is_err() {
                            break;
                        }
                    }
                }
                Ok(Err(e)) => {
                    tracing::warn!("Watch error: {e}");
                }
                Err(_) => break,
            }
        }
    });

    rx
}

fn data_root() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("agent-orchestrator"))
}

fn watch_data_root(watcher: &mut RecommendedWatcher) {
    let root = match data_root() {
        Some(r) => r,
        None => {
            tracing::warn!("Could not resolve data root path");
            return;
        }
    };

    if let Err(e) = std::fs::create_dir_all(&root) {
        tracing::warn!("Failed to create data root {}: {e}", root.display());
        return;
    }

    // Watch recursively to catch new project dirs and new JSONL files
    if let Err(e) = watcher.watch(&root, RecursiveMode::Recursive) {
        tracing::warn!("Failed to watch {}: {e}", root.display());
    }
}

fn classify_event(event: Event) -> Option<WatchEvent> {
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {}
        _ => return None,
    }

    for path in &event.paths {
        if is_jsonl_file(path) {
            return Some(WatchEvent::JsonlChanged(path.clone()));
        }

        // Directory created/removed under data root = project change
        if is_project_dir_event(path) {
            return Some(WatchEvent::ProjectsChanged);
        }
    }

    None
}

fn is_jsonl_file(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e == "jsonl")
        .unwrap_or(false)
}

fn is_project_dir_event(path: &std::path::Path) -> bool {
    // A directory event directly under the data root (not in logs/)
    let root = match data_root() {
        Some(r) => r,
        None => return false,
    };
    path.parent() == Some(root.as_path())
}
