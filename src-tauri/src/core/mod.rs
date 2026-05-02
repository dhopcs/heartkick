//! Core orchestration: ties a [`HeartRateSource`] to consumers via broadcast,
//! tracks the live session and dispatches integrations.

pub mod engine;
pub mod history;
pub mod hrv;
pub mod session;

pub use engine::{Engine, EngineEvent, EngineSnapshot};
