//! Shipping's detect plugin processor (`plugins/detect/src/{policy,handler,mic_usage_tracker}.rs`):
//! an app counts as a meeting once it has used the microphone for the detection delay, at most
//! once per app every ten minutes, unless it is excluded or Do Not Disturb pauses alerts.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use super::apps::MicApp;

const COOLDOWN: Duration = Duration::from_secs(10 * 60);
const DEFAULT_THRESHOLD_SECS: u64 = 15;

/// Apps never treated as meetings unless the user includes them (`AppCategory::all()`).
pub const DEFAULT_IGNORED: &[&str] = &[
    // Anarlog
    "com.hyprnote.dev",
    "com.hyprnote.stable",
    "com.hyprnote.nightly",
    "com.hyprnote.staging",
    // Dictation
    "com.electron.wispr-flow",
    "com.seewillow.WillowMac",
    "com.superduper.superwhisper",
    "com.prakashjoshipax.VoiceInk",
    "com.goodsnooze.macwhisper",
    "com.descript.beachcube",
    "com.apple.VoiceMemos",
    "com.electron.aqua-voice",
    // IDE
    "dev.warp.Warp-Stable",
    "com.exafunction.windsurf",
    "com.microsoft.VSCode",
    "com.todesktop.230313mzl4w4u92",
    // Screen recording
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
    // AI assistants
    "com.openai.chat",
    "com.openai.codex",
    "com.anthropic.claudefordesktop",
    // Other
    "com.raycast.macos",
    "com.apple.garageband10",
    "com.apple.Sound-Settings.extension",
];

/// The Notifications settings detection reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Policy {
    /// `notification_detect`: whether microphone activity is watched at all.
    pub enabled: bool,
    pub respect_dnd: bool,
    pub ignored: HashSet<String>,
    pub included: HashSet<String>,
    pub threshold: Duration,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            enabled: true,
            respect_dnd: false,
            ignored: HashSet::new(),
            included: HashSet::new(),
            threshold: Duration::from_secs(DEFAULT_THRESHOLD_SECS),
        }
    }
}

impl Policy {
    pub fn tracks(&self, app_id: &str) -> bool {
        !self.ignored.contains(app_id)
            && (self.included.contains(app_id) || !DEFAULT_IGNORED.contains(&app_id))
    }
}

#[derive(Default)]
pub struct Detection {
    timers: HashMap<String, (Instant, MicApp)>,
    cooldowns: HashMap<String, Instant>,
}

impl Detection {
    /// Starts the detection delay for newly active apps that are watched and not cooling down.
    pub fn mic_started(&mut self, apps: &[MicApp], policy: &Policy, now: Instant) {
        if !policy.enabled {
            return;
        }
        self.cooldowns
            .retain(|_, fired| now.saturating_duration_since(*fired) < COOLDOWN);
        for app in apps {
            if policy.tracks(&app.id)
                && !self.timers.contains_key(&app.id)
                && !self.cooldowns.contains_key(&app.id)
            {
                self.timers
                    .insert(app.id.clone(), (now + policy.threshold, app.clone()));
            }
        }
    }

    pub fn mic_stopped(&mut self, apps: &[MicApp]) {
        for app in apps {
            self.timers.remove(&app.id);
        }
    }

    pub fn next_deadline(&self) -> Option<Instant> {
        self.timers.values().map(|(deadline, _)| *deadline).min()
    }

    /// Apps whose delay elapsed. Each starts its cooldown even when the policy then
    /// suppresses the alert, as shipping's claim does.
    pub fn due(
        &mut self,
        now: Instant,
        policy: &Policy,
        do_not_disturb: impl Fn() -> bool,
    ) -> Vec<MicApp> {
        let due: Vec<String> = self
            .timers
            .iter()
            .filter(|(_, (deadline, _))| *deadline <= now)
            .map(|(id, _)| id.clone())
            .collect();
        let mut detected = Vec::new();
        for id in due {
            let Some((_, app)) = self.timers.remove(&id) else {
                continue;
            };
            self.cooldowns.insert(id, now);
            if policy.enabled && !(policy.respect_dnd && do_not_disturb()) {
                detected.push(app);
            }
        }
        detected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zoom() -> MicApp {
        MicApp::new("us.zoom.xos", "zoom.us")
    }

    #[test]
    fn detection_waits_for_the_delay_and_then_cools_down_for_ten_minutes() {
        let policy = Policy {
            threshold: Duration::from_secs(5),
            ..Policy::default()
        };
        let start = Instant::now();
        let mut detection = Detection::default();
        detection.mic_started(&[zoom()], &policy, start);
        assert!(
            detection
                .due(start + Duration::from_secs(4), &policy, || false)
                .is_empty()
        );
        assert_eq!(
            detection.due(start + Duration::from_secs(5), &policy, || false),
            vec![zoom()]
        );
        detection.mic_started(&[zoom()], &policy, start + Duration::from_secs(60));
        assert_eq!(detection.next_deadline(), None);
        let later = start + Duration::from_secs(5) + COOLDOWN;
        detection.mic_started(&[zoom()], &policy, later);
        assert_eq!(
            detection.next_deadline(),
            Some(later + Duration::from_secs(5))
        );
        detection.mic_stopped(&[zoom()]);
        assert_eq!(detection.next_deadline(), None);
    }

    #[test]
    fn excluded_apps_and_do_not_disturb_suppress_detection() {
        let start = Instant::now();
        let vscode = MicApp::new("com.microsoft.VSCode", "Code");
        let mut policy = Policy {
            ignored: HashSet::from(["us.zoom.xos".to_owned()]),
            ..Policy::default()
        };
        let mut detection = Detection::default();
        detection.mic_started(&[zoom(), vscode.clone()], &policy, start);
        assert_eq!(detection.next_deadline(), None);
        policy.included.insert(vscode.id.clone());
        policy.respect_dnd = true;
        detection.mic_started(std::slice::from_ref(&vscode), &policy, start);
        let due = start + policy.threshold;
        assert!(detection.due(due, &policy, || true).is_empty());
        // The suppressed alert still started the cooldown.
        detection.mic_started(&[vscode], &policy, due);
        assert_eq!(detection.next_deadline(), None);
    }
}
