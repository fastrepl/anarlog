use hypr_hooks::{HookEvent, HookResult, run_hooks_for_event};

use crate::config;

pub struct Hooks<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Hooks<'a, R, M> {
    pub async fn handle_event(&self, event: HookEvent) -> crate::Result<Vec<HookResult>> {
        let config = config::load_config(self.manager).await?;
        Ok(run_hooks_for_event(&config, event).await)
    }
}

pub trait HooksPluginExt<R: tauri::Runtime> {
    fn hooks(&self) -> Hooks<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> HooksPluginExt<R> for T {
    fn hooks(&self) -> Hooks<'_, R, Self>
    where
        Self: Sized,
    {
        Hooks {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
