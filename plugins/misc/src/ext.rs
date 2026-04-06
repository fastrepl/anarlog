use hypr_template_support::DeviceInfo;

pub struct Misc<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    #[allow(dead_code)]
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Misc<'a, R, M> {
    pub fn get_git_hash(&self) -> String {
        env!("VERGEN_GIT_SHA").to_string()
    }

    pub fn get_fingerprint(&self) -> String {
        hypr_host::fingerprint()
    }

    pub fn get_device_info(&self, locale: Option<String>) -> DeviceInfo {
        DeviceInfo {
            platform: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            os_version: sysinfo::System::long_os_version().unwrap_or_default(),
            app_version: self.manager.package_info().version.to_string(),
            build_hash: Some(self.get_git_hash()),
            locale,
        }
    }

    pub fn opinionated_md_to_html(&self, text: impl AsRef<str>) -> Result<String, String> {
        hypr_buffer::opinionated_md_to_html(text.as_ref()).map_err(|e| e.to_string())
    }
}

pub trait MiscPluginExt<R: tauri::Runtime> {
    fn misc(&self) -> Misc<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> MiscPluginExt<R> for T {
    fn misc(&self) -> Misc<'_, R, Self>
    where
        Self: Sized,
    {
        Misc {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
