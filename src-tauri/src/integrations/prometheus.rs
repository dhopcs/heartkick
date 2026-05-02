//! Prometheus exposition for the live engine snapshot. Served at `/metrics`,
//! and optionally pushed to a remote endpoint in Prometheus text format.

use std::collections::BTreeMap;
use std::fmt::Write;

use async_trait::async_trait;
use reqwest::Client;

use crate::config::PrometheusPushConfig;
use crate::core::{EngineEvent, EngineSnapshot};

use super::Integration;

pub fn render(snap: &EngineSnapshot) -> String {
    let mut out = String::with_capacity(256);

    out.push_str(
        "# HELP heartkick_bpm Current heart rate in beats per minute\n# TYPE heartkick_bpm gauge\n",
    );
    if let Some(s) = &snap.last_sample {
        let _ = writeln!(out, "heartkick_bpm {}", s.bpm);

        out.push_str("# HELP heartkick_rr_ms Last RR interval in milliseconds\n# TYPE heartkick_rr_ms gauge\n");
        if let Some(&rr) = s.rr_intervals_ms.last() {
            let _ = writeln!(out, "heartkick_rr_ms {rr}");
        }
    }

    out.push_str(
        "# HELP heartkick_rmssd HRV RMSSD in milliseconds\n# TYPE heartkick_rmssd gauge\n",
    );
    if let Some(v) = snap.rmssd {
        let _ = writeln!(out, "heartkick_rmssd {v}");
    }

    out.push_str("# HELP heartkick_battery Battery level of the connected device (0\u{2013}100)\n# TYPE heartkick_battery gauge\n");
    if let Some(v) = snap.battery {
        let _ = writeln!(out, "heartkick_battery {v}");
    }

    out
}

/// Renders a Prometheus text payload for push, identical in schema to `/metrics`
/// but with a Unix millisecond timestamp appended to each line.
fn render_sample(snap: &EngineSnapshot) -> Option<String> {
    let sample = snap.last_sample.as_ref()?;
    let ts_ms = sample.timestamp.timestamp_millis();

    let mut out = String::with_capacity(128);
    let _ = writeln!(out, "heartkick_bpm {} {ts_ms}", sample.bpm);
    if let Some(&rr) = sample.rr_intervals_ms.last() {
        let _ = writeln!(out, "heartkick_rr_ms {rr} {ts_ms}");
    }
    if let Some(v) = snap.battery {
        let _ = writeln!(out, "heartkick_battery {v} {ts_ms}");
    }
    if let Some(v) = snap.rmssd {
        let _ = writeln!(out, "heartkick_rmssd {v} {ts_ms}");
    }
    Some(out)
}

// ── Push integration ─────────────────────────────────────────────────────────

pub struct PrometheusPushIntegration {
    client: Client,
    url: String,
    headers: BTreeMap<String, String>,
    engine: std::sync::Arc<crate::core::Engine>,
}

impl PrometheusPushIntegration {
    pub fn new(cfg: PrometheusPushConfig, engine: std::sync::Arc<crate::core::Engine>) -> Self {
        Self {
            client: Client::new(),
            url: cfg.url,
            headers: cfg.headers,
            engine,
        }
    }
}

#[async_trait]
impl Integration for PrometheusPushIntegration {
    fn name(&self) -> &str {
        "prometheus-push"
    }

    fn wants(&self, event: &EngineEvent) -> bool {
        matches!(event, EngineEvent::Sample { .. })
    }

    async fn handle(&self, _event: &EngineEvent) {
        let snap = self.engine.snapshot();
        let Some(body) = render_sample(&snap) else {
            return;
        };

        let mut req = self
            .client
            .post(&self.url)
            .header("Content-Type", "text/plain");
        for (k, v) in &self.headers {
            req = req.header(k.as_str(), v.as_str());
        }

        if let Err(e) = req.body(body).send().await {
            tracing::warn!(error = %e, url = %self.url, "prometheus push failed");
        }
    }
}
