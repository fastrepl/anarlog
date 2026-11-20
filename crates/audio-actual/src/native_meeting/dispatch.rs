//! Turns helper messages into Anarlog's capture stream and meeting events. Pure: the process
//! layer feeds it messages and delivers what it returns, so every rule here is unit-tested.

use std::sync::Arc;

use anlg_audio::meeting::{
    CallPlatform, CaptureEndReason, CaptureIdentity, CaptureInstant, CaptureScope, ConfirmedCall,
    MeetingCaptureEvent, MicProcessing, NATIVE_SAMPLE_RATE, ObservationSource, ObservedParticipant,
    SpeakerObservation, SpeakingCue, TrackKind, TrackStatus,
};
use anlg_audio::{CaptureChannel, CaptureEvent, Error};

use super::convert::{Chunk, Converter};
use super::echo::EchoCanceller;
use super::wire::{EventLine, Message, Track};

/// A track whose buffers stop, or whose samples stay exactly zero, this long is reported once.
const QUIET_FRAMES: u64 = 3 * NATIVE_SAMPLE_RATE as u64;

pub(crate) struct Started {
    pub scope: CaptureScope,
    pub mic: MicProcessing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ack {
    Paused,
    Resumed,
}

pub(crate) enum Output {
    Started(Result<Started, Error>),
    Stream(Result<CaptureEvent, Error>),
    Event(MeetingCaptureEvent),
    Ack(Result<Ack, String>),
}

#[derive(Default)]
struct TrackState {
    ready: bool,
    status: Option<TrackStatus>,
    pcm_end: u64,
    sound_end: u64,
    frames: u64,
}

struct Ended {
    reason: String,
    frame: u64,
}

pub(crate) struct Dispatcher {
    capture: CaptureIdentity,
    converter: Converter,
    mic: TrackState,
    remote: TrackState,
    microphone_name: Option<String>,
    voice_processed: bool,
    started: bool,
    paused: bool,
    clock: u64,
    ended: Option<Ended>,
    failure: Option<String>,
    echo: EchoCanceller,
}

impl Dispatcher {
    pub(crate) fn new(capture: CaptureIdentity, converter: Converter, echo: EchoCanceller) -> Self {
        Self {
            capture,
            converter,
            mic: TrackState::default(),
            remote: TrackState::default(),
            microphone_name: None,
            voice_processed: true,
            started: false,
            paused: false,
            clock: 0,
            ended: None,
            failure: None,
            echo,
        }
    }

    pub(crate) fn handle(&mut self, message: Message) -> Vec<Output> {
        let mut out = Vec::new();
        match message {
            Message::Pcm {
                track,
                frame,
                samples,
            } => self.pcm(track, frame, &samples, &mut out),
            Message::Event(line) => self.event(&line, &mut out),
        }
        out
    }

    fn pcm(&mut self, track: Track, frame: u64, samples: &[f32], out: &mut Vec<Output>) {
        if samples.is_empty() || self.ended.is_some() {
            return;
        }
        self.converter.push(track, frame, samples);
        let end = frame + samples.len() as u64;
        let state = self.track(track);
        state.frames += samples.len() as u64;
        state.pcm_end = state.pcm_end.max(end);
        if samples.iter().any(|sample| *sample != 0.0) {
            state.sound_end = state.sound_end.max(end);
        }
        let regained = end == state.sound_end
            && matches!(
                state.status,
                Some(TrackStatus::NoBuffers | TrackStatus::Silent)
            );
        if track == Track::Microphone && !self.mic.ready {
            self.mic.ready = true;
            out.push(Output::Stream(Ok(CaptureEvent::ChannelReady {
                channel: CaptureChannel::Mic,
                device: self.microphone_name.clone(),
            })));
            self.status(track, TrackStatus::Ready, out);
        } else if regained {
            self.status(track, TrackStatus::Ready, out);
        }
        self.drain(out);
    }

