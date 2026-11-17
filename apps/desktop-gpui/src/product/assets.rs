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
    "AlertCircleIcon",
    "ArrowDown01Icon",
    "ArrowRight01Icon",
    "ArrowRight02Icon",
    "AudioWaveformIcon",
    "BellIcon",
    "BookOpen01Icon",
    "Building03Icon",
    "Camera01Icon",
    "CancelCircleIcon",
    "ChartLineData01Icon",
    "CheckIcon",
    "CheckIconBold",
    "CheckmarkCircle02Icon",
    "CloudOffIcon",
    "CodeIcon",
    "CopyIcon",
    "CreditCardIcon",
    "Cursor01Icon",
    "DiscordLogo",
    "Download01Icon",
    "ElectricPlugsIcon",
    "ExternalLinkIcon",
    "Key01Icon",
    "GitMergeIcon",
    "GitPullRequestIcon",
    "GithubLogo",
    "LoaderCircleIcon",
    "LockPasswordIcon",
    "Message01Icon",
    "Mic01Icon",
    "MoreHorizontalIcon",
    "PencilEdit01Icon",
    "Plug01Icon",
    "PlusIcon",
    "RadioButtonIcon",
    "RefreshIcon",
    "Settings02Icon",
    "SentIcon",
    "ShieldCheckIcon",
    "SparkleIcon",
    "Sun01Icon",
    "TrashIcon",
    "TriangleAlertIcon",
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
    // The OpenCode mark shipping inlines in `settings/developers/skills.tsx`.
    (
        "product/OpenCodeLight.svg",
        include_bytes!("icons/OpenCodeLight.svg"),
    ),
    (
        "product/OpenCodeDark.svg",
        include_bytes!("icons/OpenCodeDark.svg"),
    ),
    (
        "product/AnarlogIcon.png",
        include_bytes!("../../../desktop/public/assets/anarlog-icon.png"),
    ),
];

macro_rules! app_icons {
    ($($name:literal),* $(,)?) => {
        /// Shipping's `public/assets/app-icons` previews for the App icon picker.
        const APP_ICONS: &[(&str, &[u8])] = &[$((
            concat!("product/app-icons/", $name, ".png"),
            include_bytes!(concat!("../../../desktop/public/assets/app-icons/", $name, ".png")),
        )),*];
    };
}

app_icons!(
    "anagram-dark",
    "anagram-light",
    "dev-dark",
    "dev-light",
    "journal",
    "notepad",
    "stable-dark",
    "stable-light",
    "staging-dark",
    "staging-light",
    "stone",
    "typewriter-key",
    "walnut",
);

macro_rules! provider_logos {
    ($($name:literal),* $(,)?) => {
        /// Lobe logos exported by `export-provider-icons.mjs`.
        const PROVIDER_LOGOS: &[(&str, &[u8])] = &[$((
            concat!("product/providers/", $name, ".svg"),
            include_bytes!(concat!("icons/providers/", $name, ".svg")),
        )),*];
    };
}

provider_logos!(
    "alibaba_cloud",
    "amazon_bedrock",
    "anthropic",
    "apple_foundation",
    "apple_speech",
    "assemblyai",
    "aws_transcribe",
    "azure_ai",
    "azure_openai",
    "azure_speech",
    "cerebras",
    "chatgpt",
    "claude",
    "cloudflare_workers_ai",
    "cohere",
    "cursor",
    "dashscope",
    "deepseek",
    "elevenlabs",
    "fireworks",
    "github_copilot",
    "google_cloud",
    "google_generative_ai",
    "google_vertex_ai",
    "grok",
    "groq",
    "kimi_code",
    "lmstudio",
    "mistral",
    "moonshot",
    "nvidia",
    "ollama",
    "openai",
    "openrouter",
    "siliconflow",
    "together",
    "xai",
    "zai",
);

macro_rules! brand_images {
    ($(($id:literal, $file:literal)),* $(,)?) => {
        /// Shipping's `ProviderBrandImage` sources from `public/assets`, by provider id.
        pub const BRAND_IMAGES: &[(&str, &str, &[u8])] = &[$((
            $id,
            concat!("product/brands/", $file),
            include_bytes!(concat!("../../../desktop/public/assets/", $file)),
        )),*];
    };
}

brand_images!(
    ("deepgram", "deepgram-mark.svg"),
    ("gradium", "gradium-icon.svg"),
    ("revai", "revai-mark.svg"),
    ("cartesia", "cartesia-mark.svg"),
    ("gladia", "gladia-mark.svg"),
    ("smallestai", "smallestai-mark.svg"),
    ("speechmatics", "speechmatics-mark.svg"),
    ("alebex", "alebex-icon.svg"),
    ("meta", "model-icons/meta-logo.svg"),
    ("unsloth", "unsloth-mark.png"),
    ("venice", "venice.png"),
    ("inworld", "inworld-icon.png"),
    ("modulate", "modulate-icon.png"),
    ("nari", "nari-icon.png"),
    ("wisprflow", "wispr-flow-icon.png"),
    ("soniox", "soniox-black.png"),
    ("pyannote", "pyannote-logo-black.png"),
    ("aquavoice", "aquavoice-black.png"),
    ("anarlog", "anarlog-icon.png"),
    ("hubspot", "hubspot.svg"),
    ("attio", "attio.svg"),
    ("salesforce", "salesforce.svg"),
    ("pipedrive", "pipedrive.svg"),
    ("close", "close.svg"),
    ("github", "github-icon.svg"),
    ("apple-reminders", "apple-reminders.png"),
);

pub fn load(path: &str) -> Option<&'static [u8]> {
    ICONS
        .iter()
        .chain(IMAGES)
        .chain(APP_ICONS)
        .chain(PROVIDER_LOGOS)
        .find(|(name, _)| *name == path)
        .map(|(_, bytes)| *bytes)
        .or_else(|| {
            BRAND_IMAGES
                .iter()
                .find(|(_, name, _)| *name == path)
                .map(|(_, _, bytes)| *bytes)
        })
        .or_else(|| super::sharing::assets::load(path))
}

pub fn paths() -> impl Iterator<Item = &'static str> {
    ICONS
        .iter()
        .chain(IMAGES)
        .chain(APP_ICONS)
        .chain(PROVIDER_LOGOS)
        .map(|(name, _)| *name)
        .chain(BRAND_IMAGES.iter().map(|(_, name, _)| *name))
        .chain(super::sharing::assets::paths())
}
