use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use crate::claude::session::SessionManager;
use crate::state::{Session, SessionId};
use crate::state::orchestrator::{OrchestratorRun, RunId};

pub struct AppState {
    pub sessions: RwLock<HashMap<SessionId, Session>>,
    pub manager: Mutex<SessionManager>,
    pub runs: RwLock<HashMap<RunId, OrchestratorRun>>,
    pub project_path: PathBuf,
    pub jwt_secret: String,
}

pub type SharedState = Arc<AppState>;
