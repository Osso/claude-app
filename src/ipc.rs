use peercred_ipc::Client;
use serde::{Deserialize, Serialize};

fn control_socket_path() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    format!("{home}/.claude/orchestrator/control.sock")
}

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
        &control_socket_path(),
        &ControlRequest::SendMessage {
            to: to.to_string(),
            content: content.to_string(),
        },
    )
}

pub fn start_task(task: &str) -> Result<ControlResponse, peercred_ipc::IpcError> {
    Client::call(
        &control_socket_path(),
        &ControlRequest::StartTask {
            task: task.to_string(),
        },
    )
}

pub fn get_status() -> Result<ControlResponse, peercred_ipc::IpcError> {
    Client::call(&control_socket_path(), &ControlRequest::Status)
}
