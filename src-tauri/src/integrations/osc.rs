//! OSC publisher for VR overlays and stream tooling.

use std::net::UdpSocket;

use async_trait::async_trait;
use parking_lot::Mutex;
use rosc::{encoder, OscMessage, OscPacket, OscType};

use crate::config::OscConfig;
use crate::core::EngineEvent;
use crate::integrations::Integration;

pub struct OscIntegration {
    cfg: OscConfig,
    socket: Mutex<Option<UdpSocket>>,
}

impl OscIntegration {
    pub fn new(cfg: OscConfig) -> Self {
        Self {
            cfg,
            socket: Mutex::new(None),
        }
    }

    fn ensure_socket(&self) -> Option<()> {
        let mut g = self.socket.lock();
        if g.is_none() {
            match UdpSocket::bind("0.0.0.0:0") {
                Ok(s) => {
                    let _ = s.set_nonblocking(true);
                    *g = Some(s);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "OSC bind failed");
                    return None;
                }
            }
        }
        Some(())
    }
}

#[async_trait]
impl Integration for OscIntegration {
    fn name(&self) -> &str {
        "osc"
    }

    fn wants(&self, event: &EngineEvent) -> bool {
        matches!(event, EngineEvent::Sample { .. })
    }

    async fn handle(&self, event: &EngineEvent) {
        let bpm = match event {
            EngineEvent::Sample { sample, .. } => sample.bpm,
            _ => return,
        };
        if self.ensure_socket().is_none() {
            return;
        }

        let packet = OscPacket::Message(OscMessage {
            addr: self.cfg.address.clone(),
            args: vec![OscType::Int(bpm as i32)],
        });
        let bytes = match encoder::encode(&packet) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(error = %e, "OSC encode failed");
                return;
            }
        };
        if let Some(s) = self.socket.lock().as_ref() {
            let _ = s.send_to(&bytes, &self.cfg.target);
        }
    }
}
