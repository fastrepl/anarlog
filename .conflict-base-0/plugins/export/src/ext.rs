use std::path::Path;

pub struct Export<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    _manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Export<'a, R, M> {
    pub fn export_pdf(
        &self,
        path: impl AsRef<Path>,
        input: impl Into<crate::ExportInput>,
    ) -> Result<(), crate::Error> {
        hypr_export_core::export_pdf(path, input)
    }
}

pub trait ExportPluginExt<R: tauri::Runtime> {
    fn export(&self) -> Export<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> ExportPluginExt<R> for T {
    fn export(&self) -> Export<'_, R, Self>
    where
        Self: Sized,
    {
        Export {
            _manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
