//! Icon table for `icons/catalog`, whose SVGs `export-catalog-icons.mjs` exports.

/// Stored template/folder icon keys and their glyphs, in shipping's picker order.
pub const TEMPLATE_ICONS: &[(&str, &str)] = &[
    ("activity", "Pulse01Icon"),
    ("bar-chart", "ChartBarBigIcon"),
    ("bell", "BellIcon"),
    ("book-open", "BookOpen01Icon"),
    ("brain", "BrainIcon"),
    ("briefcase", "Briefcase01Icon"),
    ("bug", "Bug01Icon"),
    ("building", "Building03Icon"),
    ("calendar", "Calendar03Icon"),
    ("camera", "Camera01Icon"),
    ("chart", "ChartLineData01Icon"),
    ("alert", "AlertCircleIcon"),
    ("clipboard-check", "Task01Icon"),
    ("clock", "Clock01Icon"),
    ("cloud", "CloudIcon"),
    ("code", "CodeIcon"),
    ("coffee", "Coffee01Icon"),
    ("compass", "CompassIcon"),
    ("credit-card", "CreditCardIcon"),
    ("crown", "CrownIcon"),
    ("database", "DatabaseIcon"),
    ("dollar", "DollarSignIcon"),
    ("file-text", "FileTextIcon"),
    ("flag", "Flag01Icon"),
    ("flame", "FireIcon"),
    ("folder", "Folder01Icon"),
    ("globe", "GlobeIcon"),
    ("graduation", "GraduationCapIcon"),
    ("hammer", "HammerIcon"),
    ("handshake", "HandshakeIcon"),
    ("headphones", "HeadphonesIcon"),
    ("heart", "HeartIcon"),
    ("home", "House01Icon"),
    ("image", "Image01Icon"),
    ("key", "Key01Icon"),
    ("landmark", "BankIcon"),
    ("leaf", "Leaf01Icon"),
    ("lightbulb", "LightbulbIcon"),
    ("link", "LinkIcon"),
    ("list-checks", "ListChecksIcon"),
    ("lock", "LockPasswordIcon"),
    ("mail", "Mail01Icon"),
    ("map", "MapIcon"),
    ("megaphone", "Megaphone01Icon"),
    ("message", "MessageCircleIcon"),
    ("messages", "MessageMultiple01Icon"),
    ("mic", "Mic01Icon"),
    ("milestone", "SignpostIcon"),
    ("moon", "MoonIcon"),
    ("music", "MusicNote01Icon"),
    ("notebook-tabs", "NotebookIcon"),
    ("package", "PackageIcon"),
    ("palette", "PaletteIcon"),
    ("paperclip", "PaperclipIcon"),
    ("pen", "PencilLineIcon"),
    ("phone", "PhoneIcon"),
    ("pie-chart", "PieChartIcon"),
    ("plane", "Airplane01Icon"),
    ("presentation", "Presentation01Icon"),
    ("puzzle", "PuzzleIcon"),
    ("rocket", "RocketIcon"),
    ("scale", "BalanceScaleIcon"),
    ("search", "Search01Icon"),
    ("settings", "Settings02Icon"),
    ("shield", "ShieldCheckIcon"),
    ("shopping", "ShoppingBag01Icon"),
    ("sparkles", "SparkleIcon"),
    ("kanban", "KanbanIcon"),
    ("star", "StarIcon"),
    ("stethoscope", "StethoscopeIcon"),
    ("sun", "Sun01Icon"),
    ("target", "Target01Icon"),
    ("trending", "TrendingUpIcon"),
    ("trophy", "TrophyIcon"),
    ("user-search", "UserSearch01Icon"),
    ("users", "UsersIcon"),
    ("video", "Video01Icon"),
    ("wrench", "Wrench01Icon"),
    ("zap", "ZapIcon"),
];

