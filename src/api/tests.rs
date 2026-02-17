use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tokio::sync::{Mutex, RwLock};
use tower::ServiceExt;

use crate::api::state::{AppState, SharedState};
use crate::claude::session::SessionManager;
use crate::orchestrator::types::{AgentId, AgentRole};
use crate::orchestrator::RunHandle;
use crate::state::orchestrator::{OrchestratorRun, RunId, RunStatus};
use crate::state::{Message, Session, SessionId, SessionStatus};

use super::build_router;

const TEST_SECRET: &str = "test-secret";

struct TestApp {
    router: Router,
    state: SharedState,
}

impl TestApp {
    fn new() -> Self {
        let state: SharedState = Arc::new(AppState {
            sessions: RwLock::new(HashMap::new()),
            manager: Mutex::new(SessionManager::new()),
            runs: RwLock::new(HashMap::new()),
            project_path: PathBuf::from("/tmp/claude/test-project"),
            jwt_secret: TEST_SECRET.to_string(),
        });
        let router = build_router(state.clone());
        Self { router, state }
    }

    async fn token(&self) -> String {
        let req = Request::builder()
            .uri("/api/auth")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_string(&json!({"secret": TEST_SECRET})).unwrap(),
            ))
            .unwrap();

        let resp = self.router.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body).unwrap();
        json["token"].as_str().unwrap().to_string()
    }

    async fn auth_get(&self, path: &str) -> (StatusCode, Value) {
        let token = self.token().await;
        let req = Request::builder()
            .uri(path)
            .method("GET")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();

        let resp = self.router.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json = serde_json::from_slice(&body).unwrap_or(Value::Null);
        (status, json)
    }

    async fn auth_post(&self, path: &str, body: Value) -> (StatusCode, Value) {
        let token = self.token().await;
        let req = Request::builder()
            .uri(path)
            .method("POST")
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();

        let resp = self.router.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let json = serde_json::from_slice(&body).unwrap_or(Value::Null);
        (status, json)
    }

    async fn insert_session(&self, session: Session) {
        self.state
            .sessions
            .write()
            .await
            .insert(session.id, session);
    }

    async fn insert_run(&self, run: OrchestratorRun) {
        self.state.runs.write().await.insert(run.id, run);
    }
}

fn make_session(messages: Vec<Message>) -> Session {
    Session {
        id: SessionId::new(),
        title: "Test Session".to_string(),
        messages,
        status: SessionStatus::Idle,
        worktree_path: PathBuf::from("/tmp/claude/worktree"),
        project_path: PathBuf::from("/tmp/claude/test-project"),
    }
}

fn make_run(goal: &str, status: RunStatus, handle: Option<RunHandle>) -> OrchestratorRun {
    OrchestratorRun {
        id: RunId::new(),
        goal: goal.to_string(),
        agent_sessions: HashMap::new(),
        status,
        run_handle: handle,
    }
}

// === Auth tests ===

#[tokio::test]
async fn auth_correct_secret() {
    let app = TestApp::new();
    let req = Request::builder()
        .uri("/api/auth")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({"secret": TEST_SECRET})).unwrap(),
        ))
        .unwrap();

    let resp = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert!(json["token"].is_string());
    assert!(!json["token"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn auth_wrong_secret() {
    let app = TestApp::new();
    let req = Request::builder()
        .uri("/api/auth")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&json!({"secret": "wrong-secret"})).unwrap(),
        ))
        .unwrap();

    let resp = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn auth_no_token() {
    let app = TestApp::new();
    let req = Request::builder()
        .uri("/api/sessions")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn auth_invalid_token() {
    let app = TestApp::new();
    let req = Request::builder()
        .uri("/api/sessions")
        .method("GET")
        .header("authorization", "Bearer garbage")
        .body(Body::empty())
        .unwrap();

    let resp = app.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn auth_valid_token() {
    let app = TestApp::new();
    let (status, _) = app.auth_get("/api/sessions").await;
    assert_eq!(status, StatusCode::OK);
}

// === Session tests ===

#[tokio::test]
async fn sessions_list_empty() {
    let app = TestApp::new();
    let (status, json) = app.auth_get("/api/sessions").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json, json!([]));
}

#[tokio::test]
async fn sessions_list_with_data() {
    let app = TestApp::new();
    let session = make_session(vec![
        Message::User {
            text: "hello".to_string(),
        },
        Message::Assistant {
            text: "hi".to_string(),
        },
    ]);
    app.insert_session(session).await;

    let (status, json) = app.auth_get("/api/sessions").await;
    assert_eq!(status, StatusCode::OK);

    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["message_count"], 2);
}

