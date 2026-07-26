pub struct Messenger<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    _manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Messenger<'a, R, M> {
    pub fn ping(&self) -> Result<String, crate::Error> {
        Ok("pong".to_string())
    }
}

pub trait MessengerPluginExt<R: tauri::Runtime> {
    fn messenger(&self) -> Messenger<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> MessengerPluginExt<R> for T {
    fn messenger(&self) -> Messenger<'_, R, Self>
    where
        Self: Sized,
    {
        Messenger {
            _manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
