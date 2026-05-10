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

use actix_cors::Cors;
use actix_web::{web, App, HttpResponse, HttpServer};
use anyhow::{Context as _, Result};
use serde_json::json;

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
    let state = web::Data::new(OverlayState {
        engine,
        custom_html_path,
    });

    // HttpServer::run() is !Send; run on a dedicated thread.
    tokio::task::spawn_blocking(move || {
        actix_web::rt::System::new().block_on(async move {
            let server = HttpServer::new(move || {
                App::new()
                    .app_data(state.clone())
                    .wrap(Cors::permissive())
                    .route("/", web::get().to(index_handler))
                    .route("/api/bpm", web::get().to(bpm_handler))
            })
            .workers(1)
            .bind(&bind)?;
            tracing::info!(%bind, "overlay server listening");
            server.run().await
        })
    })
    .await
    .context("overlay server thread panicked")?
    .context("overlay server")
}

/// Serve the overlay HTML. Reads the custom file on every request so edits are
/// live without restarting the server.
async fn index_handler(state: web::Data<OverlayState>) -> HttpResponse {
    let html = tokio::fs::read_to_string(&state.custom_html_path)
        .await
        .unwrap_or_else(|_| DEFAULT_HTML.to_string());
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

/// Return `{"bpm": N}` from the latest engine snapshot.
async fn bpm_handler(state: web::Data<OverlayState>) -> web::Json<serde_json::Value> {
    let snap = state.engine.snapshot();
    let bpm = snap.last_sample.map(|s| s.bpm).unwrap_or(0);
    web::Json(json!({ "bpm": bpm }))
}
