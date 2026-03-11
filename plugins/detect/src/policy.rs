use std::collections::{BTreeSet, HashSet};

use hypr_notification_interface::NotificationKey;

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
    Hyprnote,
    Dictation,
    IDE,
    ScreenRecording,
    AIAssistant,
    Other,
}

impl AppCategory {
    pub fn bundle_ids(&self) -> &'static [&'static str] {
        match self {
            Self::Hyprnote => &[
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
                "com.timpler.screenstudio",
                "com.loom.desktop",
                "com.obsproject.obs-studio",
            ],
            Self::AIAssistant => &["com.openai.chat", "com.anthropic.claudefordesktop"],
            Self::Other => &[
                "com.raycast.macos",
                "com.apple.garageband10",
                "com.apple.Sound-Settings.extension",
            ],
        }
    }

    pub fn all() -> &'static [AppCategory] {
        &[
            Self::Hyprnote,
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
    pub apps: &'a [hypr_detect::InstalledApp],
    pub is_dnd: bool,
    pub event_type: MicEventType,
}

#[derive(Debug)]
pub struct PolicyResult {
    pub filtered_apps: Vec<hypr_detect::InstalledApp>,
    pub dedup_key: String,
}

pub struct MicNotificationPolicy {
    pub respect_dnd: bool,
    pub ignored_categories: Vec<AppCategory>,
    pub user_ignored_bundle_ids: HashSet<String>,
}

impl MicNotificationPolicy {
    pub fn should_track_app(&self, app_id: &str) -> bool {
        AppCategory::find_category(app_id).is_none()
            && !self.user_ignored_bundle_ids.contains(app_id)
    }

    fn filter_apps(
        &self,
        apps: &[hypr_detect::InstalledApp],
        is_dnd: bool,
    ) -> Result<Vec<hypr_detect::InstalledApp>, SkipReason> {
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
                !self.user_ignored_bundle_ids.contains(&app.id)
                    && !ignored_from_categories.contains(app.id.as_str())
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str) -> hypr_detect::InstalledApp {
        hypr_detect::InstalledApp {
            id: id.to_string(),
            name: id.to_string(),
        }
    }

    // --- AppCategory tests ---

    #[test]
    fn test_app_category_find() {
        assert_eq!(
            AppCategory::find_category("com.hyprnote.dev"),
            Some(AppCategory::Hyprnote)
        );
        assert_eq!(AppCategory::find_category("com.zoom.us"), None);
    }

    #[test]
    fn test_app_category_find_all_categories() {
        assert_eq!(
            AppCategory::find_category("com.electron.aqua-voice"),
            Some(AppCategory::Dictation)
        );
        assert_eq!(
            AppCategory::find_category("com.microsoft.VSCode"),
            Some(AppCategory::IDE)
        );
        assert_eq!(
            AppCategory::find_category("so.cap.desktop"),
            Some(AppCategory::ScreenRecording)
        );
        assert_eq!(
            AppCategory::find_category("com.openai.chat"),
            Some(AppCategory::AIAssistant)
        );
        assert_eq!(
            AppCategory::find_category("com.raycast.macos"),
            Some(AppCategory::Other)
        );
    }

    #[test]
    fn test_app_category_all_returns_every_variant() {
        let all = AppCategory::all();
        assert!(all.contains(&AppCategory::Hyprnote));
        assert!(all.contains(&AppCategory::Dictation));
        assert!(all.contains(&AppCategory::IDE));
        assert!(all.contains(&AppCategory::ScreenRecording));
        assert!(all.contains(&AppCategory::AIAssistant));
        assert!(all.contains(&AppCategory::Other));
        assert_eq!(all.len(), 6);
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
    }

    // --- default_ignored_bundle_ids tests ---

    #[test]
    fn test_default_ignored_bundle_ids_covers_all_categories() {
        let ignored = default_ignored_bundle_ids();
        for category in AppCategory::all() {
            for &bundle_id in category.bundle_ids() {
                assert!(
                    ignored.contains(&bundle_id.to_string()),
                    "{bundle_id} from {category:?} should be in default ignored list"
                );
            }
        }
    }

    #[test]
    fn test_default_ignored_bundle_ids_no_duplicates() {
        let ignored = default_ignored_bundle_ids();
        let deduped: HashSet<_> = ignored.iter().collect();
        assert_eq!(ignored.len(), deduped.len(), "no duplicate bundle IDs");
    }

    // --- should_track_app tests ---

    #[test]
    fn test_should_track_unknown_app() {
        let policy = MicNotificationPolicy::default();
        assert!(policy.should_track_app("us.zoom.xos"));
    }

    #[test]
    fn test_should_not_track_categorized_app() {
        let policy = MicNotificationPolicy::default();
        assert!(!policy.should_track_app("com.hyprnote.dev"));
        assert!(!policy.should_track_app("com.electron.aqua-voice"));
        assert!(!policy.should_track_app("com.microsoft.VSCode"));
    }

    #[test]
    fn test_should_not_track_user_ignored_app() {
        let policy = MicNotificationPolicy {
            user_ignored_bundle_ids: HashSet::from(["us.zoom.xos".to_string()]),
            ..Default::default()
        };
        assert!(!policy.should_track_app("us.zoom.xos"));
    }

    #[test]
    fn test_user_ignored_does_not_affect_other_apps() {
        let policy = MicNotificationPolicy {
            user_ignored_bundle_ids: HashSet::from(["us.zoom.xos".to_string()]),
            ..Default::default()
        };
        assert!(policy.should_track_app("com.tinyspeck.slackmacgap"));
    }

