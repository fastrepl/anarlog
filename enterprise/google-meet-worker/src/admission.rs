// Adapted for Anarlog from Vexa v0.12.18. See ../THIRD_PARTY_NOTICES.md.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

pub const DEFAULT_CAPTCHA_GRACE: Duration = Duration::from_secs(120);
pub const ADMISSION_PROBE_EXPRESSION: &str = include_str!("admission_probe.js");

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmissionSnapshot {
    pub waiting_room_visible: bool,
    pub consent_prompt_visible: bool,
    pub explicit_denial_indicator: Option<String>,
    pub ambiguous_error_indicator: Option<String>,
    pub visible_recaptcha_challenge: bool,
    #[serde(default)]
    pub participant_tile_labels: Vec<String>,
    pub self_name_nodes: usize,
    pub visible_admission_controls: usize,
}

impl AdmissionSnapshot {
    pub fn real_participant_tiles(&self) -> usize {
        self.participant_tile_labels
            .iter()
            .filter(|label| {
                let label = label.trim().to_lowercase();
                !label.is_empty()
                    && !label.contains("visual_effects")
                    && !label.contains("backgrounds and effects")
            })
            .count()
    }

    fn has_admission_signal(&self) -> bool {
        self.real_participant_tiles() > 0
            || self.self_name_nodes > 0
            || self.visible_admission_controls > 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionOutcome {
    WaitingForAdmission,
    ConsentRequired,
    CaptchaChallenge { remaining: Duration },
    Admitted,
    Rejected(AdmissionRejection),
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionRejection {
    pub reason: AdmissionRejectionReason,
    pub indicator: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionRejectionReason {
    HostDenied,
    ErrorPage,
    CaptchaUnsolved,
}

#[derive(Debug)]
pub struct AdmissionClassifier {
    captcha_grace: Duration,
    captcha_suppressed_since: Option<Instant>,
}

impl Default for AdmissionClassifier {
    fn default() -> Self {
        Self::new(DEFAULT_CAPTCHA_GRACE)
    }
}

impl AdmissionClassifier {
    pub fn new(captcha_grace: Duration) -> Self {
        Self {
            captcha_grace,
            captcha_suppressed_since: None,
        }
    }

    pub fn classify(&mut self, snapshot: &AdmissionSnapshot, now: Instant) -> AdmissionOutcome {
        if let Some(indicator) = snapshot.explicit_denial_indicator.as_ref() {
            self.captcha_suppressed_since = None;
            return AdmissionOutcome::Rejected(AdmissionRejection {
                reason: AdmissionRejectionReason::HostDenied,
                indicator: indicator.clone(),
            });
        }

        if let Some(indicator) = snapshot.ambiguous_error_indicator.as_ref() {
            if snapshot.visible_recaptcha_challenge {
                let since = *self.captcha_suppressed_since.get_or_insert(now);
                let elapsed = now.saturating_duration_since(since);
                if elapsed < self.captcha_grace {
                    return AdmissionOutcome::CaptchaChallenge {
                        remaining: self.captcha_grace - elapsed,
                    };
                }
                self.captcha_suppressed_since = None;
                return AdmissionOutcome::Rejected(AdmissionRejection {
                    reason: AdmissionRejectionReason::CaptchaUnsolved,
                    indicator: indicator.clone(),
                });
            }

            self.captcha_suppressed_since = None;
            return AdmissionOutcome::Rejected(AdmissionRejection {
                reason: AdmissionRejectionReason::ErrorPage,
                indicator: indicator.clone(),
            });
        }

        self.captcha_suppressed_since = None;
        if snapshot.waiting_room_visible {
            return AdmissionOutcome::WaitingForAdmission;
        }
        if snapshot.consent_prompt_visible {
            return AdmissionOutcome::ConsentRequired;
        }
        if snapshot.has_admission_signal() {
            return AdmissionOutcome::Admitted;
        }
        AdmissionOutcome::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_host_denial_wins_over_captcha_and_stale_waiting_copy() {
        let mut classifier = AdmissionClassifier::default();
        let outcome = classifier.classify(
            &AdmissionSnapshot {
                waiting_room_visible: true,
                explicit_denial_indicator: Some("denied your request".into()),
                ambiguous_error_indicator: Some("Try again".into()),
                visible_recaptcha_challenge: true,
                ..Default::default()
            },
            Instant::now(),
        );

        assert_eq!(
            outcome,
            AdmissionOutcome::Rejected(AdmissionRejection {
                reason: AdmissionRejectionReason::HostDenied,
                indicator: "denied your request".into(),
            })
        );
    }

    #[test]
    fn terminal_video_call_denial_wins_over_captcha_and_stale_waiting_copy() {
        let mut classifier = AdmissionClassifier::default();
        let outcome = classifier.classify(
            &AdmissionSnapshot {
                waiting_room_visible: true,
                explicit_denial_indicator: Some("can't join this video call".into()),
                ambiguous_error_indicator: Some("Try again".into()),
                visible_recaptcha_challenge: true,
                ..Default::default()
            },
            Instant::now(),
        );

        assert_eq!(
            outcome,
            AdmissionOutcome::Rejected(AdmissionRejection {
                reason: AdmissionRejectionReason::HostDenied,
                indicator: "can't join this video call".into(),
            })
        );
        assert!(ADMISSION_PROBE_EXPRESSION.contains("can't join this video call"));
        assert!(ADMISSION_PROBE_EXPRESSION.contains("h1,h2,h3"));
    }

    #[test]
    fn captcha_suppression_expires_instead_of_polling_forever() {
        let grace = Duration::from_secs(120);
        let mut classifier = AdmissionClassifier::new(grace);
        let started = Instant::now();
        let snapshot = AdmissionSnapshot {
            ambiguous_error_indicator: Some("Try again".into()),
            visible_recaptcha_challenge: true,
            ..Default::default()
        };

        assert_eq!(
            classifier.classify(&snapshot, started),
            AdmissionOutcome::CaptchaChallenge { remaining: grace }
        );
        assert_eq!(
            classifier.classify(&snapshot, started + Duration::from_secs(119)),
            AdmissionOutcome::CaptchaChallenge {
                remaining: Duration::from_secs(1)
            }
        );
        assert_eq!(
            classifier.classify(&snapshot, started + grace),
            AdmissionOutcome::Rejected(AdmissionRejection {
                reason: AdmissionRejectionReason::CaptchaUnsolved,
                indicator: "Try again".into(),
            })
        );
    }

    #[test]
    fn waiting_and_consent_guards_suppress_lobby_false_positives() {
        for snapshot in [
            AdmissionSnapshot {
                waiting_room_visible: true,
                visible_admission_controls: 3,
                ..Default::default()
            },
            AdmissionSnapshot {
                consent_prompt_visible: true,
                participant_tile_labels: vec!["Ada".into()],
                ..Default::default()
            },
        ] {
            let mut classifier = AdmissionClassifier::default();
            assert_ne!(
                classifier.classify(&snapshot, Instant::now()),
                AdmissionOutcome::Admitted
            );
        }
    }

    #[test]
    fn effects_preview_is_not_a_real_participant() {
        let snapshot = AdmissionSnapshot {
            participant_tile_labels: vec![
                "visual_effects Backgrounds and effects".into(),
                "Grace Hopper".into(),
            ],
            ..Default::default()
        };

        assert_eq!(snapshot.real_participant_tiles(), 1);
    }
}
