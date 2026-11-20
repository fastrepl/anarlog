use std::sync::{Arc, Mutex};

/// One microphone for dictation: an operation holds this lease until its capture has closed the
/// microphone, so the next press cannot open it under one that is still finishing.
#[derive(Clone, Default)]
pub struct MicLease(Arc<Mutex<bool>>);

pub struct MicGuard(Arc<Mutex<bool>>);

impl MicLease {
    pub fn try_acquire(&self) -> Option<MicGuard> {
        let mut held = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if *held {
            return None;
        }
        *held = true;
        Some(MicGuard(self.0.clone()))
    }
}

impl Drop for MicGuard {
    fn drop(&mut self) {
        *self.0.lock().unwrap_or_else(|error| error.into_inner()) = false;
    }
}
