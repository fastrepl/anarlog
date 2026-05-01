pub struct Webhook<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Webhook<'a, R, M> {
    pub fn todo(&self) -> Result<String, String> {
        let _ = self.manager;
        Ok("Webhook todo functionality not yet implemented".to_string())
    }
}

pub trait WebhookPluginExt<R: tauri::Runtime> {
    fn webhook(&self) -> Webhook<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> WebhookPluginExt<R> for T {
    fn webhook(&self) -> Webhook<'_, R, Self>
    where
        Self: Sized,
    {
        Webhook {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
