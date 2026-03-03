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
        projects.push(Project {
            name,
            path,
            agents,
        });
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