    fn event(&mut self, line: &EventLine, out: &mut Vec<Output>) {
        match line.event() {
            "started" => {
                let started = self.started(line);
                self.started = started.is_ok();
                out.push(Output::Started(started));
            }
            "microphone-configured" => {
                self.set_voice_processed(line, out);
                // The graph follows the Mac's default input, which may be another device now.
                if let Some(name) = line.get("microphoneName") {
                    self.microphone_name = Some(name.to_string());
                }
                self.mic.ready = false;
                // A rebuilt graph is a new echo path.
                self.echo.reset();
            }
            "microphone-interrupted" => self.channel_failed(
                Track::Microphone,
                TrackStatus::Interrupted,
                "the microphone is changing devices".into(),
                out,
            ),
            "microphone-unavailable" => {
                let message = line.get("message").unwrap_or("unavailable").to_string();
                self.channel_failed(
                    Track::Microphone,
                    TrackStatus::Failed {
                        message: message.clone(),
                    },
                    message,
                    out,
                );
            }
            "microphone-recovered" => {
                self.set_voice_processed(line, out);
                self.status(Track::Microphone, TrackStatus::Recovered, out);
            }
            "call-audio-configured" => {
                let recovered = matches!(
                    self.remote.status,
                    Some(TrackStatus::Interrupted | TrackStatus::Failed { .. })
                );
                self.remote.ready = true;
                self.echo.reset();
                self.remote.pcm_end = self.remote.pcm_end.max(self.clock);
                self.remote.sound_end = self.remote.sound_end.max(self.clock);
                out.push(Output::Stream(Ok(CaptureEvent::ChannelReady {
                    channel: CaptureChannel::Speaker,
                    device: None,
                })));
                let status = if recovered {
                    TrackStatus::Recovered
                } else {
                    TrackStatus::Ready
                };
                self.status(Track::Remote, status, out);
            }
            "call-audio-interrupted" => self.channel_failed(
                Track::Remote,
                TrackStatus::Interrupted,
                format!(
                    "call audio is rebuilding: {}",
                    line.get("reason").unwrap_or("unknown")
                ),
                out,
            ),
            "call-audio-unavailable" => {
                let message = line.get("message").unwrap_or("unavailable").to_string();
                self.channel_failed(
                    Track::Remote,
                    TrackStatus::Failed {
                        message: message.clone(),
                    },
                    message,
                    out,
                );
            }
            "tick" if self.ended.is_none() => {
                if let Some(frame) = line.u64("frame") {
                    self.clock = self.clock.max(frame);
                    self.converter.advance(frame);
                    self.quiet(out);
                    self.drain(out);
                }
            }
            "paused" | "resumed" => {
                let paused = line.event() == "paused";
                let at = CaptureInstant(line.u64("frame").unwrap_or(self.clock));
                self.paused = paused;
                if !paused {
                    // Nothing was owed while paused; quiet time restarts at the resume.
                    for state in [&mut self.mic, &mut self.remote] {
                        state.pcm_end = state.pcm_end.max(at.0);
                        state.sound_end = state.sound_end.max(at.0);
                    }
                }
                out.push(Output::Event(if paused {
                    MeetingCaptureEvent::Paused { at }
                } else {
                    MeetingCaptureEvent::Resumed { at }
                }));
                out.push(Output::Ack(Ok(if paused {
                    Ack::Paused
                } else {
                    Ack::Resumed
                })));
            }
            "command-error" => out.push(Output::Ack(Err(line
                .get("message")
                .unwrap_or("the helper refused the command")
                .to_string()))),
            "meeting-observation" => {
                if let Some(observation) = self.observation(line) {
                    out.push(Output::Event(MeetingCaptureEvent::Observation(observation)));
                }
            }
            "meeting-observation-unavailable" => {
                out.push(Output::Event(MeetingCaptureEvent::ObservationUnavailable {
                    at: CaptureInstant(line.u64("frame").unwrap_or(self.clock)),
                    reason: line.get("reason").unwrap_or("unknown").to_string(),
                }))
            }
            "ended" => {
                let frames = line.u64("frames").unwrap_or(self.clock);
                for chunk in self.converter.finish(frames) {
                    out.push(self.chunk(chunk));
                }
                for track in [Track::Microphone, Track::Remote] {
                    if self.track(track).frames == 0
                        && self.track(track).status != Some(TrackStatus::NoBuffers)
                    {
                        self.status(track, TrackStatus::NoBuffers, out);
                    }
                }
                self.ended = Some(Ended {
                    reason: line.get("reason").unwrap_or("capture_failed").to_string(),
                    frame: line.u64("frame").unwrap_or(frames),
                });
            }
            "failed" => {
                let message = format!(
                    "{}: {}",
                    line.get("code").unwrap_or("capture_failed"),
                    line.get("message").unwrap_or("the native helper failed")
                );
                if !self.started {
                    out.push(Output::Started(Err(start_error(line, message.clone()))));
                }
                self.failure = Some(message);
            }
            "live-output-overrun"
            | "microphone-startup-retry"
            | "microphone-buffer-dropped"
            | "call-audio-buffer-dropped"
            | "call-audio-processes-changed" => {
                tracing::info!(event = line.event(), "native_meeting_helper_event");
            }
            _ => {}
        }
    }

