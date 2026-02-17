use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentRole {
    Manager,
    Architect,
    Developer,
    Scorer,
}

impl AgentRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentRole::Manager => "manager",
            AgentRole::Architect => "architect",
            AgentRole::Developer => "developer",
            AgentRole::Scorer => "scorer",
        }
    }
}

impl std::fmt::Display for AgentRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Unique identifier for an agent instance.
/// Singletons (manager, architect, scorer) use index 0.
/// Developers use index 0-2 for multi-developer support.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentId {
    pub role: AgentRole,
    pub index: u8,
}

impl AgentId {
    pub fn new_singleton(role: AgentRole) -> Self {
        Self { role, index: 0 }
    }

    pub fn new_developer(index: u8) -> Self {
        Self {
            role: AgentRole::Developer,
            index,
        }
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.role == AgentRole::Developer {
            write!(f, "developer-{}", self.index)
        } else {
            write!(f, "{}", self.role.as_str())
        }
    }
}

/// Types of messages between agents
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    TaskAssignment,
    TaskComplete,
    TaskGiveUp,
    Interrupt,
    ArchitectReview,
    Info,
    Evaluation,
    Observation,
}

/// Message sent between agents via in-process channels
#[derive(Debug, Clone)]
pub struct AgentMessage {
    pub id: Uuid,
    pub from: AgentId,
    pub to: AgentId,
    pub kind: MessageKind,
    pub content: String,
}

impl AgentMessage {
    pub fn new(from: AgentId, to: AgentId, kind: MessageKind, content: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            from,
            to,
            kind,
            content,
        }
    }
}

/// Commands sent from agents to the runtime
#[derive(Debug)]
pub enum RuntimeCommand {
    /// Manager requests N developers (1-3)
    SetCrewSize { count: u8 },
    /// Scorer fires the manager
    RelieveManager { reason: String },
}
