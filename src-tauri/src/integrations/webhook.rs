//! Generic templated HTTP webhook.
//!
//! Supports `{bpm}`, `{rr}`, `{timestamp}`, `{device}` substitutions in URL,
//! header values and body. Throttled by `min_interval_ms`.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use parking_lot::Mutex;
use reqwest::Method;

use crate::config::WebhookConfig;
use crate::core::EngineEvent;
use crate::integrations::Integration;

pub struct WebhookIntegration {
    cfg: WebhookConfig,
    client: reqwest::Client,
    last_fire: Mutex<Option<Instant>>,
}

impl WebhookIntegration {
    pub fn new(cfg: WebhookConfig) -> Self {
        Self {
            cfg,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("reqwest client"),
            last_fire: Mutex::new(None),
        }
    }

    fn render(&self, template: &str, evt: &EngineEvent) -> String {
        let (bpm, rr, ts, device) = match evt {
            EngineEvent::Sample { sample, .. } => (
                sample.bpm.to_string(),
                sample
                    .rr_intervals_ms
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
                sample.timestamp.to_rfc3339(),
                String::new(),
            ),
            EngineEvent::State { device, .. } => (
                String::new(),
                String::new(),
                chrono::Utc::now().to_rfc3339(),
                device.clone().unwrap_or_default(),
            ),
            EngineEvent::SessionReset | EngineEvent::DeviceFound { .. } => (
                String::new(),
                String::new(),
                chrono::Utc::now().to_rfc3339(),
                String::new(),
            ),
        };
        template
            .replace("{bpm}", &bpm)
            .replace("{rr}", &rr)
            .replace("{timestamp}", &ts)
            .replace("{device}", &device)
    }
}

#[async_trait]
impl Integration for WebhookIntegration {
    fn name(&self) -> &str {
        &self.cfg.name
    }

    fn wants(&self, event: &EngineEvent) -> bool {
        matches!(event, EngineEvent::Sample { .. })
    }

    async fn handle(&self, event: &EngineEvent) {
        // Throttle.
        {
            let mut last = self.last_fire.lock();
            let now = Instant::now();
            if let Some(prev) = *last {
                if now.duration_since(prev) < Duration::from_millis(self.cfg.min_interval_ms) {
                    return;
                }
            }
            *last = Some(now);
        }

        let url = self.render(&self.cfg.url, event);
        let body = self.render(&self.cfg.body, event);
        let method = Method::from_bytes(self.cfg.method.as_bytes()).unwrap_or(Method::POST);

        let mut req = self.client.request(method, url).body(body);
        for (k, v) in &self.cfg.headers {
            req = req.header(k, self.render(v, event));
        }
        if let Err(e) = req.send().await {
            tracing::warn!(integration = %self.cfg.name, error = %e, "webhook request failed");
        }
    }
}
