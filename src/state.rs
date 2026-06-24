use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    pub agents: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChatMessage {
    User {
        text: String,
        timestamp: String,
    },
    Assistant {
        text: String,
        timestamp: String,
        usage: Option<TokenUsage>,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_creation: u64,
}

fn data_root() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("agent-orchestrator"))
}

/// Discover projects by scanning `~/.local/share/agent-orchestrator/` for
/// directories that contain a `logs/` subdirectory with JSONL files.
pub fn load_projects() -> Vec<Project> {
    let root = match data_root() {
        Some(r) => r,
        None => return vec![],
    };

    let entries = match std::fs::read_dir(&root) {
        Ok(e) => e,
        Err(_) => return vec![],
    };

    let mut projects = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let logs_dir = path.join("logs");
        let agents = list_agents(&logs_dir);
        if agents.is_empty() {
            continue;
        }
        projects.push(Project { name, path, agents });
    }

    projects.sort_by(|a, b| a.name.cmp(&b.name));
    projects
}

fn list_agents(logs_dir: &Path) -> Vec<String> {
    let entries = match std::fs::read_dir(logs_dir) {
        Ok(e) => e,
        Err(_) => return vec![],
    };

    let mut agents: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) == Some("jsonl") {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            } else {
                None
            }
        })
        .collect();

    agents.sort();
    agents
}

pub fn jsonl_path_for(project: &str, agent: &str) -> Option<PathBuf> {
    data_root().map(|r| r.join(project).join("logs").join(format!("{agent}.jsonl")))
}

fn parse_token_usage(usage: &serde_json::Value) -> Option<TokenUsage> {
    Some(TokenUsage {
        input: usage.get("input")?.as_u64().unwrap_or(0),
        output: usage.get("output")?.as_u64().unwrap_or(0),
        cache_read: usage
            .get("cache_read")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        cache_creation: usage
            .get("cache_creation")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
    })
}

fn parse_line(line: &str) -> Option<ChatMessage> {
    let val: serde_json::Value = serde_json::from_str(line).ok()?;
    let msg_type = val.get("type")?.as_str()?;
    let timestamp = val
        .get("timestamp")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    match msg_type {
        "user" => {
            let text = val.get("text")?.as_str()?.to_string();
            Some(ChatMessage::User { text, timestamp })
        }
        "assistant" => {
            let text = val.get("text")?.as_str()?.to_string();
            let usage = val.get("usage").and_then(parse_token_usage);
            Some(ChatMessage::Assistant {
                text,
                timestamp,
                usage,
            })
        }
        _ => None,
    }
}

