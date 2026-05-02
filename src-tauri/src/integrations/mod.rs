//! Integration framework. The [`Integration`] trait is intentionally tiny so
//! adding a new sink is as small as one struct + one `impl`.

pub mod osc;
pub mod overlay;
pub mod prometheus;
pub mod webhook;

use std::sync::Arc;

use async_trait::async_trait;
use tokio_stream::StreamExt;

use crate::core::{Engine, EngineEvent};

#[async_trait]
pub trait Integration: Send + Sync {
    /// Stable identifier shown to users.
    fn name(&self) -> &str;
    /// Whether the integration should be invoked for this event.
    fn wants(&self, _event: &EngineEvent) -> bool {
        true
    }
    /// Handle an event. Errors are logged but never propagated.
    async fn handle(&self, event: &EngineEvent);
}

/// Holds the integrations enabled for the running engine and dispatches events
/// to them on a single background task.
pub struct Registry {
    integrations: Vec<Arc<dyn Integration>>,
}

impl Registry {
    pub fn new(integrations: Vec<Arc<dyn Integration>>) -> Arc<Self> {
        Arc::new(Self { integrations })
    }

    pub fn list(&self) -> Vec<&str> {
        self.integrations.iter().map(|i| i.name()).collect()
    }

    /// Build the registry from the live config.
    pub fn from_config(
        cfg: &crate::config::Config,
        engine: &std::sync::Arc<crate::core::Engine>,
    ) -> Arc<Self> {
        let mut out: Vec<Arc<dyn Integration>> = Vec::new();
        for w in &cfg.integrations.webhooks {
            if w.enabled {
                out.push(Arc::new(webhook::WebhookIntegration::new(w.clone())));
            }
        }
        if cfg.integrations.osc.enabled {
            out.push(Arc::new(osc::OscIntegration::new(
                cfg.integrations.osc.clone(),
            )));
        }
        if let Some(push) = &cfg.integrations.prometheus.push {
            if push.enabled && !push.url.is_empty() {
                out.push(Arc::new(prometheus::PrometheusPushIntegration::new(
                    push.clone(),
                    engine.clone(),
                )));
            }
        }
        Self::new(out)
    }

    /// Spawn the dispatch task. Returns immediately.
    pub fn spawn(self: Arc<Self>, engine: Arc<Engine>) {
        if self.integrations.is_empty() {
            return;
        }
        tokio::spawn(async move {
            let mut stream = Box::pin(crate::api::event_stream(&engine));
            while let Some(evt) = stream.next().await {
                for integ in &self.integrations {
                    if integ.wants(&evt) {
                        integ.handle(&evt).await;
                    }
                }
            }
        });
    }
}
