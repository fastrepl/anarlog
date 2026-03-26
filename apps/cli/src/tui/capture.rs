use std::collections::VecDeque;
use std::fmt::Write;
use std::sync::{Arc, Mutex};

use tracing_subscriber::Layer;

const TRACE_CAPACITY: usize = 64;

pub type TraceBuffer = Arc<Mutex<VecDeque<String>>>;

pub fn new_trace_buffer() -> TraceBuffer {
    Arc::new(Mutex::new(VecDeque::with_capacity(TRACE_CAPACITY)))
}

pub struct CaptureLayer {
    buffer: TraceBuffer,
}

impl CaptureLayer {
    pub fn new(buffer: TraceBuffer) -> Self {
        Self { buffer }
    }
}

impl<S: tracing::Subscriber> Layer<S> for CaptureLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let meta = event.metadata();
        let level = meta.level();
        let target = meta.target();

        let mut message = String::new();
        let mut visitor = MessageVisitor(&mut message);
        event.record(&mut visitor);

        let line = format!("{level:>5} {target}: {message}");
        if let Ok(mut buf) = self.buffer.lock() {
            if buf.len() >= TRACE_CAPACITY {
                buf.pop_front();
            }
            buf.push_back(line);
        }
    }
}

struct MessageVisitor<'a>(&'a mut String);

impl tracing::field::Visit for MessageVisitor<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(self.0, "{:?}", value);
        } else {
            if !self.0.is_empty() {
                self.0.push(' ');
            }
            let _ = write!(self.0, "{}={:?}", field.name(), value);
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            let _ = write!(self.0, "{}", value);
        } else {
            if !self.0.is_empty() {
                self.0.push(' ');
            }
            let _ = write!(self.0, "{}={}", field.name(), value);
        }
    }
}
