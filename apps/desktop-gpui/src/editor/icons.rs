//! Editor and open-note SVGs, served to GPUI under `editor/<Name>.svg`. Hugeicons come from
//! `export-icons.mjs`; `TaskCheck` and `TranscriptAudioIcon` are drawn by hand.

const ICONS: &[(&str, &[u8])] = &[
    (
        "AlertCircleIcon",
        include_bytes!("icons/AlertCircleIcon.svg"),
    ),
    (
        "ArrowDown01Icon",
        include_bytes!("icons/ArrowDown01Icon.svg"),
    ),
    (
        "ArrowDown02Icon",
        include_bytes!("icons/ArrowDown02Icon.svg"),
    ),
    ("ArrowUp02Icon", include_bytes!("icons/ArrowUp02Icon.svg")),
    (
        "CheckmarkCircle02Icon",
        include_bytes!("icons/CheckmarkCircle02Icon.svg"),
    ),
    ("HeartIcon", include_bytes!("icons/HeartIcon.svg")),
    (
        "PencilEdit01Icon",
        include_bytes!("icons/PencilEdit01Icon.svg"),
    ),
    ("PlusIcon", include_bytes!("icons/PlusIcon.svg")),
    ("RefreshIcon", include_bytes!("icons/RefreshIcon.svg")),
    (
        "RotateClockwiseIcon",
        include_bytes!("icons/RotateClockwiseIcon.svg"),
    ),
    ("Search01Icon", include_bytes!("icons/Search01Icon.svg")),
    ("XIcon", include_bytes!("icons/XIcon.svg")),
    ("AppWindowIcon", include_bytes!("icons/AppWindowIcon.svg")),
    (
        "ArrowRight01Icon",
        include_bytes!("icons/ArrowRight01Icon.svg"),
    ),
    (
        "AudioWaveformIcon",
        include_bytes!("icons/AudioWaveformIcon.svg"),
    ),
    ("Building03Icon", include_bytes!("icons/Building03Icon.svg")),
    ("Calendar04Icon", include_bytes!("icons/Calendar04Icon.svg")),
    ("CheckIcon", include_bytes!("icons/CheckIcon.svg")),
    ("CodeIcon", include_bytes!("icons/CodeIcon.svg")),
    (
        "FileDownloadIcon",
        include_bytes!("icons/FileDownloadIcon.svg"),
    ),
    ("FileTextIcon", include_bytes!("icons/FileTextIcon.svg")),
    ("Heading01Icon", include_bytes!("icons/Heading01Icon.svg")),
    ("Heading02Icon", include_bytes!("icons/Heading02Icon.svg")),
    ("Heading03Icon", include_bytes!("icons/Heading03Icon.svg")),
    (
        "HighlighterIcon",
        include_bytes!("icons/HighlighterIcon.svg"),
    ),
    ("HistoryIcon", include_bytes!("icons/HistoryIcon.svg")),
    (
        "LeftToRightListBulletIcon",
        include_bytes!("icons/LeftToRightListBulletIcon.svg"),
    ),
    (
        "LeftToRightListNumberIcon",
        include_bytes!("icons/LeftToRightListNumberIcon.svg"),
    ),
    ("ListChecksIcon", include_bytes!("icons/ListChecksIcon.svg")),
    (
        "LockKeyholeOpenIcon",
        include_bytes!("icons/LockKeyholeOpenIcon.svg"),
    ),
    (
        "LockPasswordIcon",
        include_bytes!("icons/LockPasswordIcon.svg"),
    ),
    ("Mic01Icon", include_bytes!("icons/Mic01Icon.svg")),
    ("MicOff01Icon", include_bytes!("icons/MicOff01Icon.svg")),
    ("MinusIcon", include_bytes!("icons/MinusIcon.svg")),
    (
        "MoreHorizontalIcon",
        include_bytes!("icons/MoreHorizontalIcon.svg"),
    ),
    ("NoteIcon", include_bytes!("icons/NoteIcon.svg")),
    ("PinIcon", include_bytes!("icons/PinIcon.svg")),
    ("QuotesIcon", include_bytes!("icons/QuotesIcon.svg")),
    ("Share08Icon", include_bytes!("icons/Share08Icon.svg")),
    ("SparkleIcon", include_bytes!("icons/SparkleIcon.svg")),
    ("SquareIcon", include_bytes!("icons/SquareIcon.svg")),
    ("TaskCheck", include_bytes!("icons/TaskCheck.svg")),
    (
        "TextAlignLeftIcon",
        include_bytes!("icons/TextAlignLeftIcon.svg"),
    ),
    ("TextBoldIcon", include_bytes!("icons/TextBoldIcon.svg")),
    ("TextIcon", include_bytes!("icons/TextIcon.svg")),
    ("TextItalicIcon", include_bytes!("icons/TextItalicIcon.svg")),
    (
        "TextStrikethroughIcon",
        include_bytes!("icons/TextStrikethroughIcon.svg"),
    ),
    (
        "TextUnderlineIcon",
        include_bytes!("icons/TextUnderlineIcon.svg"),
    ),
    (
        "TranscriptAudioIcon",
        include_bytes!("icons/TranscriptAudioIcon.svg"),
    ),
    ("TrashIcon", include_bytes!("icons/TrashIcon.svg")),
    ("UserIcon", include_bytes!("icons/UserIcon.svg")),
    ("ArrowUp01Icon", include_bytes!("icons/ArrowUp01Icon.svg")),
    ("Exchange01Icon", include_bytes!("icons/Exchange01Icon.svg")),
    ("File01Icon", include_bytes!("icons/File01Icon.svg")),
    ("RepeatIcon", include_bytes!("icons/RepeatIcon.svg")),
    ("TextFontIcon", include_bytes!("icons/TextFontIcon.svg")),
    ("TextSquareIcon", include_bytes!("icons/TextSquareIcon.svg")),
];

pub fn load(path: &str) -> Option<&'static [u8]> {
    let name = path.strip_prefix("editor/")?.strip_suffix(".svg")?;
    ICONS
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, bytes)| *bytes)
}

pub fn paths() -> impl Iterator<Item = String> {
    ICONS.iter().map(|(name, _)| format!("editor/{name}.svg"))
}
