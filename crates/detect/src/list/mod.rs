#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn list_installed_apps() -> Vec<InstalledApp> {
    Vec::new()
}

pub fn list_mic_using_apps() -> Vec<InstalledApp> {
    let apps = {
        #[cfg(target_os = "macos")]
        {
            macos::list_mic_using_apps()
        }
        #[cfg(target_os = "linux")]
        {
            linux::list_mic_using_apps()
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            Vec::<InstalledApp>::new()
        }
    };

    apps.into_iter()
        .filter(|app| !app.id.to_lowercase().contains("hyprnote"))
        .collect()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct InstalledApp {
    pub id: String,
    pub name: String,
}
