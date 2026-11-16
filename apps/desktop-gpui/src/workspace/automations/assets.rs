//! Icons of the automations route, served under `automations/<name>.svg`.

const ICONS: &[(&str, &[u8])] = &[
    (
        "ArrowDown01Icon",
        include_bytes!("icons/ArrowDown01Icon.svg"),
    ),
    (
        "ArrowRight02Icon",
        include_bytes!("icons/ArrowRight02Icon.svg"),
    ),
    ("EyeIcon", include_bytes!("icons/EyeIcon.svg")),
    ("FloppyDiskIcon", include_bytes!("icons/FloppyDiskIcon.svg")),
    ("FolderOpenIcon", include_bytes!("icons/FolderOpenIcon.svg")),
    (
        "LoaderCircleIcon",
        include_bytes!("icons/LoaderCircleIcon.svg"),
    ),
    ("MarkdownMark", include_bytes!("icons/MarkdownMark.svg")),
    ("PlayIcon", include_bytes!("icons/PlayIcon.svg")),
    ("TrashIcon", include_bytes!("icons/TrashIcon.svg")),
    ("google-drive", include_bytes!("icons/google-drive.svg")),
    ("linear-icon", include_bytes!("icons/linear-icon.svg")),
    ("notion-icon", include_bytes!("icons/notion-icon.svg")),
    ("slack-icon", include_bytes!("icons/slack-icon.svg")),
];

pub fn load(path: &str) -> Option<&'static [u8]> {
    let name = path.strip_prefix("automations/")?.strip_suffix(".svg")?;
    ICONS
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, bytes)| *bytes)
}

pub fn path(name: &str) -> String {
    format!("automations/{name}.svg")
}
