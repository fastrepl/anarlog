//! Meeting surfaces' SVGs, served to GPUI under `meeting/<Name>.svg`. Hugeicons come from
//! `export-icons.mjs`.

const ICONS: &[(&str, &[u8])] = &[
    (
        "AlertCircleIcon",
        include_bytes!("icons/AlertCircleIcon.svg"),
    ),
    (
        "ArrowDown02Icon",
        include_bytes!("icons/ArrowDown02Icon.svg"),
    ),
    (
        "ArrowExpandIcon",
        include_bytes!("icons/ArrowExpandIcon.svg"),
    ),
    (
        "ArrowShrinkIcon",
        include_bytes!("icons/ArrowShrinkIcon.svg"),
    ),
    ("ArrowUp02Icon", include_bytes!("icons/ArrowUp02Icon.svg")),
    (
        "AudioWaveformIcon",
        include_bytes!("icons/AudioWaveformIcon.svg"),
    ),
    ("PauseIcon", include_bytes!("icons/PauseIcon.svg")),
    ("PlayIcon", include_bytes!("icons/PlayIcon.svg")),
    ("RefreshIcon", include_bytes!("icons/RefreshIcon.svg")),
    ("SquareIcon", include_bytes!("icons/SquareIcon.svg")),
];

pub fn load(path: &str) -> Option<&'static [u8]> {
    let name = path.strip_prefix("meeting/")?.strip_suffix(".svg")?;
    ICONS
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, bytes)| *bytes)
}

pub fn paths() -> impl Iterator<Item = String> {
    ICONS.iter().map(|(name, _)| format!("meeting/{name}.svg"))
}
