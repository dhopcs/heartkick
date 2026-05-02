//! Live session statistics: min, max, avg BPM and elapsed duration.

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::bluetooth::HrSample;

#[derive(Debug, Clone, Default, Serialize)]
pub struct SessionStats {
    pub started_at: Option<DateTime<Utc>>,
    pub last_at: Option<DateTime<Utc>>,
    pub samples: u64,
    pub min_bpm: Option<u16>,
    pub max_bpm: Option<u16>,
    pub avg_bpm: Option<f32>,
    /// Sum used for streaming average. Not exposed on the wire.
    #[serde(skip)]
    sum_bpm: u64,
}

impl SessionStats {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn record(&mut self, sample: &HrSample) {
        if self.started_at.is_none() {
            self.started_at = Some(sample.timestamp);
        }
        self.last_at = Some(sample.timestamp);
        self.samples += 1;
        self.sum_bpm += sample.bpm as u64;
        self.avg_bpm = Some(self.sum_bpm as f32 / self.samples as f32);
        self.min_bpm = Some(self.min_bpm.map_or(sample.bpm, |m| m.min(sample.bpm)));
        self.max_bpm = Some(self.max_bpm.map_or(sample.bpm, |m| m.max(sample.bpm)));
    }

    pub fn duration_secs(&self) -> u64 {
        match (self.started_at, self.last_at) {
            (Some(s), Some(l)) => (l - s).num_seconds().max(0) as u64,
            _ => 0,
        }
    }
}
