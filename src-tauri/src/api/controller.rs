//! Backend agnostic operations on the [`Engine`].
//!
//! Every transport (HTTP, IPC socket, Tauri commands) goes through this layer,
//! so behaviour stays consistent regardless of caller.

use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::bluetooth::{DeviceInfo, HrSample};
use crate::core::{Engine, EngineSnapshot};

#[derive(Debug, Deserialize)]
pub struct ScanRequest {
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    /// When true, only return devices that advertise the Heart Rate Service UUID.
    /// When false, return every discovered device.
    #[serde(default = "default_filter_hr")]
    pub filter_hr: bool,
}

fn default_timeout() -> u64 {
    5000
}
fn default_filter_hr() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct ConnectRequest {
    pub address: String,
}

#[derive(Debug, Serialize)]
pub struct HistoryResponse {
    pub samples: Vec<HrSample>,
}

pub fn snapshot(engine: &Engine) -> EngineSnapshot {
    engine.snapshot()
}

pub async fn scan(engine: &Arc<Engine>, req: ScanRequest) -> Result<Vec<DeviceInfo>> {
    let mut devices = engine.scan(req.timeout_ms).await?;
    if req.filter_hr {
        devices.retain(|d| d.advertises_hr);
    }
    Ok(devices)
}

pub async fn connect(engine: &Arc<Engine>, req: ConnectRequest) -> Result<()> {
    engine.connect(req.address).await
}

pub async fn disconnect(engine: &Arc<Engine>) -> Result<()> {
    engine.disconnect().await
}

pub fn reset_session(engine: &Engine) {
    engine.reset_session();
}

pub async fn history(engine: &Arc<Engine>, limit: usize) -> HistoryResponse {
    HistoryResponse {
        samples: engine.history().recent(limit).await,
    }
}
