//! Tauri command surface. Each command delegates to the controller layer so
//! the same logic powers HTTP, IPC and the in app webview.

use std::sync::Arc;

use serde::Serialize;
use tauri::{Emitter, State};
use tokio_stream::StreamExt;

use crate::api::controller;
use crate::bluetooth::DeviceInfo;
use crate::config::Config;
use crate::core::{Engine, EngineSnapshot};

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<Engine>,
    pub config: Arc<parking_lot::RwLock<Config>>,
    /// Absolute path to the config TOML file.
    pub config_file: std::path::PathBuf,
    /// Absolute path to the data directory.
    pub data_dir: std::path::PathBuf,
}

fn map_err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[tauri::command]
pub async fn snapshot(state: State<'_, AppState>) -> Result<EngineSnapshot, String> {
    Ok(controller::snapshot(&state.engine))
}

#[tauri::command]
pub async fn scan(
    timeout_ms: Option<u64>,
    filter_hr: Option<bool>,
    state: State<'_, AppState>,
) -> Result<Vec<DeviceInfo>, String> {
    controller::scan(
        &state.engine,
        controller::ScanRequest {
            timeout_ms: timeout_ms.unwrap_or(5000),
            filter_hr: filter_hr.unwrap_or(true),
        },
    )
    .await
    .map_err(map_err)
}

#[tauri::command]
pub async fn connect(address: String, state: State<'_, AppState>) -> Result<(), String> {
    controller::connect(&state.engine, controller::ConnectRequest { address })
        .await
        .map_err(map_err)
}

#[tauri::command]
pub async fn disconnect(state: State<'_, AppState>) -> Result<(), String> {
    controller::disconnect(&state.engine).await.map_err(map_err)
}

#[tauri::command]
pub async fn reset_session(state: State<'_, AppState>) -> Result<(), String> {
    controller::reset_session(&state.engine);
    Ok(())
}

#[tauri::command]
pub async fn history(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<controller::HistoryResponse, String> {
    Ok(controller::history(&state.engine, limit.unwrap_or(300)).await)
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<Config, String> {
    Ok(state.config.read().clone())
}

#[tauri::command]
pub async fn save_config(config: Config, state: State<'_, AppState>) -> Result<(), String> {
    config.save_to(&state.config_file).map_err(map_err)?;
    *state.config.write() = config;
    Ok(())
}

#[derive(Serialize)]
pub struct ConfigPaths {
    pub config: String,
    pub data: String,
}

#[tauri::command]
pub async fn config_paths(state: State<'_, AppState>) -> Result<ConfigPaths, String> {
    Ok(ConfigPaths {
        config: state.config_file.display().to_string(),
        data: state.data_dir.display().to_string(),
    })
}

/// Save the given address as the auto-connect device and persist config.
#[tauri::command]
pub async fn save_device(address: String, state: State<'_, AppState>) -> Result<(), String> {
    let config_to_save = {
        let mut cfg = state.config.write();
        cfg.bluetooth.device_address = Some(address);
        cfg.bluetooth.auto_reconnect = true;
        cfg.clone()
    };
    config_to_save.save_to(&state.config_file).map_err(map_err)
}

/// Return the most recent log lines from the in-process ring buffer.
#[tauri::command]
pub fn get_logs(limit: Option<usize>) -> Vec<String> {
    crate::logs::recent(limit.unwrap_or(200))
}

// ── Overlay HTML management ───────────────────────────────────────────────────

/// Return the user's custom overlay HTML, or `None` if no custom template
/// has been saved (in which case the embedded default is served).
#[tauri::command]
pub fn get_overlay_html(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let path = state.data_dir.join("overlay.html");
    if path.exists() {
        std::fs::read_to_string(&path).map(Some).map_err(map_err)
    } else {
        Ok(None)
    }
}

/// Persist a custom overlay HTML template.
#[tauri::command]
pub fn save_overlay_html(html: String, state: State<'_, AppState>) -> Result<(), String> {
    let path = state.data_dir.join("overlay.html");
    std::fs::write(&path, html).map_err(map_err)
}

/// Delete the custom overlay HTML, reverting to the embedded default.
#[tauri::command]
pub fn reset_overlay_html(state: State<'_, AppState>) -> Result<(), String> {
    let path = state.data_dir.join("overlay.html");
    if path.exists() {
        std::fs::remove_file(&path).map_err(map_err)
    } else {
        Ok(())
    }
}

/// Return the embedded default overlay HTML.
#[tauri::command]
pub fn get_default_overlay_html() -> String {
    crate::integrations::overlay::DEFAULT_HTML.to_string()
}

/// Forward [`EngineEvent`]s as a Tauri event named `heartkick://event`.
pub fn forward_events(app: &tauri::AppHandle, engine: Arc<Engine>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut stream = Box::pin(crate::api::event_stream(&engine));
        while let Some(evt) = stream.next().await {
            let _ = app.emit("heartkick://event", &evt);
        }
    });
}
