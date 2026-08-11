use crate::Permission;

/// How `open()` should guide the user to System Settings.
///
/// **Assisted** panes are list-based Privacy settings where the user must add
/// or enable the app manually (Accessibility, Screen Recording, Input
/// Monitoring, Screen & System Audio Recording). A drag-to-list overlay helps.
///
/// **Native** permissions have normal system prompts (calendar, contacts,
/// microphone, etc.) and should stay on native request flows unless the user
/// already denied and needs a recovery deep link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsGuidance {
    Native {
        anchor: &'static str,
    },
    Assisted {
        anchor: &'static str,
        pane_title: &'static str,
    },
}

impl SettingsGuidance {
    pub const fn anchor(self) -> &'static str {
        match self {
            Self::Native { anchor } | Self::Assisted { anchor, .. } => anchor,
        }
    }

    pub const fn is_assisted(self) -> bool {
        matches!(self, Self::Assisted { .. })
    }

    pub const fn pane_title(self) -> Option<&'static str> {
        match self {
            Self::Native { .. } => None,
            Self::Assisted { pane_title, .. } => Some(pane_title),
        }
    }
}

/// Serializable projection of [`SettingsGuidance`] so the renderer can explain
/// how a permission is granted without duplicating this table in TypeScript.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PermissionGuidance {
    /// `true` when `open()` shows the assisted drag overlay for a Privacy pane.
    pub assisted: bool,
    /// The Privacy pane title for assisted permissions; `None` for native ones.
    pub pane_title: Option<String>,
}

impl From<SettingsGuidance> for PermissionGuidance {
    fn from(value: SettingsGuidance) -> Self {
        Self {
            assisted: value.is_assisted(),
            pane_title: value.pane_title().map(str::to_owned),
        }
    }
}

impl Permission {
    pub const fn settings_guidance(self) -> SettingsGuidance {
        match self {
            Self::Calendar => SettingsGuidance::Native {
                anchor: "Privacy_Calendars",
            },
            Self::Reminders => SettingsGuidance::Native {
                anchor: "Privacy_Reminders",
            },
            Self::Contacts => SettingsGuidance::Native {
                anchor: "Privacy_Contacts",
            },
            Self::Microphone => SettingsGuidance::Native {
                anchor: "Privacy_Microphone",
            },
            Self::SystemAudio => SettingsGuidance::Native {
                anchor: "Privacy_ScreenCapture",
            },
            Self::ScreenRecording => SettingsGuidance::Native {
                anchor: "Privacy_ScreenCapture",
            },
            Self::Accessibility => SettingsGuidance::Assisted {
                anchor: "Privacy_Accessibility",
                pane_title: "Accessibility",
            },
            Self::InputMonitoring => SettingsGuidance::Native {
                anchor: "Privacy_ListenEvent",
            },
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn privacy_settings_deep_link_urls(anchor: &str) -> [String; 2] {
    [
        format!("x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?{anchor}"),
        format!("x-apple.systempreferences:com.apple.preference.security?{anchor}"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessibility_is_the_only_assisted_pane() {
        for permission in [
            Permission::Calendar,
            Permission::Reminders,
            Permission::Contacts,
            Permission::Microphone,
            Permission::SystemAudio,
            Permission::ScreenRecording,
            Permission::InputMonitoring,
        ] {
            assert!(!permission.settings_guidance().is_assisted());
        }

        assert!(Permission::Accessibility.settings_guidance().is_assisted());
    }

    #[test]
    fn assisted_guidance_carries_a_pane_title() {
        let guidance = PermissionGuidance::from(Permission::Accessibility.settings_guidance());
        assert!(guidance.assisted);
        assert_eq!(guidance.pane_title.as_deref(), Some("Accessibility"));

        let guidance = PermissionGuidance::from(Permission::Microphone.settings_guidance());
        assert!(!guidance.assisted);
        assert_eq!(guidance.pane_title, None);
    }
}