#[tokio::test]
async fn sessions_get_detail() {
    let app = TestApp::new();
    let session = make_session(vec![
        Message::User {
            text: "hello".to_string(),
        },
        Message::Assistant {
            text: "hi".to_string(),
        },
    ]);
    let id = session.id;
    app.insert_session(session).await;

    let (status, json) = app.auth_get(&format!("/api/sessions/{id}")).await;
    assert_eq!(status, StatusCode::OK);

    let messages = json["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["User"]["text"], "hello");
    assert_eq!(messages[1]["Assistant"]["text"], "hi");
}

#[tokio::test]
async fn sessions_get_not_found() {
    let app = TestApp::new();
    let fake_id = uuid::Uuid::new_v4();
    let (status, _) = app.auth_get(&format!("/api/sessions/{fake_id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sessions_abort() {
    let app = TestApp::new();
    let mut session = make_session(vec![]);
    session.status = SessionStatus::Running;
    let id = session.id;
    app.insert_session(session).await;

    let (status, _) = app.auth_post(&format!("/api/sessions/{id}/abort"), json!({})).await;
    assert_eq!(status, StatusCode::OK);

    let sessions = app.state.sessions.read().await;
    let s = sessions.get(&id).unwrap();
    assert_eq!(s.status, SessionStatus::Idle);
}

// === Run tests (without handle) ===

#[tokio::test]
async fn runs_list_empty() {
    let app = TestApp::new();
    let (status, json) = app.auth_get("/api/runs").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json, json!([]));
}

#[tokio::test]
async fn runs_list_with_data() {
    let app = TestApp::new();
    let run = make_run("Fix the bug", RunStatus::Running, None);
    app.insert_run(run).await;

    let (status, json) = app.auth_get("/api/runs").await;
    assert_eq!(status, StatusCode::OK);

    let arr = json.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["goal"], "Fix the bug");
    assert_eq!(arr[0]["status"], "Running");
}

#[tokio::test]
async fn runs_get_detail() {
    let app = TestApp::new();
    let run = make_run("Refactor module", RunStatus::Completed, None);
    let id = run.id;
    app.insert_run(run).await;

    let (status, json) = app.auth_get(&format!("/api/runs/{id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["goal"], "Refactor module");
    assert_eq!(json["status"], "Completed");
}

#[tokio::test]
async fn runs_get_not_found() {
    let app = TestApp::new();
    let fake_id = uuid::Uuid::new_v4();
    let (status, _) = app.auth_get(&format!("/api/runs/{fake_id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn runs_abort_no_handle() {
    let app = TestApp::new();
    let run = make_run("Some task", RunStatus::Running, None);
    let id = run.id;
    app.insert_run(run).await;

    let (status, _) = app.auth_post(&format!("/api/runs/{id}/abort"), json!({})).await;
    assert_eq!(status, StatusCode::OK);

    let runs = app.state.runs.read().await;
    let r = runs.get(&id).unwrap();
    assert_eq!(r.status, RunStatus::Failed("aborted".into()));
}

// === Run tests (with handle) ===

#[tokio::test]
async fn runs_abort_sends_signal() {
    let app = TestApp::new();
    let manager_id = AgentId::new_singleton(AgentRole::Manager);
    let (handle, mut abort_rx, _inbox_rxs) = RunHandle::new_test(vec![manager_id]);
    let run = make_run("Abort me", RunStatus::Running, Some(handle));
    let id = run.id;
    app.insert_run(run).await;

    let (status, _) = app.auth_post(&format!("/api/runs/{id}/abort"), json!({})).await;
    assert_eq!(status, StatusCode::OK);

    // Verify the abort signal was sent
    assert!(abort_rx.try_recv().is_ok());
}

#[tokio::test]
async fn runs_send_agent_message_ok() {
    let app = TestApp::new();
    let manager_id = AgentId::new_singleton(AgentRole::Manager);
    let (handle, _abort_rx, mut inbox_rxs) = RunHandle::new_test(vec![manager_id.clone()]);
    let run = make_run("Message me", RunStatus::Running, Some(handle));
    let id = run.id;
    app.insert_run(run).await;

    let (status, _) = app
        .auth_post(
            &format!("/api/runs/{id}/agents/manager/message"),
            json!({"text": "hello agent"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    // Verify the message was received on the inbox
    let inbox_rx = inbox_rxs.get_mut(&manager_id).unwrap();
    let msg = inbox_rx.try_recv().unwrap();
    assert_eq!(msg.content, "hello agent");
}

#[tokio::test]
async fn runs_send_agent_message_unknown_agent() {
    let app = TestApp::new();
    // Create a handle with only manager, then try to message a developer
    let manager_id = AgentId::new_singleton(AgentRole::Manager);
    let (handle, _abort_rx, _inbox_rxs) = RunHandle::new_test(vec![manager_id]);
    let run = make_run("Wrong agent", RunStatus::Running, Some(handle));
    let id = run.id;
    app.insert_run(run).await;

    let (status, _) = app
        .auth_post(
            &format!("/api/runs/{id}/agents/developer-0/message"),
            json!({"text": "hello"}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
