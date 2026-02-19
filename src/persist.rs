use std::collections::HashMap;
use std::path::Path;

use crate::state::{Session, SessionId, SessionStatus};
use crate::worktree::project_hash;

fn storage_dir(project_path: &Path) -> std::path::PathBuf {
    let hash = project_hash(project_path);
    dirs::home_dir()
        .unwrap_or_default()
        .join(".claude-sessions")
        .join("projects")
        .join(hash)
        .join("sessions")
}

pub fn save_session(session: &Session) {
    let dir = storage_dir(&session.project_path);
    if std::fs::create_dir_all(&dir).is_err() {
        tracing::error!("Failed to create session storage dir: {}", dir.display());
        return;
    }
    let path = dir.join(format!("{}.json", session.id));
    let tmp = dir.join(format!("{}.json.tmp", session.id));
    match serde_json::to_string(session) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&tmp, &json).and_then(|_| std::fs::rename(&tmp, &path)) {
                tracing::error!("Failed to save session {}: {e}", session.id);
                let _ = std::fs::remove_file(&tmp);
            }
        }
        Err(e) => tracing::error!("Failed to serialize session {}: {e}", session.id),
    }
}

pub fn load_sessions(project_path: &Path) -> HashMap<SessionId, Session> {
    let dir = storage_dir(project_path);
    let mut sessions = HashMap::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return sessions;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        match std::fs::read_to_string(&path).and_then(|s| {
            serde_json::from_str::<Session>(&s)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        }) {
            Ok(mut session) => {
                session.status = SessionStatus::Idle;
                sessions.insert(session.id, session);
            }
            Err(e) => {
                tracing::warn!("Skipping corrupt session file {}: {e}", path.display());
            }
        }
    }
    sessions
}

pub fn delete_session(session_id: SessionId, project_path: &Path) {
    let path = storage_dir(project_path).join(format!("{session_id}.json"));
    if let Err(e) = std::fs::remove_file(&path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            tracing::error!("Failed to delete session file {}: {e}", path.display());
        }
    }
}
