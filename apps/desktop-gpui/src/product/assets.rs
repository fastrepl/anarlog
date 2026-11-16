macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        const ICONS: &[(&str, &[u8])] = &[$((
            concat!("product/", $name, ".svg"),
            include_bytes!(concat!("icons/", $name, ".svg")),
        )),*];
    };
}

// Regenerate Hugeicons with `node apps/desktop-gpui/src/product/export-icons.mjs`. The brand
// logos copy `createBrandIcon` paths from packages/ui icons.tsx.
icons!(
    "ArrowDown01Icon",
    "ArrowRight01Icon",
    "ArrowRight02Icon",
    "AudioWaveformIcon",
    "BellIcon",
    "BookOpen01Icon",
    "Building03Icon",
    "CancelCircleIcon",
    "ChartLineData01Icon",
    "CheckIcon",
    "CheckIconBold",
    "CheckmarkCircle02Icon",
    "CloudOffIcon",
    "CodeIcon",
    "CreditCardIcon",
    "Cursor01Icon",
    "DiscordLogo",
    "Download01Icon",
    "ElectricPlugsIcon",
    "GithubLogo",
    "LoaderCircleIcon",
    "LockPasswordIcon",
    "Mic01Icon",
    "PencilEdit01Icon",
    "PlusIcon",
    "RefreshIcon",
    "Settings02Icon",
    "ShieldCheckIcon",
    "SparkleIcon",
    "Sun01Icon",
    "UserGroupIcon",
    "UserIcon",
    "Video01Icon",
    "VolumeHighIcon",
    "VolumeXIcon",
    "XIcon",
    "XLogo",
);

const IMAGES: &[(&str, &[u8])] = &[
    // One frame of apps/desktop/public/assets/onboarding-video.mp4 with shipping's two masked
    // backdrop blurs baked in (GPUI has neither video nor backdrop filters).
    (
        "product/OnboardingBackground.png",
        include_bytes!("icons/OnboardingBackground.png"),
    ),
    // apps/desktop/public/assets/google-meet.svg as Chromium rasterizes it, glow filter
    // included, at 128px.
    (
        "product/GoogleMeet.png",
        include_bytes!("icons/GoogleMeet.png"),
    ),
];

pub fn load(path: &str) -> Option<&'static [u8]> {
    ICONS
        .iter()
        .chain(IMAGES)
        .find(|(name, _)| *name == path)
        .map(|(_, bytes)| *bytes)
}

pub fn paths() -> impl Iterator<Item = &'static str> {
    ICONS.iter().chain(IMAGES).map(|(name, _)| *name)
}
