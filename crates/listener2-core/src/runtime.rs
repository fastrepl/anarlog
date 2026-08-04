use crate::BatchEvent;
use crate::DenoiseEvent;

pub trait BatchRuntime: Send + Sync + 'static {
    fn emit(&self, event: BatchEvent);

    fn is_cancelled(&self) -> bool {
        false
    }
}

pub trait DenoiseRuntime: Send + Sync + 'static {
    fn emit(&self, event: DenoiseEvent);
}
