pub mod auth;
pub mod runs;
pub mod sessions;
pub mod state;
pub mod types;

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    Router,
    middleware,
    routing::{get, post},
};
use tokio::sync::{Mutex, RwLock};
use tower_http::cors::CorsLayer;

use crate::claude::session::SessionManager;
use state::{AppState, SharedState};

pub fn build_router(state: SharedState) -> Router {
    let api_routes = Router::new()
        .route("/sessions", get(sessions::list_sessions).post(sessions::create_session))
        .route("/sessions/{id}", get(sessions::get_session).delete(sessions::delete_session))
        .route("/sessions/{id}/prompt", post(sessions::send_prompt))
        .route("/sessions/{id}/abort", post(sessions::abort_session))
        .route("/runs", get(runs::list_runs).post(runs::create_run))
        .route("/runs/{id}", get(runs::get_run))
        .route("/runs/{id}/abort", post(runs::abort_run))
        .route("/runs/{id}/agents/{agent}/message", post(runs::send_agent_message))
        .route("/runs/{id}/stream", get(runs::stream_run))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth::auth_middleware));

    Router::new()
        .route("/api/auth", post(auth::login))
        .nest("/api", api_routes)
        .layer(CorsLayer::permissive())
        .with_state(state)
}

pub async fn start_server(project_path: std::path::PathBuf) {
    let port: u16 = std::env::var("CLAUDE_APP_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3100);

    let jwt_secret = std::env::var("CLAUDE_APP_SECRET").unwrap_or_else(|_| {
        let secret = uuid::Uuid::new_v4().to_string();
        tracing::info!("Generated API secret: {secret}");
        secret
    });

    let shared_state: SharedState = Arc::new(AppState {
        sessions: RwLock::new(HashMap::new()),
        manager: Mutex::new(SessionManager::new()),
        runs: RwLock::new(HashMap::new()),
        project_path,
        jwt_secret,
    });

    let app = build_router(shared_state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("failed to bind API port");

    tracing::info!("API server listening on port {port}");

    axum::serve(listener, app).await.expect("API server error");
}

#[cfg(test)]
mod tests;
