/// Lowers other audio while dictation listens. The platform implementation belongs to the
/// insertion/native lane; the engine only holds the guard for as long as capture runs.
pub trait OutputDucker: Send + Sync + 'static {
    /// Lowers output and returns a guard that restores only what it changed when dropped.
    /// None when unsupported or nothing was changed.
    fn duck(&self) -> Option<Box<dyn Send>>;
}

pub struct NoDucker;

impl OutputDucker for NoDucker {
    fn duck(&self) -> Option<Box<dyn Send>> {
        None
    }
}
