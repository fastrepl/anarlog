use crate::BatchEvent;

pub trait BatchRuntime: Send + Sync + 'static {
    fn emit(&self, event: BatchEvent);

    fn is_cancelled(&self) -> bool {
        false
    }
}
