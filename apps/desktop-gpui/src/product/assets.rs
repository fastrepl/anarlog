macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        const ICONS: &[(&str, &[u8])] = &[$((
            concat!("product/", $name, ".svg"),
            include_bytes!(concat!("icons/", $name, ".svg")),
        )),*];
    };
}

// Regenerate with `node apps/desktop-gpui/src/product/export-icons.mjs`.
icons!(
    "AudioWaveformIcon",
    "BellIcon",
    "BookOpen01Icon",
    "Building03Icon",
    "ChartLineData01Icon",
    "CheckIcon",
    "CodeIcon",
    "CreditCardIcon",
    "Download01Icon",
    "LockPasswordIcon",
    "Mic01Icon",
    "PlusIcon",
    "RefreshIcon",
    "Settings02Icon",
    "ShieldCheckIcon",
    "SparkleIcon",
    "Sun01Icon",
    "UserGroupIcon",
    "UserIcon",
    "Video01Icon",
    "XIcon",
);

pub fn load(path: &str) -> Option<&'static [u8]> {
    ICONS
        .iter()
        .find(|(name, _)| *name == path)
        .map(|(_, bytes)| *bytes)
}

pub fn paths() -> impl Iterator<Item = &'static str> {
    ICONS.iter().map(|(name, _)| *name)
}
