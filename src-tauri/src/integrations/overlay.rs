//! Overlay HTTP server.
//!
//! Serves a single-page browser-source overlay (`/`) that displays the current
//! BPM with an animated heart, and a JSON endpoint (`/api/bpm`) that the page
//! polls every 500 ms.
//!
//! The HTML is embedded at compile time (`obs_default.html`). If the user has
//! saved a custom template in the data directory that file is served instead,
//! and changes take effect on the next browser refresh without restarting.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use axum::{
    extract::State,
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use serde_json::json;
use tower_http::cors::CorsLayer;

use crate::core::Engine;

/// The default overlay template embedded in the binary.
pub const DEFAULT_HTML: &str = include_str!("../default_overlay.html");

#[derive(Clone)]
struct OverlayState {
    engine: Arc<Engine>,
    /// Path where the user's custom HTML may live. If it exists it is served;
    /// otherwise `DEFAULT_HTML` is used.
    custom_html_path: PathBuf,
}

/// Spawn the overlay server on `bind` (e.g. `"127.0.0.1:9191"`).
pub async fn serve(engine: Arc<Engine>, bind: String, custom_html_path: PathBuf) -> Result<()> {
    let state = OverlayState {
        engine,
        custom_html_path,
    };
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/bpm", get(bpm_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(%bind, "overlay server listening");
    axum::serve(listener, app).await?;
    Ok(())
}

/// Serve the overlay HTML. Reads the custom file on every request so edits are
/// live without restarting the server.
async fn index_handler(State(s): State<OverlayState>) -> impl IntoResponse {
    let html = tokio::fs::read_to_string(&s.custom_html_path)
        .await
        .unwrap_or_else(|_| DEFAULT_HTML.to_string());
    Html(html)
}

/// Return `{"bpm": N}` from the latest engine snapshot.
async fn bpm_handler(State(s): State<OverlayState>) -> impl IntoResponse {
    let snap = s.engine.snapshot();
    let bpm = snap.last_sample.map(|s| s.bpm).unwrap_or(0);
    Json(json!({ "bpm": bpm }))
}
