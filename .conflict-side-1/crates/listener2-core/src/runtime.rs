use crate::BatchEvent;
use crate::DenoiseEvent;

pub trait BatchRuntime: Send + Sync + 'static {
    fn emit(&self, event: BatchEvent);
}

pub trait DenoiseRuntime: Send + Sync + 'static {
    fn emit(&self, event: DenoiseEvent);
}
