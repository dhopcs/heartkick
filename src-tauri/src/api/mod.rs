pub mod controller;
pub mod http;
pub mod socket;

use std::sync::Arc;

use futures::stream::Stream;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt as _;

use crate::core::{Engine, EngineEvent};

/// Fan engine broadcast events into a transport-neutral stream.
///
/// Used by the Tauri event forwarder, the integration dispatcher, and tests.
/// Defined here (not in `http`) so callers don't depend on the HTTP transport.
pub fn event_stream(engine: &Arc<Engine>) -> impl Stream<Item = EngineEvent> {
    BroadcastStream::new(engine.subscribe()).filter_map(|r| r.ok())
}