pub fn parse_jsonl_from_offset(path: &Path, offset: u64) -> (Vec<ChatMessage>, u64, bool) {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return (vec![], offset, false),
    };

    if file.seek(SeekFrom::Start(offset)).is_err() {
        return (vec![], offset, false);
    }

    let mut reader = BufReader::new(&mut file);
    let mut messages = Vec::new();
    let mut current_offset = offset;
    let mut had_reset = false;

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(n) => current_offset += n as u64,
            Err(_) => break,
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if val.get("type").and_then(|t| t.as_str()) == Some("session_reset") {
                messages.clear();
                had_reset = true;
                continue;
            }
        }

        if let Some(msg) = parse_line(trimmed) {
            messages.push(msg);
        }
    }

    (messages, current_offset, had_reset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn write_log(name: &str, content: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "claude_app_state_{name}_{}.jsonl",
            std::process::id()
        ));
        std::fs::write(&path, content).expect("write test log");
        path
    }

    fn temp_data_home(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("claude_app_data_{name}_{}", std::process::id()))
    }

    #[test]
    fn parses_user_and_assistant_messages_with_usage() {
        let path = write_log(
            "messages",
            r#"{"type":"user","text":"hello","timestamp":"2026-01-01T00:00:00Z"}
{"type":"assistant","text":"world","timestamp":"2026-01-01T00:00:01Z","usage":{"input":10,"output":20,"cache_read":3,"cache_creation":4}}
"#,
        );

        let (messages, offset, had_reset) = parse_jsonl_from_offset(&path, 0);

        assert_eq!(offset, std::fs::metadata(&path).expect("metadata").len());
        assert!(!had_reset);
        assert_eq!(
            messages,
            vec![
                ChatMessage::User {
                    text: "hello".to_string(),
                    timestamp: "2026-01-01T00:00:00Z".to_string(),
                },
                ChatMessage::Assistant {
                    text: "world".to_string(),
                    timestamp: "2026-01-01T00:00:01Z".to_string(),
                    usage: Some(TokenUsage {
                        input: 10,
                        output: 20,
                        cache_read: 3,
                        cache_creation: 4,
                    }),
                },
            ]
        );

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn session_reset_clears_prior_messages() {
        let path = write_log(
            "reset",
            r#"{"type":"user","text":"old","timestamp":"t1"}
{"type":"session_reset"}
{"type":"assistant","text":"new","timestamp":"t2"}
"#,
        );

        let (messages, _, had_reset) = parse_jsonl_from_offset(&path, 0);

        assert!(had_reset);
        assert_eq!(
            messages,
            vec![ChatMessage::Assistant {
                text: "new".to_string(),
                timestamp: "t2".to_string(),
                usage: None,
            }]
        );

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn offset_reads_only_new_lines_and_preserves_offset_on_missing_file() {
        let path = write_log(
            "offset",
            r#"{"type":"user","text":"old","timestamp":"t1"}
{"type":"assistant","text":"new","timestamp":"t2"}
"#,
        );
        let first_line_len = r#"{"type":"user","text":"old","timestamp":"t1"}
"#
        .len() as u64;

        let (messages, offset, had_reset) = parse_jsonl_from_offset(&path, first_line_len);
        let missing = path.with_extension("missing");
        let (missing_messages, missing_offset, missing_reset) =
            parse_jsonl_from_offset(&missing, 42);

        assert!(!had_reset);
        assert_eq!(offset, std::fs::metadata(&path).expect("metadata").len());
        assert_eq!(
            messages,
            vec![ChatMessage::Assistant {
                text: "new".to_string(),
                timestamp: "t2".to_string(),
                usage: None,
            }]
        );
        assert!(missing_messages.is_empty());
        assert_eq!(missing_offset, 42);
        assert!(!missing_reset);

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn load_projects_discovers_sorted_projects_and_agents() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let data_home = temp_data_home("projects");
        let root = data_home.join("agent-orchestrator");
        let alpha_logs = root.join("alpha").join("logs");
        let beta_logs = root.join("beta").join("logs");
        std::fs::create_dir_all(&alpha_logs).expect("create alpha logs");
        std::fs::create_dir_all(&beta_logs).expect("create beta logs");
        std::fs::write(alpha_logs.join("manager.jsonl"), "").expect("write manager log");
        std::fs::write(alpha_logs.join("developer.jsonl"), "").expect("write developer log");
        std::fs::write(alpha_logs.join("ignored.txt"), "").expect("write ignored file");
        std::fs::write(beta_logs.join("auditor.jsonl"), "").expect("write auditor log");

        let old_data_home = std::env::var_os("XDG_DATA_HOME");
        unsafe {
            std::env::set_var("XDG_DATA_HOME", &data_home);
        }

        let projects = load_projects();
        let path = jsonl_path_for("alpha", "manager").expect("jsonl path");

        match old_data_home {
            Some(value) => unsafe {
                std::env::set_var("XDG_DATA_HOME", value);
            },
            None => unsafe {
                std::env::remove_var("XDG_DATA_HOME");
            },
        }
        std::fs::remove_dir_all(data_home).ok();

        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].name, "alpha");
        assert_eq!(projects[0].agents, vec!["developer", "manager"]);
        assert_eq!(projects[1].name, "beta");
        assert_eq!(projects[1].agents, vec!["auditor"]);
        assert!(path.ends_with("agent-orchestrator/alpha/logs/manager.jsonl"));
    }

    #[test]
    fn parser_ignores_invalid_lines_and_defaults_optional_usage_fields() {
        let path = write_log(
            "invalid",
            r#"not json
{"type":"assistant","text":"partial","timestamp":"t1","usage":{"input":7,"output":8}}
{"type":"assistant","timestamp":"missing text"}
{"type":"other","text":"ignored","timestamp":"t2"}

"#,
        );

        let (messages, _, had_reset) = parse_jsonl_from_offset(&path, 0);

        assert!(!had_reset);
        assert_eq!(
            messages,
            vec![ChatMessage::Assistant {
                text: "partial".to_string(),
                timestamp: "t1".to_string(),
                usage: Some(TokenUsage {
                    input: 7,
                    output: 8,
                    cache_read: 0,
                    cache_creation: 0,
                }),
            }]
        );

        std::fs::remove_file(path).ok();
    }
}