    /// The helper's output ended. `exited_cleanly` is whether it exited with status 0.
    pub(crate) fn close(&mut self, exited_cleanly: bool, detail: Option<String>) -> Vec<Output> {
        let mut out = Vec::new();
        if !self.started && self.failure.is_none() {
            out.push(Output::Started(Err(Error::MeetingCaptureFailed(
                detail
                    .clone()
                    .unwrap_or_else(|| "the native helper exited before starting".into()),
            ))));
        }
        let reason = match (&self.ended, &self.failure, exited_cleanly) {
            (Some(ended), None, true) => end_reason(&ended.reason),
            _ => CaptureEndReason::CaptureFailed,
        };
        let at = CaptureInstant(self.ended.as_ref().map_or(self.clock, |ended| ended.frame));
        if self.started {
            out.push(Output::Event(MeetingCaptureEvent::Ended { at, reason }));
            // The helper finished its tracks on purpose: the runtime ends the recording, so the
            // stream must not read as a failure the listener would reopen.
            if !matches!(
                reason,
                CaptureEndReason::UserStopped
                    | CaptureEndReason::AppClosed
                    | CaptureEndReason::DurationLimit
            ) {
                let message =
                    self.failure
                        .clone()
                        .or(detail)
                        .unwrap_or_else(|| match &self.ended {
                            Some(ended) => format!("the native capture ended: {}", ended.reason),
                            None => "the native helper exited before finishing its tracks".into(),
                        });
                out.push(Output::Stream(Err(Error::MeetingCaptureFailed(message))));
            }
        }
        out
    }

    pub(crate) fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }

    fn started(&mut self, line: &EventLine) -> Result<Started, Error> {
        self.microphone_name = line.get("microphoneName").map(str::to_string);
        let mic = match line.get("micProcessing") {
            Some("raw") => MicProcessing::Raw,
            _ => MicProcessing::VoiceProcessed,
        };
        self.voice_processed = mic == MicProcessing::VoiceProcessed;
        let scope = match line.get("scope") {
            Some("system") => CaptureScope::System,
            Some("application") => {
                let call = line
                    .get("pid")
                    .and_then(|pid| pid.parse::<i32>().ok())
                    .filter(|pid| *pid > 0)
                    .zip(line.get("bundleId").filter(|id| !id.is_empty()))
                    .zip(line.get("platform").and_then(CallPlatform::from_helper));
                let Some(((pid, bundle_id), platform)) = call else {
                    return Err(Error::MeetingCaptureFailed(
                        "the helper started an app capture without a confirmed call".into(),
                    ));
                };
                CaptureScope::Call(ConfirmedCall {
                    pid,
                    bundle_id: bundle_id.to_string(),
                    platform,
                    call_url: line.get("callUrl").map(str::to_string),
                })
            }
            _ => {
                return Err(Error::MeetingCaptureFailed(
                    "the helper started without a scope".into(),
                ));
            }
        };
        Ok(Started { scope, mic })
    }

    fn observation(&self, line: &EventLine) -> Option<SpeakerObservation> {
        let platform = CallPlatform::from_helper(line.get("platform")?)?;
        let mut participants: Vec<ObservedParticipant> = Vec::new();
        let mut speaking = Vec::new();
        for (key, value) in line.fields() {
            match key {
                "participant.id" => participants.push(ObservedParticipant {
                    participant_id: value.to_string(),
                    display_name: None,
                    is_self: None,
                }),
                "participant.name" => {
                    participants.last_mut()?.display_name = Some(value.to_string())
                }
                "participant.local" => {
                    participants.last_mut()?.is_self = match value {
                        "true" => Some(true),
                        "false" => Some(false),
                        _ => None,
                    }
                }
                "speaker.id" => speaking.push(SpeakingCue {
                    participant_id: Some(value.to_string()),
                    display_name: None,
                }),
                "speaker.name" => speaking.last_mut()?.display_name = Some(value.to_string()),
                _ => {}
            }
        }
        let participant_count = line
            .get("participantCount")
            .and_then(|count| count.parse::<u32>().ok());
        // Complete only when the app's own count matches the rows it shows. Fixtures support this
        // for Webex with its participant panel open; nothing else reports a count.
        let roster_complete = line.bool("rosterVisible") == Some(true)
            && participant_count.is_some_and(|count| count as usize == participants.len());
        Some(SpeakerObservation {
            capture: self.capture.clone(),
            source: ObservationSource::Accessibility,
            platform,
            at: CaptureInstant(line.u64("frame")?),
            participants,
            roster_complete,
            participant_count,
            // Accessibility has no speaking signal: absent, never "nobody is speaking".
            speaking: (line.get("speakerStatus") == Some("available")).then_some(speaking),
        })
    }

