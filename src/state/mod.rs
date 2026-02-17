pub mod orchestrator;

use std::fmt;
use std::path::PathBuf;

use serde_json::Value;
use uuid::Uuid;

pub use orchestrator::{OrchestratorRun, RunId, RunStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    User { text: String },
    Assistant { text: String },
    ToolUse { id: String, name: String, input: Value },
    ToolResult { id: String, output: String, is_error: bool },
    System { session_id: Option<String> },
    Error { text: String },
}

impl PartialEq for Message {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::User { text: a }, Self::User { text: b }) => a == b,
            (Self::Assistant { text: a }, Self::Assistant { text: b }) => a == b,
            (Self::ToolUse { id: a, .. }, Self::ToolUse { id: b, .. }) => a == b,
            (Self::ToolResult { id: a, .. }, Self::ToolResult { id: b, .. }) => a == b,
            (Self::System { session_id: a }, Self::System { session_id: b }) => a == b,
            (Self::Error { text: a }, Self::Error { text: b }) => a == b,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SessionStatus {
    Idle,
    Running,
    Error(String),
}

#[derive(Debug)]
pub struct Session {
    pub id: SessionId,
    pub title: String,
    pub messages: Vec<Message>,
    pub status: SessionStatus,
    pub worktree_path: PathBuf,
    pub project_path: PathBuf,
}
