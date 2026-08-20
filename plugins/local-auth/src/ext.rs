use tauri::Runtime;

use crate::Error;

pub struct LocalAuth<'a, R: Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: Runtime, M: tauri::Manager<R>> LocalAuth<'a, R, M> {
    pub fn available(&self) -> Result<bool, Error> {
        #[cfg(target_os = "macos")]
        {
            crate::macos::available(&self.manager.app_handle())
        }

        #[cfg(target_os = "windows")]
        {
            crate::windows::available()
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Ok(false)
        }
    }

    pub fn authenticate(&self, reason: &str) -> Result<bool, Error> {
        #[cfg(target_os = "macos")]
        {
            crate::macos::authenticate(&self.manager.app_handle(), reason)
        }

        #[cfg(target_os = "windows")]
        {
            let _ = self.manager;
            crate::windows::authenticate(reason)
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = self.manager;
            let _ = reason;
            Err(Error::Unavailable)
        }
    }
}

pub trait LocalAuthPluginExt<R: Runtime> {
    fn local_auth(&self) -> LocalAuth<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: Runtime, T: tauri::Manager<R>> LocalAuthPluginExt<R> for T {
    fn local_auth(&self) -> LocalAuth<'_, R, Self>
    where
        Self: Sized,
    {
        LocalAuth {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
