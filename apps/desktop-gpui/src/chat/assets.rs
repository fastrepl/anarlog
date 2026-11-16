//! Hugeicons used by chat, generated like `scripts/export-icons.mjs` and checked in.

pub const ICONS: &[(&str, &[u8])] = &[
    (
        "AlertCircleIcon",
        include_bytes!("icons/AlertCircleIcon.svg"),
    ),
    (
        "ArrowExpandIcon",
        include_bytes!("icons/ArrowExpandIcon.svg"),
    ),
    (
        "ArrowRight01Icon",
        include_bytes!("icons/ArrowRight01Icon.svg"),
    ),
    (
        "ArrowShrinkIcon",
        include_bytes!("icons/ArrowShrinkIcon.svg"),
    ),
    (
        "ArrowTurnForwardIcon",
        include_bytes!("icons/ArrowTurnForwardIcon.svg"),
    ),
    ("ArrowUp02Icon", include_bytes!("icons/ArrowUp02Icon.svg")),
    ("BrainIcon", include_bytes!("icons/BrainIcon.svg")),
    ("CheckIcon", include_bytes!("icons/CheckIcon.svg")),
    ("CopyIcon", include_bytes!("icons/CopyIcon.svg")),
    ("Download01Icon", include_bytes!("icons/Download01Icon.svg")),
    ("Folder01Icon", include_bytes!("icons/Folder01Icon.svg")),
    (
        "ExternalLinkIcon",
        include_bytes!("icons/ExternalLinkIcon.svg"),
    ),
    ("HistoryIcon", include_bytes!("icons/HistoryIcon.svg")),
    ("LightbulbIcon", include_bytes!("icons/LightbulbIcon.svg")),
    ("ListChecksIcon", include_bytes!("icons/ListChecksIcon.svg")),
    (
        "LoaderCircleIcon",
        include_bytes!("icons/LoaderCircleIcon.svg"),
    ),
    ("Mail01Icon", include_bytes!("icons/Mail01Icon.svg")),
    (
        "MessageCircleIcon",
        include_bytes!("icons/MessageCircleIcon.svg"),
    ),
    ("Mic01Icon", include_bytes!("icons/Mic01Icon.svg")),
    (
        "PictureInPictureIcon",
        include_bytes!("icons/PictureInPictureIcon.svg"),
    ),
    ("PlusIcon", include_bytes!("icons/PlusIcon.svg")),
    ("RotateCcwIcon", include_bytes!("icons/RotateCcwIcon.svg")),
    ("SparkleIcon", include_bytes!("icons/SparkleIcon.svg")),
    ("SquareIcon", include_bytes!("icons/SquareIcon.svg")),
    ("TrashIcon", include_bytes!("icons/TrashIcon.svg")),
    ("XIcon", include_bytes!("icons/XIcon.svg")),
];

pub fn load(path: &str) -> Option<&'static [u8]> {
    let name = path.strip_prefix("chat/")?.strip_suffix(".svg")?;
    ICONS
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, bytes)| *bytes)
}