    /// Chunks drained from now on carry the new mode; the event records when it changed. The
    /// mode the helper started with comes from `started`, so earlier reports announce nothing.
    fn set_voice_processed(&mut self, line: &EventLine, out: &mut Vec<Output>) {
        let voice_processed = line.bool("voiceProcessing").unwrap_or(true);
        if self.started && voice_processed != self.voice_processed {
            let at = line
                .get("atMs")
                .and_then(|ms| ms.parse::<f64>().ok())
                .and_then(CaptureInstant::from_millis)
                .unwrap_or(CaptureInstant(self.clock));
            out.push(Output::Event(MeetingCaptureEvent::MicProcessingChanged {
                at,
                mic: if voice_processed {
                    MicProcessing::VoiceProcessed
                } else {
                    MicProcessing::Raw
                },
            }));
        }
        self.voice_processed = voice_processed;
    }

    fn channel_failed(
        &mut self,
        track: Track,
        status: TrackStatus,
        message: String,
        out: &mut Vec<Output>,
    ) {
        self.track(track).ready = false;
        let (channel, error) = match track {
            Track::Microphone => (
                CaptureChannel::Mic,
                Error::MicStreamInitializationFailed(message),
            ),
            Track::Remote => (
                CaptureChannel::Speaker,
                Error::SpeakerStreamInitializationFailed(message),
            ),
        };
        out.push(Output::Stream(Ok(CaptureEvent::ChannelFailed {
            channel,
            error,
        })));
        self.status(track, status, out);
    }

    /// A ready track that delivers nothing, or only exact zeros, says so once. A silent call tap
    /// gets no buffers at all; a muted device can deliver zeros; both differ from a device error.
    fn quiet(&mut self, out: &mut Vec<Output>) {
        if self.paused || self.ended.is_some() {
            return;
        }
        for track in [Track::Microphone, Track::Remote] {
            let clock = self.clock;
            let state = self.track(track);
            let ready = match track {
                Track::Microphone => state.frames > 0 && state.ready,
                Track::Remote => state.ready,
            };
            if !ready {
                continue;
            }
            let quiet = if clock.saturating_sub(state.pcm_end) > QUIET_FRAMES {
                TrackStatus::NoBuffers
            } else if clock.saturating_sub(state.sound_end) > QUIET_FRAMES {
                TrackStatus::Silent
            } else {
                continue;
            };
            if state.status.as_ref() != Some(&quiet) {
                self.status(track, quiet, out);
            }
        }
    }

    fn status(&mut self, track: Track, status: TrackStatus, out: &mut Vec<Output>) {
        let state = self.track(track);
        if state.status.as_ref() == Some(&status) && status != TrackStatus::Recovered {
            return;
        }
        state.status = Some(status.clone());
        out.push(Output::Event(MeetingCaptureEvent::TrackStatus {
            track: match track {
                Track::Microphone => TrackKind::Microphone,
                Track::Remote => TrackKind::Remote,
            },
            status,
        }));
    }

    fn drain(&mut self, out: &mut Vec<Output>) {
        match self.converter.drain() {
            Ok(chunks) => {
                for chunk in chunks {
                    out.push(self.chunk(chunk));
                }
            }
            Err(error) => {
                self.failure.get_or_insert_with(|| error.to_string());
                out.push(Output::Stream(Err(Error::MeetingCaptureFailed(
                    error.to_string(),
                ))));
            }
        }
    }

