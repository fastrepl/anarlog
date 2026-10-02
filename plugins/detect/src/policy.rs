use std::collections::{BTreeSet, HashSet};

use anlg_notification_interface::NotificationKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicEventType {
    Started,
    Stopped,
}

// We intentionally don't include the "already listening" reason here; that filtering should be done by the consumer side.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    DoNotDisturb,
    AllAppsFiltered,
}

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

pub struct PolicyContext<'a> {
    pub apps: &'a [anlg_detect::InstalledApp],
    pub is_dnd: bool,
    pub event_type: MicEventType,
}

#[derive(Debug)]
pub struct PolicyResult {
    pub filtered_apps: Vec<anlg_detect::InstalledApp>,
    pub dedup_key: String,
}

pub struct MicNotificationPolicy {
    pub respect_dnd: bool,
    pub ignored_categories: Vec<AppCategory>,
    pub user_ignored_bundle_ids: HashSet<String>,
    pub user_included_bundle_ids: HashSet<String>,
}

impl MicNotificationPolicy {
    pub fn should_track_app(&self, app_id: &str) -> bool {
        if self.user_ignored_bundle_ids.contains(app_id) {
            return false;
        }

        self.user_included_bundle_ids.contains(app_id)
            || AppCategory::find_category(app_id).is_none()
    }

    fn filter_apps(
        &self,
        apps: &[anlg_detect::InstalledApp],
        is_dnd: bool,
    ) -> Result<Vec<anlg_detect::InstalledApp>, SkipReason> {
        if self.respect_dnd && is_dnd {
            return Err(SkipReason::DoNotDisturb);
        }

        let ignored_from_categories: BTreeSet<&str> = self
            .ignored_categories
            .iter()
            .flat_map(|cat| cat.bundle_ids().iter().copied())
            .collect();

        let filtered_apps: Vec<_> = apps
            .iter()
            .filter(|app| {
                if self.user_ignored_bundle_ids.contains(&app.id) {
                    return false;
                }

                self.user_included_bundle_ids.contains(&app.id)
                    || !ignored_from_categories.contains(app.id.as_str())
            })
            .cloned()
            .collect();

        if filtered_apps.is_empty() {
            return Err(SkipReason::AllAppsFiltered);
        }

        Ok(filtered_apps)
    }

    pub fn evaluate(&self, ctx: &PolicyContext) -> Result<PolicyResult, SkipReason> {
        let filtered_apps = self.filter_apps(ctx.apps, ctx.is_dnd)?;

        let notification_key = match ctx.event_type {
            MicEventType::Started => {
                NotificationKey::mic_started(filtered_apps.iter().map(|a| a.id.clone()))
            }
            MicEventType::Stopped => {
                NotificationKey::mic_stopped(filtered_apps.iter().map(|a| a.id.clone()))
            }
        };

        Ok(PolicyResult {
            filtered_apps,
            dedup_key: notification_key.to_dedup_key(),
        })
    }
}

