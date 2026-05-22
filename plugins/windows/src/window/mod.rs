pub(crate) mod composer;
pub(crate) mod floating;
pub(crate) mod floating_bar;
mod v1;

pub type AppWindow = v1::AppWindow;

pub trait WindowImpl:
    std::fmt::Display
    + std::str::FromStr
    + std::fmt::Debug
    + Clone
    + serde::Serialize
    + serde::de::DeserializeOwned
    + specta::Type
    + PartialEq
    + Eq
    + std::hash::Hash
    + Send
    + Sync
    + 'static
{
    fn label(&self) -> String {
        self.to_string()
    }

    fn title(&self) -> String;

    fn build_window(
        &self,
        app: &tauri::AppHandle<tauri::Wry>,
    ) -> Result<tauri::WebviewWindow, crate::Error>;
}
