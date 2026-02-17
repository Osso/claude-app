use super::types::{AgentId, AgentMessage, AgentRole, MessageKind, RuntimeCommand};

/// Result of parsing an agent output section
pub enum ParsedOutput {
    Message(AgentMessage),
    Command(RuntimeCommand),
}

/// Message routing table: (prefix, target_role, kind, require_from_role)
const ROUTES: &[(&str, AgentRole, MessageKind, Option<AgentRole>)] = &[
    ("TASK:", AgentRole::Architect, MessageKind::TaskAssignment, None),
    ("REJECTED:", AgentRole::Manager, MessageKind::ArchitectReview, None),
    ("INTERRUPT:", AgentRole::Developer, MessageKind::Interrupt, Some(AgentRole::Architect)),
];

/// Route parsed sections from an agent into messages and runtime commands.
pub fn route_sections(from: &AgentId, sections: Vec<(&str, String)>) -> Vec<ParsedOutput> {
    sections
        .into_iter()
        .filter_map(|(prefix, content)| route_section(from, prefix, &content))
        .collect()
}

fn route_section(from: &AgentId, prefix: &str, content: &str) -> Option<ParsedOutput> {
    match prefix {
        "CREW:" => {
            if from.role != AgentRole::Manager {
                return None;
            }
            let count: u8 = content.trim().parse().ok()?;
            Some(ParsedOutput::Command(RuntimeCommand::SetCrewSize { count }))
        }
        "RELIEVE:" => {
            if from.role != AgentRole::Scorer {
                return None;
            }
            Some(ParsedOutput::Command(RuntimeCommand::RelieveManager {
                reason: content.to_string(),
            }))
        }
        "APPROVED:" => {
            let target = parse_developer_target(content);
            Some(ParsedOutput::Message(AgentMessage::new(
                from.clone(),
                target,
                MessageKind::TaskAssignment,
                content.to_string(),
            )))
        }
        "COMPLETE:" => Some(ParsedOutput::Message(AgentMessage::new(
            from.clone(),
            AgentId::new_singleton(AgentRole::Manager),
            MessageKind::TaskComplete,
            content.to_string(),
        ))),
        "BLOCKED:" => Some(ParsedOutput::Message(AgentMessage::new(
            from.clone(),
            AgentId::new_singleton(AgentRole::Manager),
            MessageKind::TaskGiveUp,
            content.to_string(),
        ))),
        "EVALUATION:" | "OBSERVATION:" => {
            tracing::info!("[SCORER {}] {}", prefix.trim_end_matches(':'), first_line(content));
            None
        }
        _ => route_via_table(from, prefix, content),
    }
}

/// Route a section via the static routing table (TASK:, REJECTED:, INTERRUPT:)
fn route_via_table(from: &AgentId, prefix: &str, content: &str) -> Option<ParsedOutput> {
    for &(route_prefix, target_role, kind, require_from) in ROUTES {
        if prefix != route_prefix {
            continue;
        }
        if let Some(required) = require_from {
            if from.role != required {
                continue;
            }
        }
        return Some(ParsedOutput::Message(AgentMessage::new(
            from.clone(),
            AgentId::new_singleton(target_role),
            kind,
            content.to_string(),
        )));
    }
    None
}

/// Extract `developer-N` from text prefix, default to developer-0
fn parse_developer_target(text: &str) -> AgentId {
    if let Some(rest) = text.strip_prefix("developer-") {
        if let Some(digit) = rest.chars().next() {
            if let Some(idx) = digit.to_digit(10) {
                return AgentId::new_developer(idx as u8);
            }
        }
    }
    AgentId::new_developer(0)
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crew_from_manager() {
        let from = AgentId::new_singleton(AgentRole::Manager);
        let result = route_sections(&from, vec![("CREW:", "2".to_string())]);
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0], ParsedOutput::Command(RuntimeCommand::SetCrewSize { count: 2 })));
    }

    #[test]
    fn crew_from_non_manager_ignored() {
        let from = AgentId::new_singleton(AgentRole::Architect);
        let result = route_sections(&from, vec![("CREW:", "2".to_string())]);
        assert!(result.is_empty());
    }

    #[test]
    fn approved_routes_to_developer() {
        let from = AgentId::new_singleton(AgentRole::Architect);
        let result = route_sections(&from, vec![("APPROVED:", "developer-1 looks good".to_string())]);
        assert_eq!(result.len(), 1);
        if let ParsedOutput::Message(msg) = &result[0] {
            assert_eq!(msg.to, AgentId::new_developer(1));
            assert_eq!(msg.kind, MessageKind::TaskAssignment);
        } else {
            panic!("expected message");
        }
    }

    #[test]
    fn approved_defaults_to_dev_0() {
        let from = AgentId::new_singleton(AgentRole::Architect);
        let result = route_sections(&from, vec![("APPROVED:", "looks good".to_string())]);
        assert_eq!(result.len(), 1);
        if let ParsedOutput::Message(msg) = &result[0] {
            assert_eq!(msg.to, AgentId::new_developer(0));
        } else {
            panic!("expected message");
        }
    }

    #[test]
    fn complete_routes_to_manager() {
        let from = AgentId::new_developer(0);
        let result = route_sections(&from, vec![("COMPLETE:", "done".to_string())]);
        assert_eq!(result.len(), 1);
        if let ParsedOutput::Message(msg) = &result[0] {
            assert_eq!(msg.to, AgentId::new_singleton(AgentRole::Manager));
            assert_eq!(msg.kind, MessageKind::TaskComplete);
        } else {
            panic!("expected message");
        }
    }

    #[test]
    fn task_routes_to_architect() {
        let from = AgentId::new_singleton(AgentRole::Manager);
        let result = route_sections(&from, vec![("TASK:", "implement feature".to_string())]);
        assert_eq!(result.len(), 1);
        if let ParsedOutput::Message(msg) = &result[0] {
            assert_eq!(msg.to, AgentId::new_singleton(AgentRole::Architect));
            assert_eq!(msg.kind, MessageKind::TaskAssignment);
        } else {
            panic!("expected message");
        }
    }
}
