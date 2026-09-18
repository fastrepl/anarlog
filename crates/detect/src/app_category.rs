//! Apps whose microphone use is not a meeting: the categories the detect
//! plugin ignores by default, shared with the GPUI shell's settings.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppCategory {
    Anarlog,
    Dictation,
    IDE,
    ScreenRecording,
    AIAssistant,
    Other,
}

impl AppCategory {
    pub fn bundle_ids(&self) -> &'static [&'static str] {
        match self {
            Self::Anarlog => &[
                "com.hyprnote.dev",
                "com.hyprnote.stable",
                "com.hyprnote.nightly",
                "com.hyprnote.staging",
            ],
            Self::Dictation => &[
                "com.electron.wispr-flow",
                "com.seewillow.WillowMac",
                "com.superduper.superwhisper",
                "com.prakashjoshipax.VoiceInk",
                "com.goodsnooze.macwhisper",
                "com.descript.beachcube",
                "com.apple.VoiceMemos",
                "com.electron.aqua-voice",
            ],
            Self::IDE => &[
                "dev.warp.Warp-Stable",
                "com.exafunction.windsurf",
                "com.microsoft.VSCode",
                "com.todesktop.230313mzl4w4u92",
            ],
            Self::ScreenRecording => &[
                "so.cap.desktop",
                "so.cap.desktop.dev",
                "com.timpler.screenstudio",
                "com.loom.desktop",
                "com.obsproject.obs-studio",
                "pl.maketheweb.cleanshotx",
                "com.getcleanshot.app-setapp",
                "com.wulkano.kap",
                "com.wulkano.kap.helper",
                "net.telestream.screenflow10",
                "com.techsmith.camtasia",
                "com.techsmith.camtasia2024",
                "com.TechSmith.Snagit",
                "com.TechSmith.Snagit2024",
                "com.apple.QuickTimePlayerX",
                "com.apple.screenshot.launcher",
            ],
            Self::AIAssistant => &[
                "com.openai.chat",
                "com.openai.codex",
                "com.anthropic.claudefordesktop",
            ],
            Self::Other => &[
                "com.raycast.macos",
                "com.apple.garageband10",
                "com.apple.Sound-Settings.extension",
            ],
        }
    }

    pub fn all() -> &'static [AppCategory] {
        &[
            Self::Anarlog,
            Self::Dictation,
            Self::IDE,
            Self::ScreenRecording,
            Self::AIAssistant,
            Self::Other,
        ]
    }

    pub fn find_category(bundle_id: &str) -> Option<AppCategory> {
        for category in Self::all() {
            if category.bundle_ids().contains(&bundle_id) {
                return Some(*category);
            }
        }
        None
    }
}

pub fn default_ignored_bundle_ids() -> Vec<String> {
    AppCategory::all()
        .iter()
        .flat_map(|cat| cat.bundle_ids().iter().map(|s| s.to_string()))
        .collect()
}