impl Default for MicNotificationPolicy {
    fn default() -> Self {
        Self {
            respect_dnd: false,
            ignored_categories: AppCategory::all().to_vec(),
            user_ignored_bundle_ids: HashSet::new(),
            user_included_bundle_ids: HashSet::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str) -> anlg_detect::InstalledApp {
        anlg_detect::InstalledApp {
            id: id.to_string(),
            name: id.to_string(),
        }
    }

    #[test]
    fn test_every_bundle_id_resolves_to_its_category() {
        for category in AppCategory::all() {
            for &bundle_id in category.bundle_ids() {
                assert_eq!(
                    AppCategory::find_category(bundle_id),
                    Some(*category),
                    "{bundle_id} should resolve to {category:?}"
                );
            }
        }
        assert_eq!(AppCategory::find_category("com.zoom.us"), None);
    }

    #[test]
    fn test_default_ignored_bundle_ids_cover_all_categories_once() {
        let ignored = default_ignored_bundle_ids();
        for category in AppCategory::all() {
            for &bundle_id in category.bundle_ids() {
                assert!(
                    ignored.contains(&bundle_id.to_string()),
                    "{bundle_id} from {category:?} should be in default ignored list"
                );
            }
        }
        let deduped: HashSet<_> = ignored.iter().collect();
        assert_eq!(ignored.len(), deduped.len(), "no duplicate bundle IDs");
    }

    #[test]
    fn test_should_track_app_applies_categories_then_user_overrides() {
        let cases: [(HashSet<String>, HashSet<String>, &[&str], bool); 6] = [
            (HashSet::new(), HashSet::new(), &["us.zoom.xos"], true),
            (
                HashSet::new(),
                HashSet::new(),
                &[
                    "com.hyprnote.dev",
                    "com.electron.aqua-voice",
                    "com.microsoft.VSCode",
                ],
                false,
            ),
            (
                HashSet::from(["us.zoom.xos".to_string()]),
                HashSet::new(),
                &["us.zoom.xos"],
                false,
            ),
            (
                HashSet::from(["us.zoom.xos".to_string()]),
                HashSet::new(),
                &["com.tinyspeck.slackmacgap"],
                true,
            ),
            (
                HashSet::new(),
                HashSet::from(["com.microsoft.VSCode".to_string()]),
                &["com.microsoft.VSCode"],
                true,
            ),
            (
                HashSet::from(["com.microsoft.VSCode".to_string()]),
                HashSet::from(["com.microsoft.VSCode".to_string()]),
                &["com.microsoft.VSCode"],
                false,
            ),
        ];

        for (ignored, included, apps, expected) in cases {
            let policy = MicNotificationPolicy {
                user_ignored_bundle_ids: ignored,
                user_included_bundle_ids: included,
                ..Default::default()
            };
            for app in apps {
                assert_eq!(
                    policy.should_track_app(app),
                    expected,
                    "unexpected tracking decision for {app}"
                );
            }
        }
    }

    #[test]
    fn test_evaluate_mixed_apps_keeps_unknown_only() {
        let policy = MicNotificationPolicy::default();
        let apps = vec![
            app("us.zoom.xos"),
            app("com.electron.aqua-voice"),
            app("com.tinyspeck.slackmacgap"),
        ];
        let ctx = PolicyContext {
            apps: &apps,
            is_dnd: false,
            event_type: MicEventType::Started,
        };
        let result = policy.evaluate(&ctx).unwrap();
        let ids: Vec<_> = result.filtered_apps.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["us.zoom.xos", "com.tinyspeck.slackmacgap"]);
    }

    #[test]
    fn test_evaluate_rejects_when_no_trackable_apps_remain() {
        let policy = MicNotificationPolicy::default();
        for apps in [
            vec![app("com.hyprnote.dev"), app("com.electron.aqua-voice")],
            Vec::new(),
        ] {
            let ctx = PolicyContext {
                apps: &apps,
                is_dnd: false,
                event_type: MicEventType::Started,
            };
            assert_eq!(
                policy.evaluate(&ctx).unwrap_err(),
                SkipReason::AllAppsFiltered,
                "expected all apps to be filtered from {apps:?}"
            );
        }
    }

    #[test]
    fn test_evaluate_applies_user_overrides() {
        let ignored_policy = MicNotificationPolicy {
            user_ignored_bundle_ids: HashSet::from(["us.zoom.xos".to_string()]),
            ..Default::default()
        };
        let ignored_apps = vec![app("us.zoom.xos")];
        let ignored_ctx = PolicyContext {
            apps: &ignored_apps,
            is_dnd: false,
            event_type: MicEventType::Started,
        };
        assert_eq!(
            ignored_policy.evaluate(&ignored_ctx).unwrap_err(),
            SkipReason::AllAppsFiltered
        );

        let included_policy = MicNotificationPolicy {
            user_included_bundle_ids: HashSet::from(["com.microsoft.VSCode".to_string()]),
            ..Default::default()
        };
        let included_apps = vec![app("com.microsoft.VSCode")];
        let included_ctx = PolicyContext {
            apps: &included_apps,
            is_dnd: false,
            event_type: MicEventType::Started,
        };
        let result = included_policy.evaluate(&included_ctx).unwrap();
        assert_eq!(result.filtered_apps.len(), 1);
        assert_eq!(result.filtered_apps[0].id, "com.microsoft.VSCode");
    }

