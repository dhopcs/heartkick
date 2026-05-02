//! In-process log ring buffer + a [`tracing_subscriber::Layer`] that feeds it.
//!
//! Call `get_logs` from the Tauri command surface to stream recent entries to
//! the frontend log viewer.

use std::collections::VecDeque;
use std::fmt::Write as FmtWrite;
use std::sync::OnceLock;

use parking_lot::Mutex;

const CAPACITY: usize = 500;

static BUF: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();

fn buf() -> &'static Mutex<VecDeque<String>> {
    BUF.get_or_init(|| Mutex::new(VecDeque::with_capacity(CAPACITY)))
}

fn push(line: String) {
    let mut g = buf().lock();
    if g.len() >= CAPACITY {
        g.pop_front();
    }
    g.push_back(line);
}

/// Return the most recent `n` log lines (oldest first).
pub fn recent(n: usize) -> Vec<String> {
    let g = buf().lock();
    let start = g.len().saturating_sub(n);
    g.iter().skip(start).cloned().collect()
}

/// A [`tracing_subscriber::Layer`] that captures formatted events into the
/// in-process ring buffer.
pub struct LogLayer;

impl<S: tracing::Subscriber> tracing_subscriber::layer::Layer<S> for LogLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let meta = event.metadata();
        let now = chrono::Local::now().format("%H:%M:%S%.3f");
        let level = meta.level();
        let target = meta.target();
        let mut body = String::new();

        struct Visitor<'a>(&'a mut String);
        impl tracing::field::Visit for Visitor<'_> {
            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                if field.name() == "message" {
                    self.0.push_str(value);
                } else {
                    write!(self.0, " {}={}", field.name(), value).ok();
                }
            }
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    write!(self.0, "{value:?}").ok();
                } else {
                    write!(self.0, " {}={value:?}", field.name()).ok();
                }
            }
        }

        event.record(&mut Visitor(&mut body));
        push(format!("{now} {level:<5} [{target}] {body}"));
    }
}
