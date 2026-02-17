use std::convert::Infallible;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{
        sse::{Event, Sse},
        Json,
    },
};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::orchestrator::types::{AgentId, AgentRole};
use crate::orchestrator::OrchestratorRuntime;
use crate::state::orchestrator::{OrchestratorRun, RunId, RunStatus};

use super::state::SharedState;
use super::types::{AgentMessageRequest, RunDetail, RunSummary};

pub async fn list_runs(State(state): State<SharedState>) -> Json<Vec<RunSummary>> {
    let runs = state.runs.read().await;
    let summaries = runs
        .values()
        .map(|run| RunSummary {
            id: run.id,
            goal: run.goal.clone(),
            status: run.status.clone(),
        })
        .collect();
    Json(summaries)
}

pub async fn create_run(
    State(state): State<SharedState>,
) -> Result<(StatusCode, Json<RunSummary>), (StatusCode, String)> {
    let handle = OrchestratorRuntime::spawn_run(state.project_path.clone())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to spawn run: {e}"),
            )
        })?;

    let id = RunId::new();
    let run = OrchestratorRun {
        id,
        goal: String::new(),
        agent_sessions: Default::default(),
        status: RunStatus::Running,
        run_handle: Some(handle),
    };

    let summary = RunSummary {
        id: run.id,
        goal: run.goal.clone(),
        status: run.status.clone(),
    };

    state.runs.write().await.insert(id, run);

    Ok((StatusCode::CREATED, Json(summary)))
}

pub async fn get_run(
    State(state): State<SharedState>,
    Path(id): Path<RunId>,
) -> Result<Json<RunDetail>, StatusCode> {
    let runs = state.runs.read().await;
    let run = runs.get(&id).ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(RunDetail {
        id: run.id,
        goal: run.goal.clone(),
        status: run.status.clone(),
        agent_sessions: run.agent_sessions.clone(),
    }))
}

pub async fn abort_run(
    State(state): State<SharedState>,
    Path(id): Path<RunId>,
) -> Result<StatusCode, StatusCode> {
    let mut runs = state.runs.write().await;
    let run = runs.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;

    if let Some(handle) = run.run_handle.take() {
        handle.abort();
    }
    run.status = RunStatus::Failed("aborted".into());

    Ok(StatusCode::OK)
}

pub async fn send_agent_message(
    State(state): State<SharedState>,
    Path((run_id, agent_name)): Path<(RunId, String)>,
    Json(body): Json<AgentMessageRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let agent_id = parse_agent_name(&agent_name)?;

    let runs = state.runs.read().await;
    let run = runs
        .get(&run_id)
        .ok_or((StatusCode::NOT_FOUND, "run not found".into()))?;

    let handle = run
        .run_handle
        .as_ref()
        .ok_or((StatusCode::NOT_FOUND, "run has no active handle".into()))?;

    if handle.send_to_agent(&agent_id, body.text) {
        Ok(StatusCode::OK)
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            "agent not found or send failed".into(),
        ))
    }
}

pub async fn stream_run(
    State(state): State<SharedState>,
    Path(id): Path<RunId>,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    let rx = {
        let runs = state.runs.read().await;
        let run = runs.get(&id).ok_or(StatusCode::NOT_FOUND)?;
        let handle = run.run_handle.as_ref().ok_or(StatusCode::NOT_FOUND)?;
        handle.subscribe()
    };

    let stream = BroadcastStream::new(rx)
        .filter_map(|result: Result<_, _>| result.ok())
        .map(|(agent_id, output)| {
            let data = serde_json::json!({ "agent": agent_id, "output": output });
            Ok::<_, Infallible>(Event::default().data(data.to_string()))
        });

    Ok(Sse::new(stream))
}

pub(crate) fn parse_agent_name(name: &str) -> Result<AgentId, (StatusCode, String)> {
    match name {
        "manager" => Ok(AgentId::new_singleton(AgentRole::Manager)),
        "architect" => Ok(AgentId::new_singleton(AgentRole::Architect)),
        "scorer" => Ok(AgentId::new_singleton(AgentRole::Scorer)),
        other => {
            if let Some(idx) = other.strip_prefix("developer-") {
                let n: u8 = idx.parse().map_err(|_| {
                    (
                        StatusCode::BAD_REQUEST,
                        format!("invalid developer index: {idx}"),
                    )
                })?;
                Ok(AgentId::new_developer(n))
            } else {
                Err((
                    StatusCode::BAD_REQUEST,
                    format!("unknown agent: {name}"),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_singletons() {
        let mgr = parse_agent_name("manager").unwrap();
        assert_eq!(mgr, AgentId::new_singleton(AgentRole::Manager));

        let arch = parse_agent_name("architect").unwrap();
        assert_eq!(arch, AgentId::new_singleton(AgentRole::Architect));

        let scorer = parse_agent_name("scorer").unwrap();
        assert_eq!(scorer, AgentId::new_singleton(AgentRole::Scorer));
    }

    #[test]
    fn parse_developers() {
        let dev0 = parse_agent_name("developer-0").unwrap();
        assert_eq!(dev0, AgentId::new_developer(0));

        let dev2 = parse_agent_name("developer-2").unwrap();
        assert_eq!(dev2, AgentId::new_developer(2));
    }

    #[test]
    fn parse_unknown_errors() {
        assert!(parse_agent_name("unknown").is_err());
        assert!(parse_agent_name("developer-abc").is_err());
    }
}
