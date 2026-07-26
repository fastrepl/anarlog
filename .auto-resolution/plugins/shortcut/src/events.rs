use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum Modifier {
    Command,
    Option,
    Shift,
    Control,
    Fn,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
pub struct HotKey {
    pub key: Option<u16>,
    pub modifiers: Vec<Modifier>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    #[serde(default)]
    pub use_double_tap_only: bool,
    #[serde(default = "default_true")]
    pub double_tap_lock_enabled: bool,
    #[serde(default = "default_min_key_time_ms")]
    pub minimum_key_time_ms: u64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            use_double_tap_only: false,
            double_tap_lock_enabled: true,
            minimum_key_time_ms: default_min_key_time_ms(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_min_key_time_ms() -> u64 {
    150
}

#[macro_export]
macro_rules! common_event_derives {
    ($item:item) => {
        #[derive(
            serde::Serialize, serde::Deserialize, Clone, specta::Type, tauri_specta::Event,
        )]
        $item
    };
}

common_event_derives! {
    #[serde(tag = "type", rename_all = "camelCase")]
    pub enum ShortcutEvent {
        Pressed,
        Released,
        Cancelled,
        Discarded,
    }
}
