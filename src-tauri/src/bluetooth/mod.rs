//! Heart rate source abstraction.
//!
//! All sources produce a stream of [`HrSample`] values. The single
//! implementation wraps `tauri-plugin-blec`, which handles desktop (btleplug)
//! and mobile (Android Tauri plugin, iOS CoreBluetooth) in one crate.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

pub mod blec_source;

/// One heart rate measurement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HrSample {
    pub bpm: u16,
    /// RR intervals in milliseconds, when supplied by the device.
    pub rr_intervals_ms: Vec<u16>,
    pub timestamp: DateTime<Utc>,
}

/// A discovered or saved Bluetooth device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub address: String,
    pub name: Option<String>,
    pub rssi: Option<i16>,
    /// True when the device's advertisement includes the GATT Heart Rate Service UUID.
    #[serde(default)]
    pub advertises_hr: bool,
}

/// Connection state of a [`HeartRateSource`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Disconnected,
    Scanning,
    Connecting,
    Connected,
}

/// Abstract heart rate source. Implementations may wrap BLE, simulated input,
/// or platform plugins.
#[async_trait]
pub trait HeartRateSource: Send + Sync {
    /// Scan for nearby BLE heart rate monitors. Returns after `timeout_ms`.
    async fn scan(&self, timeout_ms: u64) -> anyhow::Result<Vec<DeviceInfo>>;

    /// Connect to `address` and stream samples on the returned channel.
    async fn connect(&self, address: &str, tx: mpsc::Sender<HrSample>) -> anyhow::Result<()>;

    /// Disconnect any active session.
    async fn disconnect(&self) -> anyhow::Result<()>;

    /// Current connection state.
    fn state(&self) -> ConnectionState;

    /// Battery level (0–100 %) if the device exposes the GATT Battery Service.
    fn battery_level(&self) -> Option<u8>;
}

/// Construct the default platform source using tauri-plugin-blec.
pub fn default_source() -> std::sync::Arc<dyn HeartRateSource> {
    std::sync::Arc::new(blec_source::BlecSource::new())
}

/// Parse a Bluetooth Heart Rate Measurement characteristic payload.
///
/// Layout (Bluetooth GATT Heart Rate Service, 0x2A37):
/// - byte 0: flags. Bit 0 = HR value format (0 u8, 1 u16). Bit 4 = RR present.
/// - bytes 1..: HR value, optional energy expended, optional RR interval pairs
///   in 1/1024 second units.
pub fn parse_hr_measurement(data: &[u8]) -> Option<HrSample> {
    if data.is_empty() {
        return None;
    }
    let flags = data[0];
    let mut idx = 1usize;

    let bpm: u16 = if flags & 0x01 == 0 {
        let v = *data.get(idx)? as u16;
        idx += 1;
        v
    } else {
        let lo = *data.get(idx)? as u16;
        let hi = *data.get(idx + 1)? as u16;
        idx += 2;
        (hi << 8) | lo
    };

    if flags & 0x08 != 0 {
        idx += 2;
    }

    let mut rr = Vec::new();
    if flags & 0x10 != 0 {
        while idx + 1 < data.len() {
            let lo = data[idx] as u32;
            let hi = data[idx + 1] as u32;
            idx += 2;
            let raw = (hi << 8) | lo;
            rr.push(((raw * 1000) / 1024) as u16);
        }
    }

    Some(HrSample {
        bpm,
        rr_intervals_ms: rr,
        timestamp: Utc::now(),
    })
}

/// UUIDs from the Bluetooth GATT Heart Rate Service specification.
pub mod uuids {
    pub const HEART_RATE_SERVICE: uuid::Uuid = uuid::uuid!("0000180d-0000-1000-8000-00805f9b34fb");
    pub const HEART_RATE_MEASUREMENT: uuid::Uuid =
        uuid::uuid!("00002a37-0000-1000-8000-00805f9b34fb");
    pub const BATTERY_SERVICE: uuid::Uuid = uuid::uuid!("0000180f-0000-1000-8000-00805f9b34fb");
    pub const BATTERY_LEVEL: uuid::Uuid = uuid::uuid!("00002a19-0000-1000-8000-00805f9b34fb");
}
