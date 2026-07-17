#[macro_export]
macro_rules! common_event_derives {
    ($item:item) => {
        #[derive(
            Debug, serde::Serialize, serde::Deserialize, Clone, specta::Type, tauri_specta::Event,
        )]
        $item
    };
}

common_event_derives! {
    pub struct FileChanged {
        pub path: String,
    }
}
