use crate::events::LocalServerEvent;

pub trait LocalServerRuntime: Send + Sync + 'static {
    fn emit(&self, event: LocalServerEvent);
}

#[derive(Debug, Default)]
pub struct NoopRuntime;

impl LocalServerRuntime for NoopRuntime {
    fn emit(&self, _event: LocalServerEvent) {}
}
