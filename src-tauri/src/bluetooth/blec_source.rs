//! Heart rate source:
//! - **Desktop**: btleplug for scan AND connect/subscribe (single adapter instance).
//! - **Mobile**: tauri-plugin-blec for scan AND connect/subscribe.
//!
//! On desktop we cannot mix btleplug scan with blec connect because they maintain
//! separate adapter instances with separate peripheral caches.

use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use parking_lot::Mutex;
use tokio::sync::mpsc;

use super::{parse_hr_measurement, uuids, ConnectionState, DeviceInfo, HeartRateSource, HrSample};

pub struct BlecSource {
    state: Arc<Mutex<ConnectionState>>,
    battery: Arc<Mutex<Option<u8>>>,
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    peripheral: Arc<tokio::sync::Mutex<Option<btleplug::platform::Peripheral>>>,
}

impl BlecSource {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ConnectionState::Disconnected)),
            battery: Arc::new(Mutex::new(None)),
            #[cfg(not(any(target_os = "ios", target_os = "android")))]
            peripheral: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }
}

impl Default for BlecSource {
    fn default() -> Self {
        Self::new()
    }
}

// ── Desktop (btleplug) ───────────────────────────────────────────────────────

#[cfg(not(any(target_os = "ios", target_os = "android")))]
mod desktop {
    use super::*;
    use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
    use btleplug::platform::Manager;
    use futures::StreamExt;
    use tokio::time::{sleep, timeout, Duration};

    async fn get_adapter() -> Result<btleplug::platform::Adapter> {
        let manager = Manager::new().await.context("btleplug manager")?;
        manager
            .adapters()
            .await
            .context("btleplug adapters")?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("no BLE adapter found"))
    }

    pub async fn scan(timeout_ms: u64) -> Result<Vec<DeviceInfo>> {
        let adapter = get_adapter().await?;
        adapter
            .start_scan(ScanFilter::default())
            .await
            .context("start_scan")?;
        sleep(Duration::from_millis(timeout_ms)).await;
        adapter.stop_scan().await.context("stop_scan")?;

        let mut devices = Vec::new();
        for p in adapter.peripherals().await.context("peripherals")? {
            if let Some(props) = p.properties().await.ok().flatten() {
                let advertises_hr = props.services.contains(&uuids::HEART_RATE_SERVICE);
                devices.push(DeviceInfo {
                    address: props.address.to_string(),
                    name: props.local_name,
                    rssi: props.rssi,
                    advertises_hr,
                });
            }
        }
        Ok(devices)
    }

    pub async fn connect(
        address_str: &str,
        tx: mpsc::Sender<HrSample>,
        state: Arc<Mutex<ConnectionState>>,
        peripheral_slot: Arc<tokio::sync::Mutex<Option<btleplug::platform::Peripheral>>>,
        battery_slot: Arc<Mutex<Option<u8>>>,
    ) -> Result<()> {
        let adapter = get_adapter().await?;

        // Scan briefly to populate the adapter's peripheral cache.
        adapter
            .start_scan(ScanFilter::default())
            .await
            .context("start_scan")?;

        let target: btleplug::api::BDAddr = address_str
            .parse()
            .map_err(|_| anyhow!("invalid BLE address: {address_str}"))?;

        // Poll up to 8 s for the device to appear.
        let found = timeout(Duration::from_secs(8), async {
            loop {
                for p in adapter.peripherals().await.unwrap_or_default() {
                    if let Some(props) = p.properties().await.ok().flatten() {
                        if props.address == target {
                            return p;
                        }
                    }
                }
                sleep(Duration::from_millis(300)).await;
            }
        })
        .await
        .map_err(|_| anyhow!("device {address_str} not visible after 8 s scan"))?;

        adapter.stop_scan().await.ok();

        found.connect().await.context("btleplug connect")?;
        found
            .discover_services()
            .await
            .context("discover_services")?;

        let hr_char = found
            .characteristics()
            .into_iter()
            .find(|c| c.uuid == uuids::HEART_RATE_MEASUREMENT)
            .ok_or_else(|| anyhow!("HR measurement characteristic not found on device"))?;

        found.subscribe(&hr_char).await.context("subscribe")?;
        *state.lock() = ConnectionState::Connected;

        // Read battery level if the Battery Service is present.
        if let Some(bat_char) = found
            .characteristics()
            .into_iter()
            .find(|c| c.uuid == uuids::BATTERY_LEVEL)
        {
            if let Ok(data) = found.read(&bat_char).await {
                *battery_slot.lock() = data.first().copied();
            }
        }

        // Store peripheral so disconnect() can reach it.
        *peripheral_slot.lock().await = Some(found.clone());

        // Pump notifications in a background task.
        let mut stream = found
            .notifications()
            .await
            .context("notifications stream")?;
        let state2 = state.clone();
        let slot2 = peripheral_slot.clone();
        tokio::spawn(async move {
            while let Some(n) = stream.next().await {
                if n.uuid == uuids::HEART_RATE_MEASUREMENT {
                    if let Some(sample) = parse_hr_measurement(&n.value) {
                        if tx.send(sample).await.is_err() {
                            break;
                        }
                    }
                }
            }
            *state2.lock() = ConnectionState::Disconnected;
            *slot2.lock().await = None;
        });

        Ok(())
    }

    pub async fn disconnect(
        peripheral_slot: Arc<tokio::sync::Mutex<Option<btleplug::platform::Peripheral>>>,
    ) -> Result<()> {
        use btleplug::api::Peripheral as _;
        if let Some(p) = peripheral_slot.lock().await.take() {
            p.disconnect().await.context("btleplug disconnect")?;
        }
        Ok(())
    }
}

