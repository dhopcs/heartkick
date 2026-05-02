//! HTTP and Server Sent Events transport built on axum.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{sse::Event, IntoResponse, Sse},
    routing::{get, post},
    Json, Router,
};
use futures::stream::Stream;
use serde::Deserialize;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tower_http::cors::CorsLayer;

use crate::api::controller;
use crate::core::Engine;

#[derive(Clone)]
struct AppState {
    engine: Arc<Engine>,
    /// If `Some`, every request must carry `Authorization: Bearer <token>`.
    api_token: Option<String>,
}

/// Axum middleware that enforces Bearer token authentication when one is configured.
async fn auth_middleware(
    State(s): State<AppState>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: Next,
) -> impl IntoResponse {
    match &s.api_token {
        None => next.run(request).await.into_response(),
        Some(expected) => {
            let provided = headers
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "));

            // Constant-time comparison to avoid timing attacks.
            if provided
                .map(|p| constant_time_eq(p.as_bytes(), expected.as_bytes()))
                .unwrap_or(false)
            {
                next.run(request).await.into_response()
            } else {
                (
                    StatusCode::UNAUTHORIZED,
                    [(
                        axum::http::header::WWW_AUTHENTICATE,
                        "Bearer realm=\"heartkick\"",
                    )],
                    "Unauthorized",
                )
                    .into_response()
            }
        }
    }
}

/// Constant-time byte comparison (prevents timing side-channels on the token).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

/// Spawn the HTTP server, binding `bind` (e.g. `127.0.0.1:7878`).
pub async fn serve(engine: Arc<Engine>, bind: String, api_token: Option<String>) -> Result<()> {
    if api_token.is_some() {
        tracing::info!("HTTP API: bearer token authentication enabled");
    }
    let state = AppState { engine, api_token };
    let app = Router::new()
        .route("/v1/snapshot", get(snapshot))
        .route("/v1/scan", post(scan))
        .route("/v1/connect", post(connect))
        .route("/v1/disconnect", post(disconnect))
        .route("/v1/session/reset", post(reset_session))
        .route("/v1/history", get(history))
        .route("/v1/events", get(events))
        .route("/metrics", get(prometheus_metrics))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("binding HTTP API on {bind}"))?;
    tracing::info!(%bind, "HTTP API listening");
    axum::serve(listener, app).await.context("axum serve")?;
    Ok(())
}

async fn snapshot(State(s): State<AppState>) -> Json<crate::core::EngineSnapshot> {
    Json(controller::snapshot(&s.engine))
}

#[derive(Deserialize, Default)]
struct ScanQuery {
    timeout_ms: Option<u64>,
}

async fn scan(State(s): State<AppState>, Json(body): Json<Option<ScanQuery>>) -> impl IntoResponse {
    let timeout_ms = body.and_then(|b| b.timeout_ms).unwrap_or(5000);
    match controller::scan(
        &s.engine,
        controller::ScanRequest {
            timeout_ms,
            filter_hr: true,
        },
    )
    .await
    {
        Ok(devices) => Json(devices).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn connect(
    State(s): State<AppState>,
    Json(body): Json<controller::ConnectRequest>,
) -> impl IntoResponse {
    match controller::connect(&s.engine, body).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

async fn disconnect(State(s): State<AppState>) -> impl IntoResponse {
    let _ = controller::disconnect(&s.engine).await;
    StatusCode::NO_CONTENT
}

async fn reset_session(State(s): State<AppState>) -> impl IntoResponse {
    controller::reset_session(&s.engine);
    StatusCode::NO_CONTENT
}

#[derive(Deserialize)]
struct HistoryQuery {
    limit: Option<usize>,
}

async fn history(
    State(s): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<HistoryQuery>,
) -> Json<controller::HistoryResponse> {
    Json(controller::history(&s.engine, q.limit.unwrap_or(300)).await)
}

async fn events(
    State(s): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = s.engine.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|r| {
        let evt = r.ok()?;
        let json = serde_json::to_string(&evt).ok()?;
        Some(Ok::<_, std::convert::Infallible>(
            Event::default().data(json),
        ))
    });
    Sse::new(stream)
        .keep_alive(axum::response::sse::KeepAlive::new().interval(Duration::from_secs(15)))
}

async fn prometheus_metrics(State(s): State<AppState>) -> impl IntoResponse {
    let cfg = s.engine.config().read().integrations.prometheus.clone();
    if !cfg.enabled {
        return (StatusCode::NOT_FOUND, String::new()).into_response();
    }
    crate::integrations::prometheus::render(&s.engine.snapshot()).into_response()
}
