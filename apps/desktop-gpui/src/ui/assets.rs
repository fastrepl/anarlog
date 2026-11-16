use std::borrow::Cow;

use gpui::{AssetSource, SharedString};

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        let bytes: Option<&'static [u8]> = match path {
            "FileAddIcon.svg" => Some(include_bytes!("../../assets/FileAddIcon.svg")),
            "Search01Icon.svg" => Some(include_bytes!("../../assets/Search01Icon.svg")),
            "PencilEdit01Icon.svg" => Some(include_bytes!("../../assets/PencilEdit01Icon.svg")),
            "NoteEditIcon.svg" => Some(include_bytes!("../../assets/NoteEditIcon.svg")),
            "InfoIcon.svg" => Some(include_bytes!("../../assets/InfoIcon.svg")),
            "ArrowUp01Icon.svg" => Some(include_bytes!("../../assets/ArrowUp01Icon.svg")),
            "ArrowDown01Icon.svg" => Some(include_bytes!("../../assets/ArrowDown01Icon.svg")),
            _ => None,
        };
        Ok(bytes.map(Cow::Borrowed))
    }

    fn list(&self, _: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok([
            "FileAddIcon.svg",
            "Search01Icon.svg",
            "PencilEdit01Icon.svg",
            "NoteEditIcon.svg",
            "InfoIcon.svg",
            "ArrowUp01Icon.svg",
            "ArrowDown01Icon.svg",
        ]
        .into_iter()
        .map(Into::into)
        .collect())
    }
}
