use std::{error::Error, time::Duration};

use anlg_meeting_capture::{
    CaptureEvent, CaptureEventPayload, CaptureProviderKind, MeetingPlatform, TransitionError,
};
use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::mpsc;

use crate::{
    AdmissionMonitor, AdmissionMonitorConfig, AdmissionMonitorError, AudioFrameSink,
    AudioFrameSinkOutput, BrowserCapture, BrowserCaptureError, CaptureJobRuntime,
    ChromiumLaunchConfig, LobbyController, LobbyError, MeetingSession, MeetingSessionLaunchError,
    MeetingSessionShutdownError, RuntimeMonitor, RuntimeMonitorError, WorkerCheckpoint,
    WorkerLifecycle, X11Input, X11InputConfig, X11InputError,
};

#[derive(Debug, Clone)]
pub struct GoogleMeetRuntimeConfig {
    pub chromium: ChromiumLaunchConfig,
    pub x11: X11InputConfig,
    pub bot_name: String,
    pub admission: AdmissionMonitorConfig,
    pub runtime_poll_interval: Duration,
}

pub struct GoogleMeetRuntime<S> {
    chromium: ChromiumLaunchConfig,
    input: X11Input,
    bot_name: String,
    admission: AdmissionMonitor,
    runtime_monitor: RuntimeMonitor,
    audio_sink: S,
    session: Option<MeetingSession>,
    started: bool,
    media: MediaState,
}

// Media lifecycle as one tagged state: retry artifacts (the frozen capture
// duration) exist only in the variant that can use them, so field
// combinations that do not describe a valid lifecycle are unrepresentable.
enum MediaState {
    Idle,
    Open {
        capture: BrowserCapture,
        started_at: tokio::time::Instant,
    },
    // Sink finalization failed retryably; the duration is frozen from the
    // first attempt so retries stay idempotent.
    Finalizing {
        capture_duration: Duration,
    },
    Finalized,
}

impl<S> GoogleMeetRuntime<S> {
    pub fn new(
        config: GoogleMeetRuntimeConfig,
        audio_sink: S,
    ) -> Result<Self, GoogleMeetRuntimeConfigError> {
        validate_bot_name(&config.bot_name)?;
        let input = X11Input::new(config.x11)?;
        let admission = AdmissionMonitor::new(config.admission)?;
        let runtime_monitor = RuntimeMonitor::new(config.runtime_poll_interval)?;
        Ok(Self {
            chromium: config.chromium,
            input,
            bot_name: config.bot_name,
            admission,
            runtime_monitor,
            audio_sink,
            session: None,
            started: false,
            media: MediaState::Idle,
        })
    }
}

