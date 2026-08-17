use serde::{Deserialize, Serialize};

use crate::{BotState, CaptureProviderKind, MeetingPlatform, MeetingReference};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureWorkerCheckpoint {
    pub job_id: String,
    pub bot_id: String,
    pub provider: CaptureProviderKind,
    pub meeting: MeetingReference,
    pub state: BotState,
    pub next_sequence: u64,
}

impl CaptureWorkerCheckpoint {
    pub fn validate(&self) -> Result<(), CaptureWorkerCheckpointError> {
        validate_identifier(&self.job_id)
            .map_err(|()| CaptureWorkerCheckpointError::InvalidJobId)?;
        validate_identifier(&self.bot_id)
            .map_err(|()| CaptureWorkerCheckpointError::InvalidBotId)?;
        if self.meeting.url.is_empty()
            || self.meeting.url.len() > 8192
            || self.meeting.url.chars().any(char::is_control)
        {
            return Err(CaptureWorkerCheckpointError::InvalidMeetingUrl);
        }
        if self.state == BotState::Queued && self.next_sequence != 0
            || self.state != BotState::Queued && self.next_sequence == 0
        {
            return Err(CaptureWorkerCheckpointError::InvalidSequence {
                state: self.state,
                next_sequence: self.next_sequence,
            });
        }
        if let Some(expected) = self.provider.native_platform()
            && self.meeting.platform != expected
        {
            return Err(CaptureWorkerCheckpointError::ProviderPlatformMismatch {
                provider: self.provider,
                expected,
                actual: self.meeting.platform,
            });
        }
        Ok(())
    }
}

fn validate_identifier(value: &str) -> Result<(), ()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
    {
        return Err(());
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CaptureWorkerCheckpointError {
    #[error("capture job ID contains unsupported characters")]
    InvalidJobId,
    #[error("capture bot ID contains unsupported characters")]
    InvalidBotId,
    #[error("capture meeting URL must contain 1-8192 non-control characters")]
    InvalidMeetingUrl,
    #[error(
        "capture checkpoint state {state:?} is inconsistent with next sequence {next_sequence}"
    )]
    InvalidSequence { state: BotState, next_sequence: u64 },
    #[error("capture provider {provider:?} requires {expected:?}, but the job targets {actual:?}")]
    ProviderPlatformMismatch {
        provider: CaptureProviderKind,
        expected: MeetingPlatform,
        actual: MeetingPlatform,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkpoint(
        provider: CaptureProviderKind,
        platform: MeetingPlatform,
    ) -> CaptureWorkerCheckpoint {
        CaptureWorkerCheckpoint {
            job_id: "job-1".into(),
            bot_id: "bot-1".into(),
            provider,
            meeting: MeetingReference {
                platform,
                url: "https://example.com/meeting".into(),
                external_id: None,
                calendar_event_id: None,
            },
            state: BotState::Queued,
            next_sequence: 0,
        }
    }

    #[test]
    fn pins_vendor_native_providers_to_their_platform() {
        for (provider, platform) in [
            (CaptureProviderKind::ZoomRtms, MeetingPlatform::Zoom),
            (
                CaptureProviderKind::MicrosoftGraph,
                MeetingPlatform::MicrosoftTeams,
            ),
            (
                CaptureProviderKind::WebexMeetingsSdk,
                MeetingPlatform::Webex,
            ),
        ] {
            checkpoint(provider, platform).validate().unwrap();
        }

        assert!(matches!(
            checkpoint(CaptureProviderKind::ZoomRtms, MeetingPlatform::GoogleMeet).validate(),
            Err(CaptureWorkerCheckpointError::ProviderPlatformMismatch { .. })
        ));
    }

    #[test]
    fn keeps_owned_and_external_aggregators_platform_neutral() {
        for provider in [CaptureProviderKind::Anarlog, CaptureProviderKind::Recall] {
            for platform in [
                MeetingPlatform::GoogleMeet,
                MeetingPlatform::Zoom,
                MeetingPlatform::MicrosoftTeams,
                MeetingPlatform::Webex,
                MeetingPlatform::Jitsi,
            ] {
                checkpoint(provider, platform).validate().unwrap();
            }
        }
    }

    #[test]
    fn rejects_inconsistent_resume_sequences() {
        let mut checkpoint = checkpoint(CaptureProviderKind::ZoomRtms, MeetingPlatform::Zoom);
        checkpoint.state = BotState::Capturing;
        assert!(matches!(
            checkpoint.validate(),
            Err(CaptureWorkerCheckpointError::InvalidSequence { .. })
        ));
    }
}