const ICONS: &[(&str, &[u8])] = &[
    (
        "Airplane01Icon",
        include_bytes!("icons/catalog/Airplane01Icon.svg"),
    ),
    (
        "AlertCircleIcon",
        include_bytes!("icons/catalog/AlertCircleIcon.svg"),
    ),
    (
        "ArrowLeft02Icon",
        include_bytes!("icons/catalog/ArrowLeft02Icon.svg"),
    ),
    (
        "ArrowUpDownIcon",
        include_bytes!("icons/catalog/ArrowUpDownIcon.svg"),
    ),
    (
        "BalanceScaleIcon",
        include_bytes!("icons/catalog/BalanceScaleIcon.svg"),
    ),
    ("BankIcon", include_bytes!("icons/catalog/BankIcon.svg")),
    ("BellIcon", include_bytes!("icons/catalog/BellIcon.svg")),
    (
        "BookOpen01Icon",
        include_bytes!("icons/catalog/BookOpen01Icon.svg"),
    ),
    (
        "BookOpenTextIcon",
        include_bytes!("icons/catalog/BookOpenTextIcon.svg"),
    ),
    ("BrainIcon", include_bytes!("icons/catalog/BrainIcon.svg")),
    (
        "Briefcase01Icon",
        include_bytes!("icons/catalog/Briefcase01Icon.svg"),
    ),
    ("Bug01Icon", include_bytes!("icons/catalog/Bug01Icon.svg")),
    (
        "Building03Icon",
        include_bytes!("icons/catalog/Building03Icon.svg"),
    ),
    (
        "Calendar03Icon",
        include_bytes!("icons/catalog/Calendar03Icon.svg"),
    ),
    (
        "Camera01Icon",
        include_bytes!("icons/catalog/Camera01Icon.svg"),
    ),
    (
        "ChartBarBigIcon",
        include_bytes!("icons/catalog/ChartBarBigIcon.svg"),
    ),
    (
        "ChartLineData01Icon",
        include_bytes!("icons/catalog/ChartLineData01Icon.svg"),
    ),
    ("CheckIcon", include_bytes!("icons/catalog/CheckIcon.svg")),
    // Hand-drawn: shipping's conic-gradient custom color swatch.
    ("ColorWheel", include_bytes!("icons/catalog/ColorWheel.svg")),
    (
        "CheckIconBold",
        include_bytes!("icons/catalog/CheckIconBold.svg"),
    ),
    (
        "Clock01Icon",
        include_bytes!("icons/catalog/Clock01Icon.svg"),
    ),
    ("CloudIcon", include_bytes!("icons/catalog/CloudIcon.svg")),
    ("CodeIcon", include_bytes!("icons/catalog/CodeIcon.svg")),
    (
        "Coffee01Icon",
        include_bytes!("icons/catalog/Coffee01Icon.svg"),
    ),
    (
        "CompassIcon",
        include_bytes!("icons/catalog/CompassIcon.svg"),
    ),
    (
        "CreditCardIcon",
        include_bytes!("icons/catalog/CreditCardIcon.svg"),
    ),
    ("CrownIcon", include_bytes!("icons/catalog/CrownIcon.svg")),
    (
        "DatabaseIcon",
        include_bytes!("icons/catalog/DatabaseIcon.svg"),
    ),
    (
        "DollarSignIcon",
        include_bytes!("icons/catalog/DollarSignIcon.svg"),
    ),
    ("File01Icon", include_bytes!("icons/catalog/File01Icon.svg")),
    (
        "FileTextIcon",
        include_bytes!("icons/catalog/FileTextIcon.svg"),
    ),
    ("FireIcon", include_bytes!("icons/catalog/FireIcon.svg")),
    ("Flag01Icon", include_bytes!("icons/catalog/Flag01Icon.svg")),
    (
        "Folder01Icon",
        include_bytes!("icons/catalog/Folder01Icon.svg"),
    ),
    ("GlobeIcon", include_bytes!("icons/catalog/GlobeIcon.svg")),
    (
        "GraduationCapIcon",
        include_bytes!("icons/catalog/GraduationCapIcon.svg"),
    ),
    (
        "GripVerticalIcon",
        include_bytes!("icons/catalog/GripVerticalIcon.svg"),
    ),
    ("HammerIcon", include_bytes!("icons/catalog/HammerIcon.svg")),
    (
        "HandshakeIcon",
        include_bytes!("icons/catalog/HandshakeIcon.svg"),
    ),
    (
        "HeadphonesIcon",
        include_bytes!("icons/catalog/HeadphonesIcon.svg"),
    ),
    ("HeartIcon", include_bytes!("icons/catalog/HeartIcon.svg")),
    (
        "HeartIconBold",
        include_bytes!("icons/catalog/HeartIconBold.svg"),
    ),
    (
        "House01Icon",
        include_bytes!("icons/catalog/House01Icon.svg"),
    ),
    (
        "Image01Icon",
        include_bytes!("icons/catalog/Image01Icon.svg"),
    ),
    ("KanbanIcon", include_bytes!("icons/catalog/KanbanIcon.svg")),
    ("Key01Icon", include_bytes!("icons/catalog/Key01Icon.svg")),
    ("Leaf01Icon", include_bytes!("icons/catalog/Leaf01Icon.svg")),
    (
        "LightbulbIcon",
        include_bytes!("icons/catalog/LightbulbIcon.svg"),
    ),
    ("LinkIcon", include_bytes!("icons/catalog/LinkIcon.svg")),
    (
        "ListChecksIcon",
        include_bytes!("icons/catalog/ListChecksIcon.svg"),
    ),
    (
        "LockPasswordIcon",
        include_bytes!("icons/catalog/LockPasswordIcon.svg"),
    ),
    (
        "MagicWand01Icon",
        include_bytes!("icons/catalog/MagicWand01Icon.svg"),
    ),
    ("Mail01Icon", include_bytes!("icons/catalog/Mail01Icon.svg")),
    ("MapIcon", include_bytes!("icons/catalog/MapIcon.svg")),
    (
        "Megaphone01Icon",
        include_bytes!("icons/catalog/Megaphone01Icon.svg"),
    ),
    (
        "MessageCircleIcon",
        include_bytes!("icons/catalog/MessageCircleIcon.svg"),
    ),
    (
        "MessageMultiple01Icon",
        include_bytes!("icons/catalog/MessageMultiple01Icon.svg"),
    ),
    ("Mic01Icon", include_bytes!("icons/catalog/Mic01Icon.svg")),
    ("MoonIcon", include_bytes!("icons/catalog/MoonIcon.svg")),
    (
        "MoreHorizontalIcon",
        include_bytes!("icons/catalog/MoreHorizontalIcon.svg"),
    ),
    (
        "MusicNote01Icon",
        include_bytes!("icons/catalog/MusicNote01Icon.svg"),
    ),
    (
        "NotebookIcon",
        include_bytes!("icons/catalog/NotebookIcon.svg"),
    ),
    (
        "PackageIcon",
        include_bytes!("icons/catalog/PackageIcon.svg"),
    ),
    (
        "PaletteIcon",
        include_bytes!("icons/catalog/PaletteIcon.svg"),
    ),
    (
        "PaperclipIcon",
        include_bytes!("icons/catalog/PaperclipIcon.svg"),
    ),
    (
        "PencilLineIcon",
        include_bytes!("icons/catalog/PencilLineIcon.svg"),
    ),
    ("PhoneIcon", include_bytes!("icons/catalog/PhoneIcon.svg")),
    (
        "PieChartIcon",
        include_bytes!("icons/catalog/PieChartIcon.svg"),
    ),
    ("PlusIcon", include_bytes!("icons/catalog/PlusIcon.svg")),
    (
        "Presentation01Icon",
        include_bytes!("icons/catalog/Presentation01Icon.svg"),
    ),
    (
        "Pulse01Icon",
        include_bytes!("icons/catalog/Pulse01Icon.svg"),
    ),
    ("PuzzleIcon", include_bytes!("icons/catalog/PuzzleIcon.svg")),
    ("RocketIcon", include_bytes!("icons/catalog/RocketIcon.svg")),
    (
        "Search01Icon",
        include_bytes!("icons/catalog/Search01Icon.svg"),
    ),
    (
        "Settings02Icon",
        include_bytes!("icons/catalog/Settings02Icon.svg"),
    ),
    (
        "Share08Icon",
        include_bytes!("icons/catalog/Share08Icon.svg"),
    ),
    (
        "ShieldCheckIcon",
        include_bytes!("icons/catalog/ShieldCheckIcon.svg"),
    ),
    (
        "ShoppingBag01Icon",
        include_bytes!("icons/catalog/ShoppingBag01Icon.svg"),
    ),
    (
        "SignpostIcon",
        include_bytes!("icons/catalog/SignpostIcon.svg"),
    ),
    (
        "SparkleIcon",
        include_bytes!("icons/catalog/SparkleIcon.svg"),
    ),
    ("StarIcon", include_bytes!("icons/catalog/StarIcon.svg")),
    (
        "StethoscopeIcon",
        include_bytes!("icons/catalog/StethoscopeIcon.svg"),
    ),
    ("Sun01Icon", include_bytes!("icons/catalog/Sun01Icon.svg")),
    (
        "Target01Icon",
        include_bytes!("icons/catalog/Target01Icon.svg"),
    ),
    ("Task01Icon", include_bytes!("icons/catalog/Task01Icon.svg")),
    (
        "TrendingUpIcon",
        include_bytes!("icons/catalog/TrendingUpIcon.svg"),
    ),
    ("TrophyIcon", include_bytes!("icons/catalog/TrophyIcon.svg")),
    (
        "UserSearch01Icon",
        include_bytes!("icons/catalog/UserSearch01Icon.svg"),
    ),
    ("UsersIcon", include_bytes!("icons/catalog/UsersIcon.svg")),
    (
        "Video01Icon",
        include_bytes!("icons/catalog/Video01Icon.svg"),
    ),
    (
        "Wrench01Icon",
        include_bytes!("icons/catalog/Wrench01Icon.svg"),
    ),
    ("XIcon", include_bytes!("icons/catalog/XIcon.svg")),
    ("ZapIcon", include_bytes!("icons/catalog/ZapIcon.svg")),
];

/// Asset path of an icon, e.g. `catalog/PlusIcon.svg`.
pub fn path(name: &str) -> gpui::SharedString {
    format!("catalog/{name}.svg").into()
}

pub fn load(path: &str) -> Option<&'static [u8]> {
    let name = path.strip_prefix("catalog/")?.strip_suffix(".svg")?;
    ICONS
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, bytes)| *bytes)
}

pub fn paths() -> impl Iterator<Item = String> {
    ICONS.iter().map(|(name, _)| format!("catalog/{name}.svg"))
}

/// The emoji picker's table: `category\tid\tnative\tname\tsearch` per line.
pub const EMOJI: &str = include_str!("icons/catalog/emoji.tsv");
