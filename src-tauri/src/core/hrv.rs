use std::collections::VecDeque;

const WINDOW: usize = 60;

#[derive(Debug, Default)]
pub struct HrvCalc {
    rr: VecDeque<u16>,
}

impl HrvCalc {
    pub fn push(&mut self, rr_intervals: &[u16]) {
        for v in rr_intervals {
            if self.rr.len() >= WINDOW {
                self.rr.pop_front();
            }
            self.rr.push_back(*v);
        }
    }

    /// Returns the current RMSSD in milliseconds, or `None` when the window has
    /// fewer than two samples.
    pub fn rmssd(&self) -> Option<f32> {
        if self.rr.len() < 2 {
            return None;
        }
        let mut sum_sq = 0f64;
        let mut n = 0u32;
        let mut prev = None;
        for v in &self.rr {
            if let Some(p) = prev {
                let d = *v as f64 - p as f64;
                sum_sq += d * d;
                n += 1;
            }
            prev = Some(*v);
        }
        if n == 0 {
            None
        } else {
            Some(((sum_sq / n as f64).sqrt()) as f32)
        }
    }

    pub fn reset(&mut self) {
        self.rr.clear();
    }
}
