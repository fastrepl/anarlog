use std::path::PathBuf;

pub trait StorageRuntime: Send + Sync + 'static {
    fn global_base(&self) -> Result<PathBuf, crate::Error>;
    fn vault_base(&self) -> Result<PathBuf, crate::Error>;
}