#[async_trait]
impl<S> CaptureJobRuntime for GoogleMeetRuntime<S>
where
    S: AudioFrameSink,
{
    type Error = GoogleMeetRuntimeError<S::Error>;

    fn validate_checkpoint(&self, checkpoint: &WorkerCheckpoint) -> Result<(), Self::Error> {
        google_meet_url(checkpoint).map(|_| ())
    }

    async fn run(
        &mut self,
        checkpoint: &WorkerCheckpoint,
        lifecycle: &mut WorkerLifecycle,
        events: mpsc::Sender<CaptureEvent>,
    ) -> Result<(), Self::Error> {
        if self.started {
            return Err(GoogleMeetRuntimeError::AlreadyStarted);
        }
        let meeting_url = google_meet_url(checkpoint)?;
        self.started = true;
        send_event(&events, lifecycle.launch_started(Utc::now())?).await?;

        let session = MeetingSession::launch(self.chromium.clone(), &meeting_url).await?;
        self.session = Some(session);

        {
            let page = self
                .session
                .as_mut()
                .expect("launched runtime owns a session")
                .page_mut();
            LobbyController::new(page, &self.input)
                .join(&self.bot_name, self.chromium.authenticated)
                .await?;
            self.admission
                .wait_streaming(page, lifecycle, &events)
                .await?;
        }
        if lifecycle.state().is_terminal() {
            return Ok(());
        }

        let (capture, _) = BrowserCapture::install(
            self.session
                .as_mut()
                .expect("launched runtime owns a session")
                .page_mut(),
        )
        .await?;
        self.media = MediaState::Open {
            capture,
            started_at: tokio::time::Instant::now(),
        };
        send_event(&events, lifecycle.capture_started(Utc::now())?).await?;

        let mut probes = tokio::time::interval(self.runtime_monitor.poll_interval());
        probes.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let terminal_outcome = loop {
            tokio::select! {
                _ = probes.tick() => {
                    let outcome = self.runtime_monitor
                        .probe_outcome(
                            self.session
                                .as_mut()
                                .expect("launched runtime owns a session")
                                .page_mut(),
                            lifecycle,
                        )
                        .await?;
                    if let Some(outcome) = outcome {
                        break outcome;
                    }
                }
                frame = next_open_frame(&mut self.media) => {
                    let payloads = self.audio_sink
                        .write_frame(frame?)
                        .await
                        .map_err(GoogleMeetRuntimeError::AudioSink)?;
                    send_outputs(&events, lifecycle, payloads).await?;
                }
            }
        };

        let finalization = self.finalize_media().await;
        let payloads = finalization.into_result()?;
        send_outputs(&events, lifecycle, payloads).await?;
        send_event(
            &events,
            lifecycle.apply_runtime_outcome(terminal_outcome, Utc::now())?,
        )
        .await?;
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<Vec<CaptureEventPayload>, Self::Error> {
        let finalization = self.finalize_media().await;
        let session_error = match self.session.take() {
            Some(session) => session.shutdown().await.err(),
            None => None,
        };

        if finalization.capture_error.is_none()
            && finalization.sink_error.is_none()
            && session_error.is_none()
        {
            Ok(finalization.payloads.into_iter().map(Into::into).collect())
        } else {
            Err(GoogleMeetRuntimeError::Cleanup {
                capture_error: finalization.capture_error,
                sink_error: finalization.sink_error,
                session_error,
            })
        }
    }
}

fn google_meet_url<S>(
    checkpoint: &WorkerCheckpoint,
) -> Result<crate::GoogleMeetUrl, GoogleMeetRuntimeError<S>>
where
    S: Error + Send + Sync + 'static,
{
    if checkpoint.provider != CaptureProviderKind::Anarlog
        || checkpoint.meeting.platform != MeetingPlatform::GoogleMeet
    {
        return Err(GoogleMeetRuntimeError::UnsupportedCheckpoint);
    }
    crate::GoogleMeetUrl::parse(&checkpoint.meeting.url)
        .map_err(GoogleMeetRuntimeError::InvalidMeetingUrl)
}

impl<S> GoogleMeetRuntime<S>
where
    S: AudioFrameSink,
{
    async fn finalize_media(&mut self) -> MediaFinalization<S::Error> {
        let (capture_error, trailing_frames, capture_duration) =
            match std::mem::replace(&mut self.media, MediaState::Finalized) {
                state @ (MediaState::Idle | MediaState::Finalized) => {
                    self.media = state;
                    return MediaFinalization {
                        payloads: Vec::new(),
                        capture_error: None,
                        sink_error: None,
                    };
                }
                MediaState::Open {
                    mut capture,
                    started_at,
                } => {
                    let capture_duration = started_at.elapsed();
                    let stopped = match self.session.as_mut() {
                        Some(session) => capture.stop_and_drain(session.page_mut()).await,
                        None => Ok(Vec::new()),
                    };
                    match stopped {
                        Ok(frames) => (None, frames, capture_duration),
                        Err(error) => (Some(error), Vec::new(), capture_duration),
                    }
                }
                MediaState::Finalizing { capture_duration } => (None, Vec::new(), capture_duration),
            };

        let mut payloads = Vec::new();
        let mut sink_error = None;
        for frame in trailing_frames {
            match self.audio_sink.write_frame(frame).await {
                Ok(output) => payloads.extend(output),
                Err(error) => {
                    sink_error = Some(error);
                    break;
                }
            }
        }
        match self.audio_sink.finish(capture_duration).await {
            Ok(output) => payloads.extend(output),
            Err(error) => {
                self.media = MediaState::Finalizing { capture_duration };
                if sink_error.is_none() {
                    sink_error = Some(error);
                }
            }
        }
        MediaFinalization {
            payloads,
            capture_error,
            sink_error,
        }
    }
}

async fn next_open_frame(media: &mut MediaState) -> Result<crate::AudioFrame, BrowserCaptureError> {
    match media {
        MediaState::Open { capture, .. } => capture.next_frame().await,
        _ => unreachable!("the capture loop only runs while media is open"),
    }
}

struct MediaFinalization<E> {
    payloads: Vec<AudioFrameSinkOutput>,
    capture_error: Option<BrowserCaptureError>,
    sink_error: Option<E>,
}

impl<E> MediaFinalization<E>
where
    E: Error + Send + Sync + 'static,
{
    fn into_result(self) -> Result<Vec<AudioFrameSinkOutput>, GoogleMeetRuntimeError<E>> {
        if let Some(error) = self.capture_error {
            return Err(GoogleMeetRuntimeError::Capture(error));
        }
        if let Some(error) = self.sink_error {
            return Err(GoogleMeetRuntimeError::AudioSink(error));
        }
        Ok(self.payloads)
    }
}

async fn send_event<E>(
    events: &mpsc::Sender<CaptureEvent>,
    event: CaptureEvent,
) -> Result<(), GoogleMeetRuntimeError<E>>
where
    E: Error + Send + Sync + 'static,
{
    events
        .send(event)
        .await
        .map_err(|_| GoogleMeetRuntimeError::EventChannelClosed)
}

async fn send_outputs<E>(
    events: &mpsc::Sender<CaptureEvent>,
    lifecycle: &mut WorkerLifecycle,
    outputs: Vec<AudioFrameSinkOutput>,
) -> Result<(), GoogleMeetRuntimeError<E>>
where
    E: Error + Send + Sync + 'static,
{
    for output in outputs {
        send_event(events, lifecycle.emit_payload(output.into(), Utc::now())).await?;
    }
    Ok(())
}

fn validate_bot_name(value: &str) -> Result<(), GoogleMeetRuntimeConfigError> {
    if value.is_empty() || value.chars().count() > 80 || value.chars().any(char::is_control) {
        return Err(GoogleMeetRuntimeConfigError::InvalidBotName);
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum GoogleMeetRuntimeConfigError {
    #[error("bot display name must contain 1-80 non-control characters")]
    InvalidBotName,
    #[error(transparent)]
    X11(#[from] X11InputError),
    #[error(transparent)]
    Admission(#[from] AdmissionMonitorError),
    #[error(transparent)]
    RuntimeMonitor(#[from] RuntimeMonitorError),
}

#[derive(Debug, thiserror::Error)]
pub enum GoogleMeetRuntimeError<E>
where
    E: Error + Send + Sync + 'static,
{
    #[error("Google Meet runtime can only be started once")]
    AlreadyStarted,
    #[error("Google Meet runtime received a checkpoint for another provider or platform")]
    UnsupportedCheckpoint,
    #[error("Google Meet runtime received an invalid meeting URL")]
    InvalidMeetingUrl(#[source] crate::CdpError),
    #[error("capture supervisor stopped accepting runtime events")]
    EventChannelClosed,
    #[error(transparent)]
    Transition(#[from] TransitionError),
    #[error(transparent)]
    Launch(#[from] MeetingSessionLaunchError),
    #[error(transparent)]
    Lobby(#[from] LobbyError),
    #[error(transparent)]
    Admission(#[from] AdmissionMonitorError),
    #[error(transparent)]
    Capture(#[from] BrowserCaptureError),
    #[error(transparent)]
    RuntimeMonitor(#[from] RuntimeMonitorError),
    #[error("audio frame sink failed")]
    AudioSink(#[source] E),
    #[error("Google Meet runtime cleanup failed")]
    Cleanup {
        capture_error: Option<BrowserCaptureError>,
        sink_error: Option<E>,
        session_error: Option<MeetingSessionShutdownError>,
    },
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use anlg_meeting_capture::{
        BotState, CaptureProviderKind, MeetingPlatform, MeetingReference, TranscriptSegment,
    };

    use super::*;

    struct UnusedSink;

    #[derive(Debug, thiserror::Error)]
    #[error("unused sink failed")]
    struct UnusedSinkError;

    struct RetryingFinishSink {
        attempts: usize,
    }

    struct DurationRecordingSink {
        fail_attempts: usize,
        attempts: usize,
        durations: Vec<Duration>,
    }

    #[async_trait]
    impl AudioFrameSink for DurationRecordingSink {
        type Error = UnusedSinkError;

        async fn write_frame(
            &mut self,
            _frame: crate::AudioFrame,
        ) -> Result<Vec<AudioFrameSinkOutput>, Self::Error> {
            Ok(Vec::new())
        }

        async fn finish(
            &mut self,
            capture_duration: Duration,
        ) -> Result<Vec<AudioFrameSinkOutput>, Self::Error> {
            self.attempts += 1;
            self.durations.push(capture_duration);
            if self.attempts <= self.fail_attempts {
                return Err(UnusedSinkError);
            }
            Ok(Vec::new())
        }
    }

    #[async_trait]
    impl AudioFrameSink for UnusedSink {
        type Error = UnusedSinkError;

        async fn write_frame(
            &mut self,
            _frame: crate::AudioFrame,
        ) -> Result<Vec<AudioFrameSinkOutput>, Self::Error> {
            Ok(Vec::new())
        }

        async fn finish(
            &mut self,
            _capture_duration: Duration,
        ) -> Result<Vec<AudioFrameSinkOutput>, Self::Error> {
            Ok(Vec::new())
        }
    }

    #[async_trait]
    impl AudioFrameSink for RetryingFinishSink {
        type Error = UnusedSinkError;

        async fn write_frame(
            &mut self,
            _frame: crate::AudioFrame,
        ) -> Result<Vec<AudioFrameSinkOutput>, Self::Error> {
            Ok(Vec::new())
        }

        async fn finish(
            &mut self,
            _capture_duration: Duration,
        ) -> Result<Vec<AudioFrameSinkOutput>, Self::Error> {
            self.attempts += 1;
            if self.attempts == 1 {
                return Err(UnusedSinkError);
            }
            Ok(vec![transcript_output()])
        }
    }

    fn transcript_output() -> AudioFrameSinkOutput {
        AudioFrameSinkOutput::Transcript(TranscriptSegment {
            id: "segment-final".into(),
            sequence: 0,
            start_ms: 0,
            end_ms: Some(500),
            text: "final words".into(),
            speaker: None,
            is_final: true,
        })
    }

    fn config(bot_name: &str) -> GoogleMeetRuntimeConfig {
        GoogleMeetRuntimeConfig {
            chromium: ChromiumLaunchConfig {
                binary: PathBuf::from("chromium"),
                user_data_dir: PathBuf::from("profile"),
                locale: "en-US".into(),
                authenticated: false,
                headless: false,
                disable_sandbox: false,
                startup_timeout: Duration::from_secs(10),
            },
            x11: X11InputConfig {
                binary: PathBuf::from("xdotool"),
                display: ":99".into(),
                command_timeout: Duration::from_secs(5),
            },
            bot_name: bot_name.into(),
            admission: AdmissionMonitorConfig::default(),
            runtime_poll_interval: Duration::from_secs(1),
        }
    }

    #[test]
    fn validates_runtime_configuration_before_launching_processes() {
        assert!(matches!(
            GoogleMeetRuntime::new(config("bad\nname"), UnusedSink),
            Err(GoogleMeetRuntimeConfigError::InvalidBotName)
        ));

        let mut invalid_x11 = config("Anarlog Notes");
        invalid_x11.x11.display.clear();
        assert!(matches!(
            GoogleMeetRuntime::new(invalid_x11, UnusedSink),
            Err(GoogleMeetRuntimeConfigError::X11(
                X11InputError::InvalidDisplay
            ))
        ));

        let mut invalid_monitor = config("Anarlog Notes");
        invalid_monitor.runtime_poll_interval = Duration::ZERO;
        assert!(matches!(
            GoogleMeetRuntime::new(invalid_monitor, UnusedSink),
            Err(GoogleMeetRuntimeConfigError::RuntimeMonitor(
                RuntimeMonitorError::InvalidPollInterval
            ))
        ));
    }

    #[test]
    fn rejects_other_platforms_before_launching_the_browser() {
        let runtime = GoogleMeetRuntime::new(config("Anarlog Notetaker"), UnusedSink).unwrap();
        let checkpoint = WorkerCheckpoint {
            job_id: "job-1".into(),
            bot_id: "bot-1".into(),
            provider: CaptureProviderKind::MicrosoftGraph,
            meeting: MeetingReference {
                platform: MeetingPlatform::MicrosoftTeams,
                url: "https://teams.microsoft.com/l/meetup-join/test".into(),
                external_id: None,
                calendar_event_id: None,
            },
            state: BotState::Queued,
            next_sequence: 0,
        };

        assert!(matches!(
            runtime.validate_checkpoint(&checkpoint),
            Err(GoogleMeetRuntimeError::UnsupportedCheckpoint)
        ));
    }

    #[tokio::test]
    async fn sequences_final_media_before_the_terminal_event() {
        let mut lifecycle = WorkerLifecycle::new("bot-1");
        lifecycle.launch_started(Utc::now()).unwrap();
        lifecycle
            .transition(BotState::Joined, None, Utc::now())
            .unwrap();
        lifecycle.capture_started(Utc::now()).unwrap();
        let (events_tx, mut events_rx) = mpsc::channel(2);

        send_outputs::<UnusedSinkError>(&events_tx, &mut lifecycle, vec![transcript_output()])
            .await
            .unwrap();
        send_event::<UnusedSinkError>(
            &events_tx,
            lifecycle
                .apply_runtime_outcome(
                    crate::RuntimeOutcome::MeetingEnded("meeting ended".into()),
                    Utc::now(),
                )
                .unwrap(),
        )
        .await
        .unwrap();

        let media = events_rx.recv().await.unwrap();
        let terminal = events_rx.recv().await.unwrap();
        assert_eq!((media.sequence, terminal.sequence), (3, 4));
        assert!(matches!(media.payload, CaptureEventPayload::Transcript(_)));
        assert!(matches!(
            terminal.payload,
            CaptureEventPayload::Lifecycle(ref transition)
                if transition.to == BotState::Completed
        ));
    }

    #[tokio::test]
    async fn retries_sink_finalization_during_cleanup_after_a_transient_failure() {
        let mut runtime =
            GoogleMeetRuntime::new(config("Anarlog Notes"), RetryingFinishSink { attempts: 0 })
                .unwrap();
        let frozen = Duration::from_secs(42);
        runtime.media = MediaState::Finalizing {
            capture_duration: frozen,
        };

        let first = runtime.finalize_media().await;
        assert!(first.sink_error.is_some());
        assert!(matches!(
            runtime.media,
            MediaState::Finalizing { capture_duration } if capture_duration == frozen
        ));

        let second = runtime.finalize_media().await;
        assert!(second.sink_error.is_none());
        assert_eq!(second.payloads, vec![transcript_output()]);
        assert!(matches!(runtime.media, MediaState::Finalized));
    }

    #[tokio::test]
    async fn finalize_is_a_no_op_before_capture_and_after_completion() {
        let mut runtime = GoogleMeetRuntime::new(config("Anarlog Notes"), UnusedSink).unwrap();

        let idle = runtime.finalize_media().await;
        assert!(idle.payloads.is_empty());
        assert!(idle.capture_error.is_none() && idle.sink_error.is_none());
        assert!(matches!(runtime.media, MediaState::Idle));

        runtime.media = MediaState::Finalized;
        let done = runtime.finalize_media().await;
        assert!(done.payloads.is_empty());
        assert!(done.capture_error.is_none() && done.sink_error.is_none());
        assert!(matches!(runtime.media, MediaState::Finalized));
    }

    #[tokio::test]
    async fn repeated_finalize_retries_reuse_the_frozen_duration() {
        let mut runtime = GoogleMeetRuntime::new(
            config("Anarlog Notes"),
            DurationRecordingSink {
                fail_attempts: 2,
                attempts: 0,
                durations: Vec::new(),
            },
        )
        .unwrap();
        let frozen = Duration::from_millis(1_234);
        runtime.media = MediaState::Finalizing {
            capture_duration: frozen,
        };

        assert!(runtime.finalize_media().await.sink_error.is_some());
        assert!(runtime.finalize_media().await.sink_error.is_some());
        assert!(runtime.finalize_media().await.sink_error.is_none());
        assert!(matches!(runtime.media, MediaState::Finalized));
        assert_eq!(runtime.audio_sink.durations, vec![frozen, frozen, frozen]);
    }
}
