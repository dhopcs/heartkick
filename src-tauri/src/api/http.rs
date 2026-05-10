use std::sync::Arc;
use std::time::Duration;

use actix_cors::Cors;
use actix_web::{
    body::BoxBody,
    dev::{ServiceRequest, ServiceResponse},
    middleware::{from_fn, Next},
    web::{self, Bytes},
    App, HttpResponse, HttpServer,
};
use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use serde::Deserialize;
use tokio::time::interval;
use tokio_stream::wrappers::{BroadcastStream, IntervalStream};

use crate::api::controller;
use crate::core::Engine;

#[derive(Clone)]
struct AppState {
    engine: Arc<Engine>,
    /// If `Some`, every request must carry `Authorization: Bearer <token>`.
    api_token: Option<String>,
}

/// Middleware that enforces Bearer token authentication when one is configured.
async fn auth_middleware(
    req: ServiceRequest,
    next: Next<impl actix_web::body::MessageBody + 'static>,
) -> Result<ServiceResponse<BoxBody>, actix_web::Error> {
    let expected = req
        .app_data::<web::Data<AppState>>()
        .and_then(|s| s.api_token.clone());

    match expected {
        None => Ok(next.call(req).await?.map_into_boxed_body()),
        Some(expected) => {
            let ok = req
                .headers()
                .get(actix_web::http::header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
                // Constant-time comparison to avoid timing attacks.
                .map(|p| constant_time_eq(p.as_bytes(), expected.as_bytes()))
                .unwrap_or(false);

            if ok {
                Ok(next.call(req).await?.map_into_boxed_body())
            } else {
                let (req, _) = req.into_parts();
                Ok(ServiceResponse::new(
                    req,
                    HttpResponse::Unauthorized()
                        .insert_header((
                            actix_web::http::header::WWW_AUTHENTICATE,
                            "Bearer realm=\"heartkick\"",
                        ))
                        .body("Unauthorized")
                        .map_into_boxed_body(),
                ))
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
    let state = web::Data::new(AppState { engine, api_token });

    // HttpServer::run() is !Send (actix-web uses Rc internally), so it must
    // live on a dedicated OS thread with its own actix-rt System.
    tokio::task::spawn_blocking(move || {
        actix_web::rt::System::new().block_on(async move {
            let server = HttpServer::new(move || {
                App::new()
                    .app_data(state.clone())
                    // CORS is outermost (last .wrap = outermost), auth inside it.
                    .wrap(from_fn(auth_middleware))
                    .wrap(Cors::permissive())
                    .route("/v1/snapshot", web::get().to(snapshot))
                    .route("/v1/scan", web::post().to(scan))
                    .route("/v1/connect", web::post().to(connect))
                    .route("/v1/disconnect", web::post().to(disconnect))
                    .route("/v1/session/reset", web::post().to(reset_session))
                    .route("/v1/history", web::get().to(history))
                    .route("/v1/events", web::get().to(events))
                    .route("/metrics", web::get().to(prometheus_metrics))
            })
            .bind(&bind)
            .with_context(|| format!("binding HTTP API on {bind}"))?;
            tracing::info!(%bind, "HTTP API listening");
            server.run().await.context("actix-web serve")
        })
    })
    .await
    .context("HTTP server thread panicked")?
}

async fn snapshot(data: web::Data<AppState>) -> web::Json<crate::core::EngineSnapshot> {
    web::Json(controller::snapshot(&data.engine))
}

#[derive(Deserialize, Default)]
struct ScanQuery {
    timeout_ms: Option<u64>,
}

async fn scan(data: web::Data<AppState>, body: Option<web::Json<ScanQuery>>) -> HttpResponse {
    let timeout_ms = body.as_ref().and_then(|b| b.timeout_ms).unwrap_or(5000);
    match controller::scan(
        &data.engine,
        controller::ScanRequest {
            timeout_ms,
            filter_hr: true,
        },
    )
    .await
    {
        Ok(devices) => HttpResponse::Ok().json(devices),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

async fn connect(
    data: web::Data<AppState>,
    body: web::Json<controller::ConnectRequest>,
) -> HttpResponse {
    match controller::connect(&data.engine, body.into_inner()).await {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(e) => HttpResponse::BadRequest().body(e.to_string()),
    }
}

async fn disconnect(data: web::Data<AppState>) -> HttpResponse {
    let _ = controller::disconnect(&data.engine).await;
    HttpResponse::NoContent().finish()
}

async fn reset_session(data: web::Data<AppState>) -> HttpResponse {
    controller::reset_session(&data.engine);
    HttpResponse::NoContent().finish()
}

#[derive(Deserialize)]
struct HistoryQuery {
    limit: Option<usize>,
}

async fn history(
    data: web::Data<AppState>,
    query: web::Query<HistoryQuery>,
) -> web::Json<controller::HistoryResponse> {
    web::Json(controller::history(&data.engine, query.limit.unwrap_or(300)).await)
}

async fn events(data: web::Data<AppState>) -> HttpResponse {
    let rx = data.engine.subscribe();

    let event_stream = BroadcastStream::new(rx)
        .filter_map(|r| async move {
            let evt = r.ok()?;
            let json = serde_json::to_string(&evt).ok()?;
            Some(Ok::<Bytes, actix_web::Error>(Bytes::from(format!(
                "data: {json}\n\n"
            ))))
        })
        .boxed();

    // Interleave keepalive comments so proxies and browsers don't time out.
    let keepalive = IntervalStream::new(interval(Duration::from_secs(15)))
        .map(|_| Ok::<Bytes, actix_web::Error>(Bytes::from_static(b": keepalive\n\n")))
        .boxed();

    HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(stream::select(event_stream, keepalive))
}

async fn prometheus_metrics(data: web::Data<AppState>) -> HttpResponse {
    let cfg = data.engine.config().read().integrations.prometheus.clone();
    if !cfg.enabled {
        return HttpResponse::NotFound().finish();
    }
    let body = crate::integrations::prometheus::render(&data.engine.snapshot());
    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4; charset=utf-8")
        .body(body)
}
