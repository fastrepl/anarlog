use tauri_plugin_settings::SettingsPluginExt;

pub struct FsSync<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> FsSync<'a, R, M> {
    fn core(&self) -> Result<hypr_fs_sync_core::FsSyncCore, crate::Error> {
        let base_dir = self
            .manager
            .app_handle()
            .settings()
            .vault_base()
            .map(|p| p.into_std_path_buf())
            .map_err(|e| crate::Error::Path(e.to_string()))?;

        Ok(hypr_fs_sync_core::FsSyncCore::new(base_dir))
    }

    pub fn list_folders(&self) -> Result<crate::ListFoldersResult, crate::Error> {
        self.core()?.list_folders()
    }

    pub fn move_session(
        &self,
        session_id: &str,
        from_folder_path: &str,
        target_folder_path: &str,
    ) -> Result<crate::MoveSessionResult, crate::Error> {
        self.core()?
            .move_session(session_id, from_folder_path, target_folder_path)
    }

    pub fn create_folder(&self, folder_path: &str) -> Result<(), crate::Error> {
        self.core()?.create_folder(folder_path)
    }

    pub fn rename_folder(
        &self,
        old_path: &str,
        new_path: &str,
    ) -> Result<crate::RenameFolderResult, crate::Error> {
        self.core()?.rename_folder(old_path, new_path)
    }

    pub fn delete_folder(&self, folder_path: &str) -> Result<(), crate::Error> {
        self.core()?.delete_folder(folder_path)
    }

    pub fn cleanup_orphan(
        &self,
        target: crate::CleanupTarget,
        valid_ids: Vec<String>,
    ) -> Result<u32, crate::Error> {
        self.core()?.cleanup_orphan(target, valid_ids)
    }

    pub fn attachment_save(
        &self,
        session_id: &str,
        data: &[u8],
        filename: &str,
    ) -> Result<crate::AttachmentSaveResult, crate::Error> {
        self.core()?.attachment_save(session_id, data, filename)
    }

    pub fn attachment_list(
        &self,
        session_id: &str,
    ) -> Result<Vec<crate::AttachmentInfo>, crate::Error> {
        self.core()?.attachment_list(session_id)
    }

    pub fn attachment_remove(
        &self,
        session_id: &str,
        attachment_id: &str,
    ) -> Result<(), crate::Error> {
        self.core()?.attachment_remove(session_id, attachment_id)
    }
}

pub trait FsSyncPluginExt<R: tauri::Runtime> {
    fn fs_sync(&self) -> FsSync<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> FsSyncPluginExt<R> for T {
    fn fs_sync(&self) -> FsSync<'_, R, Self>
    where
        Self: Sized,
    {
        FsSync {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
