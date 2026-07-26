use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct GlobalTimer {
    inner: Arc<Mutex<GlobalTimerInner>>,
}

#[derive(Debug)]
struct GlobalTimerInner {
    accumulated_duration: f64,
}

impl GlobalTimer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(GlobalTimerInner {
                accumulated_duration: 0.0,
            })),
        }
    }

    pub fn add_audio_duration(&self, duration_seconds: f64) -> f64 {
        let mut inner = self.inner.lock().unwrap();
        let current_offset = inner.accumulated_duration;
        inner.accumulated_duration += duration_seconds;
        current_offset
    }

    pub fn current_duration(&self) -> f64 {
        self.inner.lock().unwrap().accumulated_duration
    }
}

impl Default for GlobalTimer {
    fn default() -> Self {
        Self::new()
    }
}
