use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use parking_lot::Mutex;
use tokio::sync::{mpsc, OnceCell};

use super::{parse_hr_measurement, uuids, ConnectionState, DeviceInfo, HeartRateSource, HrSample};

pub struct BlecSource {
    state: Arc<Mutex<ConnectionState>>,
    battery: Arc<Mutex<Option<u8>>>,
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    adapter: Arc<OnceCell<btleplug::platform::Adapter>>,
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    peripheral: Arc<tokio::sync::Mutex<Option<btleplug::platform::Peripheral>>>,
}

impl BlecSource {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ConnectionState::Disconnected)),
            battery: Arc::new(Mutex::new(None)),
            #[cfg(not(any(target_os = "ios", target_os = "android")))]
            adapter: Arc::new(OnceCell::new()),
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

#[cfg(not(any(target_os = "ios", target_os = "android")))]
mod desktop {
    use super::*;
    use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter};
    use btleplug::platform::{Adapter, Manager, Peripheral};
    use futures::StreamExt;
    use std::collections::HashMap;
    use tokio::time::{timeout, timeout_at, Duration, Instant};

    /// Returns the cached adapter, initialising it on first call.
    async fn get_adapter(cache: &OnceCell<Adapter>) -> Result<Adapter> {
        cache
            .get_or_try_init(|| async {
                let manager = Manager::new().await.context("BLE manager init")?;
                manager
                    .adapters()
                    .await
                    .context("listing BLE adapters")?
                    .into_iter()
                    .next()
                    .ok_or_else(|| anyhow!("no Bluetooth adapter found"))
            })
            .await
            .cloned()
    }

    /// Scan for `timeout_ms`, collecting and deduplicating devices via the
    /// central event stream (updates RSSI/name as fresher advertisements arrive).
    /// Each newly-seen device is sent on `progress` immediately so callers can
    /// display results as they appear.
    pub async fn scan(
        cache: &OnceCell<Adapter>,
        timeout_ms: u64,
        progress: mpsc::Sender<DeviceInfo>,
    ) -> Result<Vec<DeviceInfo>> {
        let adapter = get_adapter(cache).await?;
        let mut events = adapter.events().await.context("BLE event stream")?;
        adapter
            .start_scan(ScanFilter::default())
            .await
            .context("start_scan")?;

        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        let mut devices: HashMap<String, DeviceInfo> = HashMap::new();

        loop {
            match timeout_at(deadline, events.next()).await {
                Ok(Some(
                    CentralEvent::DeviceDiscovered(id) | CentralEvent::DeviceUpdated(id),
                )) => {
                    if let Ok(p) = adapter.peripheral(&id).await {
                        if let Some(props) = p.properties().await.ok().flatten() {
                            let advertises_hr =
                                props.services.contains(&uuids::HEART_RATE_SERVICE);
                            let key = props.address.to_string();
                            let is_new = !devices.contains_key(&key);
                            let entry = devices
                                .entry(key)
                                .and_modify(|d| {
                                    if props.rssi.is_some() {
                                        d.rssi = props.rssi;
                                    }
                                    if props.local_name.is_some() {
                                        d.name.clone_from(&props.local_name);
                                    }
                                    d.advertises_hr |= advertises_hr;
                                })
                                .or_insert_with(|| DeviceInfo {
                                    address: props.address.to_string(),
                                    name: props.local_name,
                                    rssi: props.rssi,
                                    advertises_hr,
                                });
                            // Emit on the progress channel only for the first
                            // sighting — updates are captured in the final Vec.
                            if is_new {
                                progress.send(entry.clone()).await.ok();
                            }
                        }
                    }
                }
                Ok(Some(_)) => {} // other events (state updates etc.), keep scanning
                Ok(None) | Err(_) => break, // stream closed or timeout elapsed
            }
        }

        adapter.stop_scan().await.ok();
        Ok(devices.into_values().collect())
    }

    /// Connect to `address_str`, subscribing to HR notifications on success.
    /// Device discovery is event-driven — no polling loop.
    pub async fn connect(
        cache: &OnceCell<Adapter>,
        address_str: &str,
        tx: mpsc::Sender<HrSample>,
        state: Arc<Mutex<ConnectionState>>,
        peripheral_slot: Arc<tokio::sync::Mutex<Option<Peripheral>>>,
        battery_slot: Arc<Mutex<Option<u8>>>,
    ) -> Result<()> {
        let adapter = get_adapter(cache).await?;
        let mut events = adapter.events().await.context("BLE event stream")?;

        let target: btleplug::api::BDAddr = address_str
            .parse()
            .map_err(|_| anyhow!("invalid BLE address: {address_str}"))?;

        // Stop any in-progress scan (e.g. user clicked Connect during a scan).
        adapter.stop_scan().await.ok();
        adapter
            .start_scan(ScanFilter::default())
            .await
            .context("start_scan")?;

        // Wait up to 10 s for the target device to appear in the event stream.
        // Only DeviceDiscovered is checked — DeviceUpdated is a re-advertisement
        // of a device we may already know about and incurs an extra properties()
        // round-trip that slows discovery.
        let found_result: Result<Peripheral> = timeout(Duration::from_secs(10), async {
            loop {
                match events.next().await {
                    Some(CentralEvent::DeviceDiscovered(id)) => {
                        if let Ok(p) = adapter.peripheral(&id).await {
                            if let Some(props) = p.properties().await.ok().flatten() {
                                if props.address == target {
                                    return Ok(p);
                                }
                            }
                        }
                    }
                    None => return Err(anyhow!("BLE event stream closed")),
                    _ => {}
                }
            }
        })
        .await
        .map_err(|_| anyhow!("device {address_str} not found within 10 s"))
        .and_then(|r| r.context("device discovery"));

        // Always stop scanning regardless of whether discovery succeeded.
        adapter.stop_scan().await.ok();

        let found = found_result?;

        found.connect().await.context("connect")?;
        found
            .discover_services()
            .await
            .context("discover_services")?;

        let hr_char = found
            .characteristics()
            .into_iter()
            .find(|c| c.uuid == uuids::HEART_RATE_MEASUREMENT)
            .ok_or_else(|| anyhow!("heart rate characteristic not found on device"))?;

        found.subscribe(&hr_char).await.context("subscribe to HR")?;
        *state.lock() = ConnectionState::Connected;

        // Best-effort battery read.
        if let Some(bat_char) = found
            .characteristics()
            .into_iter()
            .find(|c| c.uuid == uuids::BATTERY_LEVEL)
        {
            if let Ok(data) = found.read(&bat_char).await {
                *battery_slot.lock() = data.first().copied();
            }
        }

        *peripheral_slot.lock().await = Some(found.clone());

        let peripheral_id = found.id();
        let address_owned = address_str.to_owned();
        // Second event stream for disconnect detection while the first was consumed.
        let mut central_events = adapter.events().await.context("central events")?;
        let mut stream = found.notifications().await.context("notifications")?;
        let state2 = state.clone();
        let slot2 = peripheral_slot.clone();
        let battery2 = battery_slot.clone();

        tokio::spawn(async move {
            let _adapter = adapter; // keep adapter alive so central events flow
            loop {
                tokio::select! {
                    notif = stream.next() => match notif {
                        Some(n) if n.uuid == uuids::HEART_RATE_MEASUREMENT => {
                            if let Some(sample) = parse_hr_measurement(&n.value) {
                                if tx.send(sample).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Some(_) => {}
                        None => break,
                    },
                    evt = central_events.next() => match evt {
                        Some(CentralEvent::DeviceDisconnected(id)) if id == peripheral_id => {
                            tracing::warn!(address = %address_owned, "BLE device disconnected unexpectedly");
                            break;
                        }
                        None => break,
                        _ => {}
                    },
                }
            }
            *state2.lock() = ConnectionState::Disconnected;
            *battery2.lock() = None;
            *slot2.lock().await = None;
        });

        Ok(())
    }

    pub async fn disconnect(
        peripheral_slot: Arc<tokio::sync::Mutex<Option<Peripheral>>>,
    ) -> Result<()> {
        use btleplug::api::Peripheral as _;
        if let Some(p) = peripheral_slot.lock().await.take() {
            p.disconnect().await.context("disconnect")?;
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

    pub async fn scan(timeout_ms: u64, progress: mpsc::Sender<DeviceInfo>) -> Result<Vec<DeviceInfo>> {
        let handler = get_handler().context("blec handler not initialised")?;

        // On Android the very first scan may fail if a Bluetooth permission
        // dialog was just dismissed. Retry once after a short delay.
        let mut rx = {
            let (tx, rx) = mpsc::channel::<Vec<BleDevice>>(16);
            match handler.discover(Some(tx), timeout_ms, ScanFilter::None, false).await {
                Ok(()) => rx,
                Err(_) => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    let (tx2, rx2) = mpsc::channel::<Vec<BleDevice>>(16);
                    handler
                        .discover(Some(tx2), timeout_ms, ScanFilter::None, false)
                        .await
                        .context("blec discover")?;
                    rx2
                }
            }
        };

        let mut seen: std::collections::HashMap<String, DeviceInfo> = Default::default();
        while let Some(batch) = rx.recv().await {
            for d in batch {
                if !seen.contains_key(&d.address) {
                    let advertises_hr = d.services.contains(&uuids::HEART_RATE_SERVICE);
                    let info = DeviceInfo {
                        address: d.address.clone(),
                        name: Some(d.name),
                        rssi: d.rssi,
                        advertises_hr,
                    };
                    progress.send(info.clone()).await.ok();
                    seen.insert(d.address, info);
                }
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
        let address_owned = address.to_owned();

        handler
            .connect(
                address,
                (move || {
                    tracing::warn!(address = %address_owned, "BLE device disconnected unexpectedly");
                    *state2.lock() = ConnectionState::Disconnected;
                })
                .into(),
                false,
            )
            .await
            .context("blec connect")?;
        *state.lock() = ConnectionState::Connected;

        if let Ok(data) = handler.recv_data(uuids::BATTERY_LEVEL, None).await {
            *battery_slot.lock() = data.first().copied();
        }

        // Spawn so samples are delivered via the async channel rather than
        // blocking inside the blec callback (avoids dropping under backpressure).
        handler
            .subscribe(
                uuids::HEART_RATE_MEASUREMENT,
                None,
                move |data: Vec<u8>| {
                    if let Some(s) = parse_hr_measurement(&data) {
                        let tx = tx.clone();
                        tokio::spawn(async move { tx.send(s).await.ok(); });
                    }
                },
            )
            .await
            .context("blec subscribe")?;
        Ok(())
    }

    pub async fn disconnect() -> Result<()> {
        if let Ok(h) = tauri_plugin_blec::get_handler() {
            if let Err(e) = h.disconnect().await {
                tracing::warn!(error = %e, "blec disconnect error");
            }
        }
        Ok(())
    }
}

#[async_trait]
impl HeartRateSource for BlecSource {
    async fn scan(&self, timeout_ms: u64, progress: mpsc::Sender<DeviceInfo>) -> Result<Vec<DeviceInfo>> {
        *self.state.lock() = ConnectionState::Scanning;
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        let result = desktop::scan(&self.adapter, timeout_ms, progress).await;
        #[cfg(any(target_os = "ios", target_os = "android"))]
        let result = mobile::scan(timeout_ms, progress).await;
        *self.state.lock() = ConnectionState::Disconnected;
        result
    }

    async fn connect(&self, address: &str, tx: mpsc::Sender<HrSample>) -> Result<()> {
        *self.state.lock() = ConnectionState::Connecting;
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        let result = desktop::connect(
            &self.adapter,
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
