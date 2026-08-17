use std::{error::Error, time::Duration};

use anlg_meeting_capture::{CaptureEvent, TransitionError};
use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::{mpsc, watch};

use crate::{
    AdmissionMonitor, AdmissionMonitorConfig, AdmissionMonitorError, AudioFrameSink,
    BrowserCapture, BrowserCaptureError, CaptureJobRuntime, ChromiumLaunchConfig, LobbyController,
    LobbyError, MeetingSession, MeetingSessionLaunchError, MeetingSessionShutdownError,
    RuntimeMonitor, RuntimeMonitorError, WorkerCheckpoint, WorkerLifecycle, X11Input,
    X11InputConfig, X11InputError,
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
    capture: Option<BrowserCapture>,
    started: bool,
    sink_started: bool,
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
            capture: None,
            started: false,
            sink_started: false,
        })
    }
}

#[async_trait]
impl<S> CaptureJobRuntime for GoogleMeetRuntime<S>
where
    S: AudioFrameSink,
{
    type Error = GoogleMeetRuntimeError<S::Error>;

    async fn run(
        &mut self,
        checkpoint: &WorkerCheckpoint,
        lifecycle: &mut WorkerLifecycle,
        events: mpsc::Sender<CaptureEvent>,
    ) -> Result<(), Self::Error> {
        if self.started {
            return Err(GoogleMeetRuntimeError::AlreadyStarted);
        }
        self.started = true;
        send_event(&events, lifecycle.launch_started(Utc::now())?).await?;

        let session =
            MeetingSession::launch(self.chromium.clone(), &checkpoint.meeting_url).await?;
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
        self.capture = Some(capture);
        self.sink_started = true;
        send_event(&events, lifecycle.capture_started(Utc::now())?).await?;

        let runtime_events = {
            let page = self
                .session
                .as_mut()
                .expect("launched runtime owns a session")
                .page_mut();
            let (_stop_tx, stop_rx) = watch::channel(false);
            let monitor = self.runtime_monitor.run(page, lifecycle, stop_rx);
            tokio::pin!(monitor);

            loop {
                tokio::select! {
                    result = &mut monitor => break result?,
                    frame = self.capture.as_mut().expect("capture was installed").next_frame() => {
                        self.audio_sink
                            .write_frame(frame?)
                            .await
                            .map_err(GoogleMeetRuntimeError::AudioSink)?;
                    }
                }
            }
        };

        for frame in self.stop_capture().await? {
            self.audio_sink
                .write_frame(frame)
                .await
                .map_err(GoogleMeetRuntimeError::AudioSink)?;
        }
        for event in runtime_events {
            send_event(&events, event).await?;
        }
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), Self::Error> {
        let (capture_error, trailing_frames) = match self.stop_capture().await {
            Ok(frames) => (None, frames),
            Err(error) => (Some(error), Vec::new()),
        };
        let mut sink_error = None;
        for frame in trailing_frames {
            if let Err(error) = self.audio_sink.write_frame(frame).await {
                sink_error = Some(error);
                break;
            }
        }
        if self.sink_started {
            self.sink_started = false;
            if let Err(error) = self.audio_sink.finish().await
                && sink_error.is_none()
            {
                sink_error = Some(error);
            }
        }
        let session_error = match self.session.take() {
            Some(session) => session.shutdown().await.err(),
            None => None,
        };

        if capture_error.is_none() && sink_error.is_none() && session_error.is_none() {
            Ok(())
        } else {
            Err(GoogleMeetRuntimeError::Cleanup {
                capture_error,
                sink_error,
                session_error,
            })
        }
    }
}

impl<S> GoogleMeetRuntime<S>
where
    S: AudioFrameSink,
{
    async fn stop_capture(&mut self) -> Result<Vec<crate::AudioFrame>, BrowserCaptureError> {
        let Some(mut capture) = self.capture.take() else {
            return Ok(Vec::new());
        };
        let Some(session) = self.session.as_mut() else {
            return Ok(Vec::new());
        };
        capture.stop_and_drain(session.page_mut()).await
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

    use super::*;

    struct UnusedSink;

    #[derive(Debug, thiserror::Error)]
    #[error("unused sink failed")]
    struct UnusedSinkError;

    #[async_trait]
    impl AudioFrameSink for UnusedSink {
        type Error = UnusedSinkError;

        async fn write_frame(&mut self, _frame: crate::AudioFrame) -> Result<(), Self::Error> {
            Ok(())
        }

        async fn finish(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
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
}
