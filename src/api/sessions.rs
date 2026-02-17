use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{
        sse::{Event, Sse},
        IntoResponse, Json,
    },
};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

use super::state::SharedState;
use super::types::{CreateSessionRequest, PromptRequest, SessionDetail, SessionSummary};
use crate::state::SessionId;

pub async fn list_sessions(State(state): State<SharedState>) -> Json<Vec<SessionSummary>> {
    let sessions = state.sessions.read().await;
    let summaries = sessions
        .values()
        .map(|s| SessionSummary {
            id: s.id,
            title: s.title.clone(),
            status: s.status.clone(),
            message_count: s.messages.len(),
        })
        .collect();
    Json(summaries)
}

pub async fn create_session(
    State(state): State<SharedState>,
    Json(body): Json<CreateSessionRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let session = state
        .manager
        .lock()
        .await
        .new_session(&state.project_path, &body.title)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let summary = SessionSummary {
        id: session.id,
        title: session.title.clone(),
        status: session.status.clone(),
        message_count: 0,
    };

    state.sessions.write().await.insert(session.id, session);

    Ok((StatusCode::CREATED, Json(summary)))
}

pub async fn get_session(
    State(state): State<SharedState>,
    Path(id): Path<SessionId>,
) -> Result<Json<SessionDetail>, StatusCode> {
    let sessions = state.sessions.read().await;
    let session = sessions.get(&id).ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(SessionDetail {
        id: session.id,
        title: session.title.clone(),
        status: session.status.clone(),
        messages: session.messages.clone(),
    }))
}

pub async fn delete_session(
    State(state): State<SharedState>,
    Path(id): Path<SessionId>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut manager = state.manager.lock().await;
    let mut sessions = state.sessions.write().await;

    manager
        .remove_session(id, &mut sessions)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn send_prompt(
    State(state): State<SharedState>,
    Path(id): Path<SessionId>,
    Json(body): Json<PromptRequest>,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>>, (StatusCode, String)>
{
    let rx = {
        let mut manager = state.manager.lock().await;
        let mut sessions = state.sessions.write().await;
        manager
            .send_prompt(id, &body.text, &mut sessions)
            .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?
    };

    let stream = ReceiverStream::new(rx).map(|msg| {
        let data = serde_json::to_string(&msg).unwrap_or_default();
        Ok::<_, std::convert::Infallible>(Event::default().data(data))
    });

    Ok(Sse::new(stream))
}

pub async fn abort_session(
    State(state): State<SharedState>,
    Path(id): Path<SessionId>,
) -> Result<StatusCode, StatusCode> {
    state.manager.lock().await.abort_session(id);

    let mut sessions = state.sessions.write().await;
    let session = sessions.get_mut(&id).ok_or(StatusCode::NOT_FOUND)?;
    session.status = crate::state::SessionStatus::Idle;

    Ok(StatusCode::OK)
}
