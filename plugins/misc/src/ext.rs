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

    pub fn parse_meeting_link(&self, text: impl AsRef<str>) -> Option<String> {
        let text = text.as_ref();

        for regex in MEETING_REGEXES.iter() {
            if let Some(capture) = regex.find(text) {
                return Some(capture.as_str().to_string());
            }
        }

        let url_pattern = r"https?://[^\s]+";
        if let Ok(regex) = regex::Regex::new(url_pattern)
            && let Some(capture) = regex.find(text)
        {
            return Some(capture.as_str().to_string());
        }

        None
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

lazy_static::lazy_static! {
    pub static ref MEETING_REGEXES: Vec<regex::Regex> = vec![
        regex::Regex::new(r"https://meet\.google\.com/[a-z0-9]{3,4}-[a-z0-9]{3,4}-[a-z0-9]{3,4}").unwrap(),
        regex::Regex::new(r"https://[a-z0-9.-]+\.zoom\.us/j/\d+(\?pwd=[a-zA-Z0-9.]+)?").unwrap(),
        regex::Regex::new(r"https://app\.cal\.com/video/[a-zA-Z0-9]+").unwrap(),
    ];
}
