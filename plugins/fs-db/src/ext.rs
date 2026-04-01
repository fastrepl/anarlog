pub struct FsDb<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> FsDb<'a, R, M> {
    pub fn ensure_version_file(&self) -> crate::Result<()> {
        use tauri_plugin_settings::SettingsPluginExt;

        let base_dir = self
            .manager
            .app_handle()
            .settings()
            .resolve_startup_vault_base()?;

        if crate::version::known::exists(&base_dir) {
            return Ok(());
        }

        let app_version = self
            .manager
            .app_handle()
            .config()
            .version
            .as_ref()
            .map_or_else(
                || hypr_version::Version::new(0, 0, 0),
                |v| {
                    v.parse::<hypr_version::Version>()
                        .expect("version must be semver")
                },
            );

        crate::version::write_version(&base_dir, &app_version)?;
        Ok(())
    }
}

pub trait FsDbPluginExt<R: tauri::Runtime> {
    fn fs_db(&self) -> FsDb<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> FsDbPluginExt<R> for T {
    fn fs_db(&self) -> FsDb<'_, R, Self>
    where
        Self: Sized,
    {
        FsDb {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
