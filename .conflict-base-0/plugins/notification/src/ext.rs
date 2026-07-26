use crate::error::Error;

pub struct Notification<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Notification<'a, R, M> {
    #[tracing::instrument(skip(self))]
    pub fn show(&self, v: hypr_notification::Notification) -> Result<(), Error> {
        let _ = self.manager;
        hypr_notification::show(&v);
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub fn clear(&self) -> Result<(), Error> {
        let _ = self.manager;
        hypr_notification::clear();
        Ok(())
    }
}

pub trait NotificationPluginExt<R: tauri::Runtime> {
    fn notification(&self) -> Notification<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> NotificationPluginExt<R> for T {
    fn notification(&self) -> Notification<'_, R, Self>
    where
        Self: Sized,
    {
        Notification {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
