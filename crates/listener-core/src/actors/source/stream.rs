use futures_util::StreamExt;
use ractor::{ActorProcessingErr, ActorRef};
use std::sync::atomic::Ordering;
use tokio_util::sync::CancellationToken;

use crate::{SessionDataEvent, SessionProgressEvent, actors::ChannelMode};
use anlg_audio::{AudioProvider, CaptureConfig, CaptureFrame, CaptureStream};
use anlg_audio_utils::chunk_size_for_stt;

use super::{SourceFrame, SourceMsg, SourceState};

const CAPTURE_FRAME_QUEUE_CAPACITY: usize = 32;

/// Returns the settings the streams opened with so the routing watcher can start from the same
/// verdict instead of sampling its own.
pub(super) async fn start_source_loop(
    myself: &ActorRef<SourceMsg>,
    st: &mut SourceState,
) -> Result<CaptureSettings, ActorProcessingErr> {
    let new_mode = ChannelMode::determine(st.onboarding);

    let mode_changed = st.current_mode != new_mode;
    st.current_mode = new_mode;

    tracing::info!(?new_mode, mode_changed, "start_source_loop");

    st.pipeline.reset();

    st.active_mic_device = active_mic_device(st.mic_device.clone(), st.audio.as_ref());
    let mic_swapped = st.mic_device.is_none() && st.active_mic_device.is_some();
    let capture = capture_settings(mic_swapped);
    let result = start_streams(myself, st, capture).await;

    if result.is_ok() {
        st.runtime.emit_progress(SessionProgressEvent::AudioReady {
            session_id: st.session_id.clone(),
            device: st.active_mic_device.clone(),
        });
        if new_mode == ChannelMode::MicAndSpeaker {
            st.runtime.emit_data(SessionDataEvent::MicIsolated {
                session_id: st.session_id.clone(),
                value: capture.mic_isolated,
            });
        }
    }

    result.map(|()| capture)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CaptureSettings {
    enable_aec: bool,
    /// Every playing output is a headphone. Re-evaluated per stream because the source restarts
    /// on default output changes; the routing watcher starts from this verdict.
    pub(super) headphone_output: bool,
    /// Headphones keep speaker output out of the mic, so whatever the mic hears is the local
    /// user. Not claimed when the mic was swapped away from the user's Bluetooth headset: the
    /// headset mic sat on their head, the replacement is a room mic as far as we know.
    pub(super) mic_isolated: bool,
}

// Headphones make AEC pure cost: it burns CPU and can degrade near-end speech. The check covers
// every output that is playing, not just the default, because meeting apps pick their own speaker.
fn capture_settings(mic_swapped: bool) -> CaptureSettings {
    let headphone_output = anlg_audio_device::headphone_only_output();
    if let Some(device) = &headphone_output {
        tracing::info!(
            device = %device.name,
            transport = ?device.transport_type,
            "aec_disabled_headphone_output"
        );
    }
    resolve_capture_settings(
        std::env::var("NO_AEC").as_deref() == Ok("1"),
        headphone_output.is_some(),
        mic_swapped,
    )
}

fn resolve_capture_settings(
    no_aec_override: bool,
    headphone_output: bool,
    mic_swapped: bool,
) -> CaptureSettings {
    CaptureSettings {
        enable_aec: !no_aec_override && !headphone_output,
        headphone_output,
        mic_isolated: headphone_output && !mic_swapped,
    }
}

// The mic opens by the provider's device name, which only matches the device layer's name on
// macOS and Windows. On Linux the mic goes through cpal/ALSA, whose names are ALSA PCM hints and
// never match PulseAudio source names, so a replacement could not be opened; avoiding HFP there
// needs a PulseAudio/PipeWire mic capture path.
const SWAPS_BLUETOOTH_DEFAULT_MIC: bool = cfg!(any(target_os = "macos", target_os = "windows"));

// Opening a Bluetooth headset's mic forces it into HFP: the headset gates the mic to silence
// between words and the wearer's audio drops to 16 kHz. Only the system default is swapped; an
// explicit selection is respected.
fn active_mic_device(explicit: Option<String>, audio: &dyn AudioProvider) -> Option<String> {
    if explicit.is_some() || !SWAPS_BLUETOOTH_DEFAULT_MIC {
        return explicit;
    }

    let replacement = anlg_audio_device::wired_input_replacing_bluetooth_default()?;
    let active = audio
        .list_mic_devices()
        .into_iter()
        .find(|name| *name == replacement.name);
    match &active {
        Some(device) => tracing::info!(device = %device, "bluetooth_default_mic_replaced"),
        None => tracing::warn!(
            device = %replacement.name,
            "bluetooth_default_mic_replacement_unavailable"
        ),
    }
    active
}

async fn start_streams(
    myself: &ActorRef<SourceMsg>,
    st: &mut SourceState,
    capture: CaptureSettings,
) -> Result<(), ActorProcessingErr> {
    let mode = st.current_mode;
    let myself2 = myself.clone();
    let mic_muted = st.mic_muted.clone();
    let mic_device = st.active_mic_device.clone();
    let audio = st.audio.clone();
    let (frame_tx, frame_rx) = tokio::sync::mpsc::channel(CAPTURE_FRAME_QUEUE_CAPACITY);
    let wake_pending = st.capture_wake_pending.clone();
    wake_pending.store(false, Ordering::Release);
    st.capture_frames = Some(frame_rx);

    let stream_cancel_token = CancellationToken::new();
    st.stream_cancel_token = Some(stream_cancel_token.clone());

    let handle = tokio::spawn(async move {
        let ctx = StreamContext {
            actor: myself2,
            cancel_token: stream_cancel_token,
            mic_muted,
            mic_device,
            enable_aec: capture.enable_aec,
            audio,
            frame_tx,
            wake_pending,
        };

        run_stream_loop(ctx, mode).await;
    });

    st.run_task = Some(handle);
    Ok(())
}

struct StreamContext {
    actor: ActorRef<SourceMsg>,
    cancel_token: CancellationToken,
    mic_muted: std::sync::Arc<std::sync::atomic::AtomicBool>,
    mic_device: Option<String>,
    enable_aec: bool,
    audio: std::sync::Arc<dyn AudioProvider>,
    frame_tx: tokio::sync::mpsc::Sender<SourceFrame>,
    wake_pending: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl StreamContext {
    fn report_failure(&self, reason: impl Into<String>) {
        let _ = self.actor.cast(SourceMsg::StreamFailed(reason.into()));
    }

    fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }
}

enum StreamResult {
    Continue,
    Stop,
}

async fn run_stream_loop(ctx: StreamContext, mode: ChannelMode) {
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    if mode == ChannelMode::MicOnly {
        return;
    }

    let sample_rate = crate::actors::SAMPLE_RATE;
    let chunk_size = chunk_size_for_stt(sample_rate);

    let capture_result: Result<CaptureStream, _> = match mode {
        ChannelMode::MicAndSpeaker => {
            let config = CaptureConfig {
                sample_rate,
                chunk_size,
                mic_device: ctx.mic_device.clone(),
                enable_aec: ctx.enable_aec,
            };
            ctx.audio.open_capture(config)
        }
        ChannelMode::SpeakerOnly => ctx.audio.open_speaker_capture(sample_rate, chunk_size),
        ChannelMode::MicOnly => {
            ctx.audio
                .open_mic_capture(ctx.mic_device.clone(), sample_rate, chunk_size)
        }
    };

    let mut capture_stream = match capture_result {
        Ok(stream) => stream,
        Err(error) => {
            ctx.report_failure(error.to_string());
            return;
        }
    };

    loop {
        let result = tokio::select! {
            _ = ctx.cancel_token.cancelled() => StreamResult::Stop,
            item = capture_stream.next() => handle_capture_item(&ctx, item).await
        };

        if matches!(result, StreamResult::Stop) {
            return;
        }
    }
}

async fn handle_capture_item(
    ctx: &StreamContext,
    item: Option<Result<CaptureFrame, anlg_audio::Error>>,
) -> StreamResult {
    match item {
        Some(Ok(frame)) => {
            let frame = SourceFrame {
                capture: frame,
                mic_muted: ctx.mic_muted.load(std::sync::atomic::Ordering::Relaxed),
            };

            let send_result = tokio::select! {
                _ = ctx.cancel_token.cancelled() => return StreamResult::Stop,
                result = ctx.frame_tx.send(frame) => result,
            };
            if send_result.is_err() {
                if !ctx.is_cancelled() {
                    tracing::debug!("capture_frame_queue_closed");
                }
                return StreamResult::Stop;
            }

            if !ctx.wake_pending.swap(true, Ordering::AcqRel)
                && ctx.actor.cast(SourceMsg::CaptureFramesReady).is_err()
            {
                if !ctx.is_cancelled() {
                    tracing::debug!("failed_to_schedule_capture_frames");
                }
                return StreamResult::Stop;
            }
            StreamResult::Continue
        }
        Some(Err(error)) => {
            tracing::error!(error.message = %error, "capture_stream_failed");
            ctx.report_failure(error.to_string());
            StreamResult::Stop
        }
        None => {
            ctx.report_failure("capture stream ended unexpectedly");
            StreamResult::Stop
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CaptureSettings, resolve_capture_settings};

    #[test]
    fn headphones_disable_aec_and_isolate_the_mic() {
        assert_eq!(
            resolve_capture_settings(false, true, false),
            CaptureSettings {
                enable_aec: false,
                headphone_output: true,
                mic_isolated: true,
            }
        );
    }

    #[test]
    fn speakers_keep_aec_and_leave_the_mic_shared() {
        assert_eq!(
            resolve_capture_settings(false, false, false),
            CaptureSettings {
                enable_aec: true,
                headphone_output: false,
                mic_isolated: false,
            }
        );
    }

    #[test]
    fn no_aec_override_does_not_imply_isolation() {
        assert_eq!(
            resolve_capture_settings(true, false, false),
            CaptureSettings {
                enable_aec: false,
                headphone_output: false,
                mic_isolated: false,
            }
        );
    }

    #[test]
    fn swapped_mic_is_not_isolated_but_headphones_still_skip_aec() {
        assert_eq!(
            resolve_capture_settings(false, true, true),
            CaptureSettings {
                enable_aec: false,
                headphone_output: true,
                mic_isolated: false,
            }
        );
    }
}
