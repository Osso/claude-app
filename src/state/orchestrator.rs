use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::orchestrator::types::AgentId;
use crate::orchestrator::RunHandle;

use super::SessionId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunId(Uuid);

impl RunId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl fmt::Display for RunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum RunStatus {
    Running,
    Completed,
    Failed(String),
}

pub struct OrchestratorRun {
    pub id: RunId,
    pub goal: String,
    pub agent_sessions: HashMap<AgentId, SessionId>,
    pub status: RunStatus,
    pub run_handle: Option<RunHandle>,
}

impl fmt::Debug for OrchestratorRun {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OrchestratorRun")
            .field("id", &self.id)
            .field("goal", &self.goal)
            .field("agent_sessions", &self.agent_sessions)
            .field("status", &self.status)
            .finish()
    }
}