    fn chunk(&mut self, mut chunk: Chunk) -> Output {
        // Apple's voice processing already cancelled the echo: the frame says so, and the
        // capture's provenance tells Anarlog never to cancel it again. A raw microphone gets
        // Anarlog's canceller against the call track, as on the existing engine.
        if self.voice_processed {
            chunk.frame.aec_mic = Some(Arc::clone(&chunk.frame.raw_mic));
        } else if chunk.captured && self.mic.ready && self.remote.ready {
            chunk.frame.aec_mic = self
                .echo
                .cancel(&chunk.frame.raw_mic, &chunk.frame.raw_speaker);
        }
        Output::Stream(Ok(if chunk.captured {
            CaptureEvent::Frame(chunk.frame)
        } else {
            CaptureEvent::Gap(chunk.frame)
        }))
    }

    fn track(&mut self, track: Track) -> &mut TrackState {
        match track {
            Track::Microphone => &mut self.mic,
            Track::Remote => &mut self.remote,
        }
    }
}

fn end_reason(reason: &str) -> CaptureEndReason {
    match reason {
        "user_stopped" => CaptureEndReason::UserStopped,
        "app_closed" => CaptureEndReason::AppClosed,
        "duration_limit" => CaptureEndReason::DurationLimit,
        _ => CaptureEndReason::CaptureFailed,
    }
}