#[cfg(any(target_os = "ios", target_os = "android"))]
mod mobile {
    use super::*;
    use tauri_plugin_blec::{
        get_handler,
        models::{BleDevice, ScanFilter},
    };

    pub async fn scan(timeout_ms: u64) -> Result<Vec<DeviceInfo>> {
        let handler = get_handler().context("blec handler not initialised")?;
        let (tx, mut rx) = mpsc::channel::<Vec<BleDevice>>(16);
        handler
            .discover(Some(tx), timeout_ms, ScanFilter::None, false)
            .await
            .context("blec discover")?;

        let mut seen: std::collections::HashMap<String, DeviceInfo> = Default::default();
        while let Some(batch) = rx.recv().await {
            for d in &batch {
                seen.entry(d.address.clone()).or_insert_with(|| DeviceInfo {
                    address: d.address.clone(),
                    name: Some(d.name.clone()),
                    rssi: d.rssi.map(|r| r as i16),
                    advertises_hr: false, // blec doesn't expose advertised services
                });
            }
        }
        Ok(seen.into_values().collect())
    }

    pub async fn connect(
        address: &str,
        tx: mpsc::Sender<HrSample>,
        state: Arc<Mutex<ConnectionState>>,
        battery_slot: Arc<Mutex<Option<u8>>>,
    ) -> Result<()> {
        let handler = get_handler().context("blec handler not initialised")?;
        let state2 = state.clone();
        handler
            .connect(
                address,
                (move || {
                    *state2.lock() = ConnectionState::Disconnected;
                })
                .into(),
                false,
            )
            .await
            .context("blec connect")?;
        *state.lock() = ConnectionState::Connected;

        // Read battery level if the Battery Service is present.
        if let Ok(data) = handler.recv_data(uuids::BATTERY_LEVEL, None).await {
            *battery_slot.lock() = data.first().copied();
        }

        handler
            .subscribe(uuids::HEART_RATE_MEASUREMENT, None, move |data: Vec<u8>| {
                if let Some(s) = parse_hr_measurement(&data) {
                    let _ = tx.try_send(s);
                }
            })
            .await
            .context("blec subscribe")?;
        Ok(())
    }

    pub async fn disconnect() -> Result<()> {
        if let Ok(h) = tauri_plugin_blec::get_handler() {
            let _ = h.disconnect().await;
        }
        Ok(())
    }
}

#[async_trait]
impl HeartRateSource for BlecSource {
    async fn scan(&self, timeout_ms: u64) -> Result<Vec<DeviceInfo>> {
        *self.state.lock() = ConnectionState::Scanning;
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        let result = desktop::scan(timeout_ms).await;
        #[cfg(any(target_os = "ios", target_os = "android"))]
        let result = mobile::scan(timeout_ms).await;
        *self.state.lock() = ConnectionState::Disconnected;
        result
    }

    async fn connect(&self, address: &str, tx: mpsc::Sender<HrSample>) -> Result<()> {
        *self.state.lock() = ConnectionState::Connecting;
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        let result = desktop::connect(
            address,
            tx,
            self.state.clone(),
            self.peripheral.clone(),
            self.battery.clone(),
        )
        .await;
        #[cfg(any(target_os = "ios", target_os = "android"))]
        let result = mobile::connect(address, tx, self.state.clone(), self.battery.clone()).await;
        if result.is_err() {
            *self.state.lock() = ConnectionState::Disconnected;
        }
        result
    }

    async fn disconnect(&self) -> Result<()> {
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        let result = desktop::disconnect(self.peripheral.clone()).await;
        #[cfg(any(target_os = "ios", target_os = "android"))]
        let result = mobile::disconnect().await;
        *self.state.lock() = ConnectionState::Disconnected;
        if result.is_ok() {
            *self.battery.lock() = None;
        }
        result
    }

    fn state(&self) -> ConnectionState {
        *self.state.lock()
    }

    fn battery_level(&self) -> Option<u8> {
        *self.battery.lock()
    }
}
