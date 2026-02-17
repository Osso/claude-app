use serde::{Deserialize, Serialize};
use serde_json::Value;

// --- Input types (sent to Claude CLI stdin) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClaudeInput {
    User { message: UserMessage },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    pub role: String,
    pub content: String,
}

impl ClaudeInput {
    pub fn user(content: impl Into<String>) -> Self {
        Self::User {
            message: UserMessage {
                role: "user".to_string(),
                content: content.into(),
            },
        }
    }
}

// --- Output types (read from Claude CLI stdout) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClaudeOutput {
    System(SystemMessage),
    Assistant(AssistantMessage),
    ToolUse(ToolUseMessage),
    ToolResult(ToolResultMessage),
    Result(ResultMessage),
    Error(ErrorMessage),
    #[serde(other)]
    Unknown,
}

impl ClaudeOutput {
    pub fn is_final(&self) -> bool {
        matches!(self, Self::Result(_) | Self::Error(_))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMessage {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(flatten)]
    pub _extra: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub message: AssistantContent,
    #[serde(flatten)]
    pub _extra: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantContent {
    #[serde(default)]
    pub content: Vec<ContentBlock>,
    #[serde(flatten)]
    pub _extra: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: Value },
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseMessage {
    pub tool_use_id: String,
    pub tool_name: String,
    #[serde(default)]
    pub input: Value,
    #[serde(flatten)]
    pub _extra: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultMessage {
    pub tool_use_id: String,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub is_error: Option<bool>,
    #[serde(flatten)]
    pub _extra: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultMessage {
    #[serde(default)]
    pub is_error: bool,
    #[serde(default)]
    pub result: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(flatten)]
    pub _extra: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMessage {
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(flatten)]
    pub _extra: Value,
}
