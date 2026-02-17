use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::orchestrator::types::AgentId;
use crate::state::orchestrator::{RunId, RunStatus};
use crate::state::{Message, SessionId, SessionStatus};

// Auth
#[derive(Deserialize)]
pub struct AuthRequest {
    pub secret: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub token: String,
}

// Sessions
#[derive(Deserialize)]
pub struct CreateSessionRequest {
    pub title: String,
}

#[derive(Serialize)]
pub struct SessionSummary {
    pub id: SessionId,
    pub title: String,
    pub status: SessionStatus,
    pub message_count: usize,
}

#[derive(Serialize)]
pub struct SessionDetail {
    pub id: SessionId,
    pub title: String,
    pub status: SessionStatus,
    pub messages: Vec<Message>,
}

#[derive(Deserialize)]
pub struct PromptRequest {
    pub text: String,
}

// Runs
#[derive(Serialize)]
pub struct RunSummary {
    pub id: RunId,
    pub goal: String,
    pub status: RunStatus,
}

#[derive(Serialize)]
pub struct RunDetail {
    pub id: RunId,
    pub goal: String,
    pub status: RunStatus,
    pub agent_sessions: HashMap<AgentId, SessionId>,
}

#[derive(Deserialize)]
pub struct AgentMessageRequest {
    pub text: String,
}
