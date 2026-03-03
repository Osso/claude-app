use peercred_ipc::Client;
use serde::{Deserialize, Serialize};

const CONTROL_SOCKET: &str = "/tmp/claude/orchestrator/control.sock";

#[derive(Serialize, Deserialize, Debug)]
pub enum ControlRequest {
    SendMessage { to: String, content: String },
    StartTask { task: String },
    Abort,
    Status,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ControlResponse {
    Ok,
    Error { message: String },
    Status { agents: Vec<AgentStatus>, project: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentStatus {
    pub name: String,
    pub role: String,
}

pub fn send_message(to: &str, content: &str) -> Result<ControlResponse, peercred_ipc::IpcError> {
    Client::call(
        CONTROL_SOCKET,
        &ControlRequest::SendMessage {
            to: to.to_string(),
            content: content.to_string(),
        },
    )
}

pub fn start_task(task: &str) -> Result<ControlResponse, peercred_ipc::IpcError> {
    Client::call(
        CONTROL_SOCKET,
        &ControlRequest::StartTask {
            task: task.to_string(),
        },
    )
}

pub fn get_status() -> Result<ControlResponse, peercred_ipc::IpcError> {
    Client::call(CONTROL_SOCKET, &ControlRequest::Status)
}