/// Refusals the user can act on are "unavailable"; anything else failed.
fn start_error(line: &EventLine, message: String) -> Error {
    match line.get("code") {
        Some(
            "microphone_not_default"
            | "microphone_permission"
            | "microphone_unavailable"
            | "call_app_unavailable",
        ) => Error::MeetingCaptureUnavailable(message),
        _ => Error::MeetingCaptureFailed(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_meeting::wire::encode_event;
    use crate::native_meeting::wire::read_message;

    fn identity() -> CaptureIdentity {
        CaptureIdentity {
            account_id: "account".into(),
            session_id: "session".into(),
            capture_id: "capture".into(),
        }
    }

    /// Headphones: no echo cancellation, so raw frames stay raw.
    fn dispatcher() -> Dispatcher {
        Dispatcher::new(
            identity(),
            Converter::new(16_000, 1_920).unwrap(),
            EchoCanceller::new(16_000, Arc::new(std::sync::atomic::AtomicBool::new(false))),
        )
    }

    fn event(name: &str, fields: &[(&str, &str)]) -> Message {
        read_message(&mut encode_event(name, fields).as_slice())
            .unwrap()
            .unwrap()
    }

    fn pcm(track: Track, frame: u64, samples: Vec<f32>) -> Message {
        Message::Pcm {
            track,
            frame,
            samples,
        }
    }

    fn tick(frame: u64) -> Message {
        event("tick", &[("frame", &frame.to_string())])
    }

    fn started() -> Message {
        event(
            "started",
            &[
                ("scope", "application"),
                ("pid", "42"),
                ("bundleId", "com.google.Chrome"),
                ("platform", "google-meet"),
                ("callUrl", "https://meet.google.com/abc-defg-hij"),
                ("microphoneName", "MacBook Pro Microphone"),
                ("micProcessing", "voice_processed"),
            ],
        )
    }

    #[derive(Debug, Default, PartialEq)]
    struct Seen {
        frames: usize,
        gaps: usize,
        samples: usize,
        ready: Vec<CaptureChannel>,
        failed: Vec<CaptureChannel>,
        errors: Vec<String>,
        statuses: Vec<(TrackKind, TrackStatus)>,
        events: Vec<String>,
        voice_processed_frames: usize,
    }

    fn feed(dispatcher: &mut Dispatcher, messages: Vec<Message>, seen: &mut Seen) {
        for message in messages {
            for output in dispatcher.handle(message) {
                record(output, seen);
            }
        }
    }

    fn record(output: Output, seen: &mut Seen) {
        match output {
            Output::Stream(Ok(CaptureEvent::Frame(frame))) => {
                seen.frames += 1;
                seen.samples += frame.raw_mic.len();
                seen.voice_processed_frames += frame.aec_mic.is_some() as usize;
            }
            Output::Stream(Ok(CaptureEvent::Gap(frame))) => {
                seen.gaps += 1;
                seen.samples += frame.raw_mic.len();
            }
            Output::Stream(Ok(CaptureEvent::ChannelReady { channel, .. })) => {
                seen.ready.push(channel)
            }
            Output::Stream(Ok(CaptureEvent::ChannelFailed { channel, .. })) => {
                seen.failed.push(channel)
            }
            Output::Stream(Err(error)) => seen.errors.push(error.to_string()),
            Output::Event(MeetingCaptureEvent::TrackStatus { track, status }) => {
                seen.statuses.push((track, status))
            }
            Output::Event(event) => seen.events.push(format!("{event:?}")),
            Output::Started(_) | Output::Ack(_) => {}
        }
    }

    #[test]
    fn the_started_line_carries_the_confirmed_call_and_mic_provenance() {
        let mut dispatcher = dispatcher();
        let [Output::Started(Ok(started))] = &dispatcher.handle(started())[..] else {
            panic!("expected a start");
        };
        assert_eq!(started.mic, MicProcessing::VoiceProcessed);
        assert_eq!(
            started.scope,
            CaptureScope::Call(ConfirmedCall {
                pid: 42,
                bundle_id: "com.google.Chrome".into(),
                platform: CallPlatform::GoogleMeet,
                call_url: Some("https://meet.google.com/abc-defg-hij".into()),
            })
        );
        // An app capture without its confirmed call is refused, never treated as system-wide.
        let mut dispatcher = super::tests::dispatcher();
        let out = dispatcher.handle(event("started", &[("scope", "application"), ("pid", "1")]));
        assert!(matches!(&out[..], [Output::Started(Err(_))]));
    }

    #[test]
    fn a_refusal_before_start_is_unavailable_with_its_code() {
        let mut dispatcher = dispatcher();
        let out = dispatcher.handle(event(
            "failed",
            &[
                ("code", "microphone_not_default"),
                ("message", "Not the default"),
            ],
        ));
        let [Output::Started(Err(Error::MeetingCaptureUnavailable(message)))] = &out[..] else {
            panic!("expected an unavailable start");
        };
        assert_eq!(message, "microphone_not_default: Not the default");
    }

    /// A capture with a pause: audio before and after, Gap over the pause on the same clock,
    /// and the pause reported as capture instants.
    #[test]
    fn a_pause_is_gap_padding_and_paired_instants() {
        let mut dispatcher = dispatcher();
        let mut seen = Seen::default();
        let configured = event(
            "call-audio-configured",
            &[("scope", "application"), ("processes", "2")],
        );
        feed(
            &mut dispatcher,
            vec![
                started(),
                configured,
                pcm(Track::Microphone, 0, vec![0.25; 48_000]),
                pcm(Track::Remote, 0, vec![0.5; 48_000]),
                tick(48_000),
                event("paused", &[("frame", "48000")]),
                tick(240_000),
                event("resumed", &[("frame", "240000")]),
                pcm(Track::Microphone, 240_000, vec![0.25; 48_000]),
                pcm(Track::Remote, 240_000, vec![0.5; 48_000]),
                tick(288_000),
                event("stopping", &[("frame", "288000")]),
                event(
                    "ended",
                    &[
                        ("reason", "user_stopped"),
                        ("frame", "288000"),
                        ("frames", "288000"),
                        ("microphoneFrames", "96000"),
                        ("remoteFrames", "96000"),
                    ],
                ),
            ],
            &mut seen,
        );
        for output in dispatcher.close(true, None) {
            record(output, &mut seen);
        }
        assert_eq!(seen.samples, 96_000, "6 s of capture time at 16 kHz");
        // Four seconds paused, longer than a quiet track is reported after: every whole chunk
        // inside the pause is a Gap.
        assert_eq!(seen.gaps, 64_000 / 1_920 - 1);
        assert_eq!(seen.frames + seen.gaps, 96_000_usize.div_ceil(1_920));
        assert_eq!(seen.voice_processed_frames, seen.frames);
        assert_eq!(
            seen.ready,
            vec![CaptureChannel::Speaker, CaptureChannel::Mic]
        );
        assert!(seen.errors.is_empty(), "{:?}", seen.errors);
        assert_eq!(
            seen.events,
            vec![
                "Paused { at: CaptureInstant(48000) }",
                "Resumed { at: CaptureInstant(240000) }",
                "Ended { at: CaptureInstant(288000), reason: UserStopped }",
            ]
        );
        // A pause is not a track without buffers.
        assert!(
            !seen
                .statuses
                .iter()
                .any(|(_, status)| *status == TrackStatus::NoBuffers)
        );
    }

    #[test]
    fn no_buffers_silence_and_device_errors_stay_distinct() {
        let mut dispatcher = dispatcher();
        let mut seen = Seen::default();
        feed(
            &mut dispatcher,
            vec![
                started(),
                event("call-audio-configured", &[]),
                // The microphone delivers exact zeros (a muted device); the call tap nothing.
                pcm(Track::Microphone, 0, vec![0.0; 48_000 * 4]),
                tick(48_000 * 4),
                event("microphone-unavailable", &[("message", "device gone")]),
            ],
            &mut seen,
        );
        assert_eq!(
            seen.statuses,
            vec![
                (TrackKind::Remote, TrackStatus::Ready),
                (TrackKind::Microphone, TrackStatus::Ready),
                (TrackKind::Microphone, TrackStatus::Silent),
                (TrackKind::Remote, TrackStatus::NoBuffers),
                (
                    TrackKind::Microphone,
                    TrackStatus::Failed {
                        message: "device gone".into()
                    }
                ),
            ]
        );
        assert_eq!(seen.failed, vec![CaptureChannel::Mic]);
        // Sound on the call clears its quiet status.
        feed(
            &mut dispatcher,
            vec![pcm(Track::Remote, 48_000 * 4, vec![0.3; 4_800])],
            &mut seen,
        );
        assert_eq!(
            seen.statuses.last(),
            Some(&(TrackKind::Remote, TrackStatus::Ready))
        );
    }

    #[test]
    fn an_empty_recording_reports_tracks_without_buffers() {
        let mut dispatcher = dispatcher();
        let mut seen = Seen::default();
        feed(
            &mut dispatcher,
            vec![
                started(),
                event(
                    "ended",
                    &[("reason", "user_stopped"), ("frames", "0"), ("frame", "0")],
                ),
            ],
            &mut seen,
        );
        for output in dispatcher.close(true, None) {
            record(output, &mut seen);
        }
        assert_eq!(seen.samples, 0);
        assert_eq!(
            seen.statuses,
            vec![
                (TrackKind::Microphone, TrackStatus::NoBuffers),
                (TrackKind::Remote, TrackStatus::NoBuffers),
            ]
        );
        assert_eq!(
            seen.events,
            vec!["Ended { at: CaptureInstant(0), reason: UserStopped }"]
        );
        assert!(seen.errors.is_empty());
    }

    #[test]
    fn a_helper_that_dies_mid_capture_fails_the_stream_and_ends_the_capture() {
        let mut dispatcher = dispatcher();
        let mut seen = Seen::default();
        feed(
            &mut dispatcher,
            vec![
                started(),
                pcm(Track::Microphone, 0, vec![0.1; 9_600]),
                tick(9_600),
            ],
            &mut seen,
        );
        for output in dispatcher.close(false, Some("killed by signal 9".into())) {
            record(output, &mut seen);
        }
        assert_eq!(
            seen.events,
            vec!["Ended { at: CaptureInstant(9600), reason: CaptureFailed }"]
        );
        assert_eq!(
            seen.errors,
            vec!["meeting capture failed: killed by signal 9"]
        );
        // A failure the helper reported wins over a clean-looking end.
        let mut dispatcher = super::tests::dispatcher();
        let mut seen = Seen::default();
        feed(
            &mut dispatcher,
            vec![
                started(),
                event("ended", &[("reason", "capture_failed"), ("frames", "0")]),
                event(
                    "failed",
                    &[
                        ("code", "live_output_overrun"),
                        ("message", "Anarlog stopped reading"),
                    ],
                ),
            ],
            &mut seen,
        );
        for output in dispatcher.close(false, None) {
            record(output, &mut seen);
        }
        assert_eq!(
            seen.errors,
            vec!["meeting capture failed: live_output_overrun: Anarlog stopped reading"]
        );
    }

    #[test]
    fn a_call_app_quitting_or_the_length_limit_ends_the_capture_without_a_stream_failure() {
        for (reason, expected) in [
            ("app_closed", CaptureEndReason::AppClosed),
            ("duration_limit", CaptureEndReason::DurationLimit),
        ] {
            let mut dispatcher = dispatcher();
            let mut seen = Seen::default();
            feed(
                &mut dispatcher,
                vec![
                    started(),
                    pcm(Track::Microphone, 0, vec![0.1; 4_800]),
                    event(
                        "ended",
                        &[("reason", reason), ("frames", "4800"), ("frame", "4800")],
                    ),
                ],
                &mut seen,
            );
            for output in dispatcher.close(true, None) {
                record(output, &mut seen);
            }
            assert_eq!(
                seen.events,
                vec![format!(
                    "Ended {{ at: CaptureInstant(4800), reason: {expected:?} }}"
                )]
            );
            // A failure here would make the listener reopen the helper: past the limit, or on a
            // call app that is gone.
            assert!(seen.errors.is_empty(), "{reason}: {:?}", seen.errors);
        }
    }

    #[test]
    fn a_headphone_switch_reports_when_mic_processing_changed() {
        let mut dispatcher = dispatcher();
        let mut seen = Seen::default();
        feed(
            &mut dispatcher,
            vec![
                // The graph is configured before `started`; that is the starting mode, not a change.
                event("microphone-configured", &[("voiceProcessing", "true")]),
                started(),
                pcm(Track::Microphone, 0, vec![0.1; 4_800]),
                tick(4_800),
                event(
                    "microphone-configured",
                    &[("voiceProcessing", "false"), ("atMs", "100.0")],
                ),
                pcm(Track::Microphone, 4_800, vec![0.1; 4_800]),
                tick(9_600),
                event("microphone-recovered", &[("voiceProcessing", "false")]),
            ],
            &mut seen,
        );
        assert_eq!(
            seen.events,
            vec!["MicProcessingChanged { at: CaptureInstant(4800), mic: Raw }"]
        );
        // Raw frames are not marked as already echo-cancelled.
        assert!(seen.frames > seen.voice_processed_frames);
    }

    #[test]
    fn a_microphone_that_moved_to_the_new_default_is_ready_under_its_own_name() {
        let mut dispatcher = dispatcher();
        let mut ready = Vec::new();
        for message in [
            started(),
            pcm(Track::Microphone, 0, vec![0.1; 4_800]),
            event("microphone-interrupted", &[]),
            event(
                "microphone-configured",
                &[("voiceProcessing", "false"), ("microphoneName", "AirPods")],
            ),
            event("microphone-recovered", &[("voiceProcessing", "false")]),
            pcm(Track::Microphone, 9_600, vec![0.1; 4_800]),
        ] {
            for output in dispatcher.handle(message) {
                if let Output::Stream(Ok(CaptureEvent::ChannelReady {
                    channel: CaptureChannel::Mic,
                    device,
                })) = output
                {
                    ready.push(device);
                }
            }
        }
        assert_eq!(
            ready,
            vec![
                Some("MacBook Pro Microphone".to_string()),
                Some("AirPods".to_string())
            ]
        );
    }

    #[test]
    fn roster_observations_are_hints_with_no_speaking_signal() {
        let mut dispatcher = dispatcher();
        let out = dispatcher.handle(event(
            "meeting-observation",
            &[
                ("frame", "96000"),
                ("platform", "webex"),
                ("scopeId", "window"),
                ("rosterVisible", "true"),
                ("participantCount", "2"),
                ("speakerStatus", "unavailable"),
                ("participant.id", "window/name"),
                ("participant.name", "Alex"),
                ("participant.local", "true"),
                ("participant.id", "window/name2"),
                ("participant.name", "Sam"),
                ("participant.local", "unknown"),
            ],
        ));
        let [Output::Event(MeetingCaptureEvent::Observation(observation))] = &out[..] else {
            panic!("expected an observation");
        };
        assert_eq!(observation.capture, identity());
        assert_eq!(observation.at, CaptureInstant(96_000));
        assert!(observation.roster_complete);
        assert_eq!(observation.speaking, None);
        assert_eq!(observation.participants[0].is_self, Some(true));
        assert_eq!(observation.participants[1].is_self, None);
        assert_eq!(
            observation.participants[1].display_name.as_deref(),
            Some("Sam")
        );
        // A visible roster without the app's own count is never complete.
        let out = dispatcher.handle(event(
            "meeting-observation",
            &[
                ("frame", "1"),
                ("platform", "zoom"),
                ("rosterVisible", "true"),
                ("participant.id", "a"),
            ],
        ));
        let [Output::Event(MeetingCaptureEvent::Observation(observation))] = &out[..] else {
            panic!("expected an observation");
        };
        assert!(!observation.roster_complete);
    }
}