    // --- evaluate / filter_apps tests (through public evaluate) ---

    #[test]
    fn test_evaluate_passes_unknown_app() {
        let policy = MicNotificationPolicy::default();
        let apps = vec![app("us.zoom.xos")];
        let ctx = PolicyContext {
            apps: &apps,
            is_dnd: false,
            event_type: MicEventType::Started,
        };
        let result = policy.evaluate(&ctx).unwrap();
        assert_eq!(result.filtered_apps.len(), 1);
        assert_eq!(result.filtered_apps[0].id, "us.zoom.xos");
    }

    #[test]
    fn test_evaluate_filters_all_categorized_apps() {
        let policy = MicNotificationPolicy::default();
        let apps = vec![app("com.hyprnote.dev"), app("com.electron.aqua-voice")];
        let ctx = PolicyContext {
            apps: &apps,
            is_dnd: false,
            event_type: MicEventType::Started,
        };
        let result = policy.evaluate(&ctx);
        assert_eq!(result.unwrap_err(), SkipReason::AllAppsFiltered);
    }

    #[test]
    fn test_evaluate_filters_user_ignored_apps() {
        let policy = MicNotificationPolicy {
            user_ignored_bundle_ids: HashSet::from(["us.zoom.xos".to_string()]),
            ..Default::default()
        };
        let apps = vec![app("us.zoom.xos")];
        let ctx = PolicyContext {
            apps: &apps,
            is_dnd: false,
            event_type: MicEventType::Started,
        };
        assert_eq!(
            policy.evaluate(&ctx).unwrap_err(),
            SkipReason::AllAppsFiltered
        );
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
    fn test_evaluate_dnd_respected_skips() {
        let policy = MicNotificationPolicy {
            respect_dnd: true,
            ..Default::default()
        };
        let apps = vec![app("us.zoom.xos")];
        let ctx = PolicyContext {
            apps: &apps,
            is_dnd: true,
            event_type: MicEventType::Started,
        };
        assert_eq!(policy.evaluate(&ctx).unwrap_err(), SkipReason::DoNotDisturb);
    }

    #[test]
    fn test_evaluate_dnd_not_respected_passes() {
        let policy = MicNotificationPolicy {
            respect_dnd: false,
            ..Default::default()
        };
        let apps = vec![app("us.zoom.xos")];
        let ctx = PolicyContext {
            apps: &apps,
            is_dnd: true,
            event_type: MicEventType::Started,
        };
        let result = policy.evaluate(&ctx).unwrap();
        assert_eq!(result.filtered_apps.len(), 1);
    }

    #[test]
    fn test_evaluate_dnd_respected_but_not_active_passes() {
        let policy = MicNotificationPolicy {
            respect_dnd: true,
            ..Default::default()
        };
        let apps = vec![app("us.zoom.xos")];
        let ctx = PolicyContext {
            apps: &apps,
            is_dnd: false,
            event_type: MicEventType::Started,
        };
        let result = policy.evaluate(&ctx).unwrap();
        assert_eq!(result.filtered_apps.len(), 1);
    }

    #[test]
    fn test_evaluate_empty_apps_list() {
        let policy = MicNotificationPolicy::default();
        let apps: Vec<hypr_detect::InstalledApp> = vec![];
        let ctx = PolicyContext {
            apps: &apps,
            is_dnd: false,
            event_type: MicEventType::Started,
        };
        assert_eq!(
            policy.evaluate(&ctx).unwrap_err(),
            SkipReason::AllAppsFiltered
        );
    }

    #[test]
    fn test_evaluate_started_vs_stopped_produce_different_dedup_keys() {
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
    }

    #[test]
    fn test_evaluate_same_apps_same_dedup_key() {
        let policy = MicNotificationPolicy::default();
        let apps = vec![app("us.zoom.xos")];
        let ctx = PolicyContext {
            apps: &apps,
            is_dnd: false,
            event_type: MicEventType::Started,
        };

        let key1 = policy.evaluate(&ctx).unwrap().dedup_key;
        let key2 = policy.evaluate(&ctx).unwrap().dedup_key;
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_notification_key_dedup() {
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
    fn test_policy_with_no_ignored_categories_passes_all() {
        let policy = MicNotificationPolicy {
            ignored_categories: vec![],
            ..Default::default()
        };
        let apps = vec![app("com.hyprnote.dev"), app("us.zoom.xos")];
        let ctx = PolicyContext {
            apps: &apps,
            is_dnd: false,
            event_type: MicEventType::Started,
        };
        let result = policy.evaluate(&ctx).unwrap();
        assert_eq!(result.filtered_apps.len(), 2);
    }

    #[test]
    fn test_policy_with_selective_ignored_categories() {
        let policy = MicNotificationPolicy {
            ignored_categories: vec![AppCategory::Dictation],
            ..Default::default()
        };
        let apps = vec![
            app("com.electron.aqua-voice"),
            app("com.hyprnote.dev"),
            app("us.zoom.xos"),
        ];
        let ctx = PolicyContext {
            apps: &apps,
            is_dnd: false,
            event_type: MicEventType::Started,
        };
        let result = policy.evaluate(&ctx).unwrap();
        let ids: Vec<_> = result.filtered_apps.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(ids, vec!["com.hyprnote.dev", "us.zoom.xos"]);
    }
}