    #[test]
    fn test_evaluate_dnd_only_skips_when_respected_and_active() {
        for (respect_dnd, is_dnd, expected_skip) in [
            (true, true, true),
            (false, true, false),
            (true, false, false),
        ] {
            let policy = MicNotificationPolicy {
                respect_dnd,
                ..Default::default()
            };
            let apps = vec![app("us.zoom.xos")];
            let ctx = PolicyContext {
                apps: &apps,
                is_dnd,
                event_type: MicEventType::Started,
            };
            let result = policy.evaluate(&ctx);
            if expected_skip {
                assert_eq!(
                    result.unwrap_err(),
                    SkipReason::DoNotDisturb,
                    "expected skip for respect_dnd={respect_dnd}, is_dnd={is_dnd}"
                );
            } else {
                assert_eq!(
                    result.unwrap().filtered_apps.len(),
                    1,
                    "expected app to pass for respect_dnd={respect_dnd}, is_dnd={is_dnd}"
                );
            }
        }
    }

    #[test]
    fn test_dedup_key_depends_on_event_type_and_app_set_not_order() {
        let policy = MicNotificationPolicy::default();
        let apps = vec![app("us.zoom.xos")];
        let started_ctx = PolicyContext {
            apps: &apps,
            is_dnd: false,
            event_type: MicEventType::Started,
        };
        let stopped_ctx = PolicyContext {
            apps: &apps,
            is_dnd: false,
            event_type: MicEventType::Stopped,
        };

        let started_key = policy.evaluate(&started_ctx).unwrap().dedup_key;
        let stopped_key = policy.evaluate(&stopped_ctx).unwrap().dedup_key;
        assert_ne!(started_key, stopped_key);
        let key1 = policy.evaluate(&started_ctx).unwrap().dedup_key;
        let key2 = policy.evaluate(&started_ctx).unwrap().dedup_key;
        assert_eq!(key1, key2);

        let key1 = NotificationKey::mic_started(["com.zoom.us".to_string()]);
        let key2 = NotificationKey::mic_started(["com.zoom.us".to_string()]);
        assert_eq!(key1.to_dedup_key(), key2.to_dedup_key());

        let key3 = NotificationKey::mic_started([
            "com.zoom.us".to_string(),
            "com.slack.Slack".to_string(),
        ]);
        let key4 = NotificationKey::mic_started([
            "com.slack.Slack".to_string(),
            "com.zoom.us".to_string(),
        ]);
        assert_eq!(key3.to_dedup_key(), key4.to_dedup_key());
    }

    #[test]
    fn test_ignored_categories_control_category_filtering() {
        let apps = vec![
            app("com.electron.aqua-voice"),
            app("com.hyprnote.dev"),
            app("us.zoom.xos"),
        ];

        let policy = MicNotificationPolicy {
            ignored_categories: vec![],
            ..Default::default()
        };
        let ctx = PolicyContext {
            apps: &apps,
            is_dnd: false,
            event_type: MicEventType::Started,
        };
        assert_eq!(policy.evaluate(&ctx).unwrap().filtered_apps.len(), 3);

        let policy = MicNotificationPolicy {
            ignored_categories: vec![AppCategory::Dictation],
            ..Default::default()
        };
        let result = policy.evaluate(&ctx).unwrap();
        let ids: Vec<_> = result.filtered_apps.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["com.hyprnote.dev", "us.zoom.xos"]);
    }
}
