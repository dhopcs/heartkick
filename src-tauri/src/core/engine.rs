//! The [`Engine`] is the single source of truth: it owns the heart rate source,
//! the session statistics, the history store, and a broadcast channel that any
//! consumer (Tauri commands, HTTP, IPC socket, integrations) can subscribe to.

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use anyhow::Result;
use parking_lot::RwLock;
use serde::Serialize;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::time::{sleep, Duration};

use crate::bluetooth::{ConnectionState, DeviceInfo, HeartRateSource, HrSample};
use crate::config::Config;
use crate::core::{history::HistoryStore, hrv::HrvCalc, session::SessionStats};

/// Snapshot of the engine state as exposed to consumers.
#[derive(Debug, Clone, Serialize)]
pub struct EngineSnapshot {
    pub state: ConnectionState,
    pub device_address: Option<String>,
    pub last_sample: Option<HrSample>,
    pub session: SessionStats,
    pub rmssd: Option<f32>,
    pub battery: Option<u8>,
}

/// Events fanned out on the engine's broadcast channel.
///
/// `Sample` carries the authoritative `rmssd` and `session` snapshot so that
/// consumers (Tauri webview, integrations, IPC) never need a separate
/// `snapshot()` call to derive those values.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EngineEvent {
    Sample {
        #[serde(flatten)]
        sample: HrSample,
        rmssd: Option<f32>,
        session: crate::core::session::SessionStats,
    },
    State {
        state: ConnectionState,
        device: Option<String>,
    },
    SessionReset,
}

pub struct Engine {
    source: Arc<dyn HeartRateSource>,
    history: Arc<dyn HistoryStore>,
    config: Arc<RwLock<Config>>,
    session: RwLock<SessionStats>,
    hrv: RwLock<HrvCalc>,
    last_sample: RwLock<Option<HrSample>>,
    device_address: RwLock<Option<String>>,
    events: broadcast::Sender<EngineEvent>,
    /// Lock held while a connect task is running.
    connect_lock: Mutex<()>,
    /// Incremented on every disconnect (user-initiated or via reconnect).
    /// The background reconnect task compares its captured generation and exits
    /// when it no longer matches, preventing stale reconnect loops.
    connect_generation: Arc<AtomicU64>,
}

impl Engine {
    pub fn new(
        source: Arc<dyn HeartRateSource>,
        history: Arc<dyn HistoryStore>,
        config: Arc<RwLock<Config>>,
    ) -> Arc<Self> {
        // 32 slots: at ~1 Hz that is 32 seconds of backlog before lagging.
        let (events, _) = broadcast::channel(32);
        Arc::new(Self {
            source,
            history,
            config,
            session: RwLock::new(SessionStats::default()),
            hrv: RwLock::new(HrvCalc::default()),
            last_sample: RwLock::new(None),
            device_address: RwLock::new(None),
            events,
            connect_lock: Mutex::new(()),
            connect_generation: Arc::new(AtomicU64::new(0)),
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.events.subscribe()
    }

    pub fn config(&self) -> Arc<RwLock<Config>> {
        self.config.clone()
    }

    pub fn history(&self) -> Arc<dyn HistoryStore> {
        self.history.clone()
    }

    pub async fn scan(&self, timeout_ms: u64) -> Result<Vec<DeviceInfo>> {
        self.source.scan(timeout_ms).await
    }

    pub fn snapshot(&self) -> EngineSnapshot {
        let last = self.last_sample.read().clone();
        EngineSnapshot {
            state: self.source.state(),
            device_address: self.device_address.read().clone(),
            last_sample: last,
            session: self.session.read().clone(),
            rmssd: self.hrv.read().rmssd(),
            battery: self.source.battery_level(),
        }
    }

    pub async fn connect(self: &Arc<Self>, address: String) -> Result<()> {
        // Serialize connects so duplicate calls don't fight.
        let _guard = self.connect_lock.lock().await;
        self.disconnect_inner().await;

        let (tx, mut rx) = mpsc::channel::<HrSample>(8);
        self.source.connect(&address, tx).await?;
        *self.device_address.write() = Some(address.clone());
        // Snapshot the generation *after* disconnect_inner incremented it so
        // the reconnect loop can detect a subsequent user disconnect/reconnect.
        let gen = self.connect_generation.load(Ordering::Relaxed);
        let gen_arc = self.connect_generation.clone();
        let _ = self.events.send(EngineEvent::State {
            state: self.source.state(),
            device: Some(address.clone()),
        });

        let me = self.clone();
        tokio::spawn(async move {
            while let Some(sample) = rx.recv().await {
                me.ingest(sample).await;
            }
            // Channel closed: source disconnected unexpectedly.
            *me.device_address.write() = None;
            let _ = me.events.send(EngineEvent::State {
                state: ConnectionState::Disconnected,
                device: None,
            });

            // Auto-reconnect with exponential backoff.
            // Exits immediately if the generation changed (user disconnect / new connect).
            let mut delay = Duration::from_secs(2);
            loop {
                if gen_arc.load(Ordering::Relaxed) != gen {
                    break;
                }
                sleep(delay).await;
                if gen_arc.load(Ordering::Relaxed) != gen {
                    break;
                }

                let _ = me.events.send(EngineEvent::State {
                    state: ConnectionState::Connecting,
                    device: Some(address.clone()),
                });

                let (tx2, mut rx2) = mpsc::channel::<HrSample>(8);
                match me.source.connect(&address, tx2).await {
                    Ok(()) => {
                        // If the user disconnected while we were connecting, undo it.
                        if gen_arc.load(Ordering::Relaxed) != gen {
                            let _ = me.source.disconnect().await;
                            break;
                        }
                        *me.device_address.write() = Some(address.clone());
                        let _ = me.events.send(EngineEvent::State {
                            state: me.source.state(),
                            device: Some(address.clone()),
                        });
                        delay = Duration::from_secs(2); // reset backoff
                        while let Some(sample) = rx2.recv().await {
                            me.ingest(sample).await;
                        }
                        // Lost connection again — loop and retry.
                        *me.device_address.write() = None;
                        let _ = me.events.send(EngineEvent::State {
                            state: ConnectionState::Disconnected,
                            device: None,
                        });
                    }
                    Err(_) => {
                        // Back off, cap at 30 s.
                        delay = (delay * 2).min(Duration::from_secs(30));
                    }
                }
            }
        });

        Ok(())
    }

    pub async fn disconnect(&self) -> Result<()> {
        self.disconnect_inner().await;
        Ok(())
    }

    async fn disconnect_inner(&self) {
        // Invalidate any running reconnect loop before disconnecting.
        self.connect_generation.fetch_add(1, Ordering::Relaxed);
        let _ = self.source.disconnect().await;
        *self.device_address.write() = None;
        let _ = self.events.send(EngineEvent::State {
            state: ConnectionState::Disconnected,
            device: None,
        });
    }

    pub fn reset_session(&self) {
        self.session.write().reset();
        self.hrv.write().reset();
        let _ = self.events.send(EngineEvent::SessionReset);
    }

    async fn ingest(&self, sample: HrSample) {
        self.session.write().record(&sample);
        self.hrv.write().push(&sample.rr_intervals_ms);
        *self.last_sample.write() = Some(sample.clone());
        self.history.push(sample.clone()).await;
        let rmssd = self.hrv.read().rmssd();
        let session = self.session.read().clone();
        let _ = self.events.send(EngineEvent::Sample {
            sample,
            rmssd,
            session,
        });
    }
}
