use futures_util::StreamExt;
use ractor::{ActorProcessingErr, ActorRef};
use std::sync::atomic::Ordering;
use tokio_util::sync::CancellationToken;

use crate::{SessionProgressEvent, actors::ChannelMode};
use anlg_audio::{AudioProvider, CaptureConfig, CaptureFrame, CaptureStream};
use anlg_audio_utils::chunk_size_for_stt;

use super::{SourceFrame, SourceMsg, SourceState};

const CAPTURE_FRAME_QUEUE_CAPACITY: usize = 32;

pub(super) async fn start_source_loop(
    myself: &ActorRef<SourceMsg>,
    st: &mut SourceState,
) -> Result<(), ActorProcessingErr> {
    let new_mode = ChannelMode::determine(st.onboarding);

    let mode_changed = st.current_mode != new_mode;
    st.current_mode = new_mode;

    tracing::info!(?new_mode, mode_changed, "start_source_loop");

    st.pipeline.reset();

    let result = start_streams(myself, st).await;

    if result.is_ok() {
        st.runtime.emit_progress(SessionProgressEvent::AudioReady {
            session_id: st.session_id.clone(),
            device: st.mic_device.clone(),
        });
    }

    result
}

async fn start_streams(
    myself: &ActorRef<SourceMsg>,
    st: &mut SourceState,
) -> Result<(), ActorProcessingErr> {
    let mode = st.current_mode;
    let myself2 = myself.clone();
    let mic_muted = st.mic_muted.clone();
    let mic_device = st.mic_device.clone();
    let speaker_device = st.speaker_device.clone();
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
            speaker_device,
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
    speaker_device: Option<String>,
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
                speaker_device: ctx.speaker_device.clone(),
                enable_aec: aec_enabled(),
            };
            ctx.audio.open_capture(config)
        }
        ChannelMode::SpeakerOnly => {
            ctx.audio
                .open_speaker_capture(ctx.speaker_device.clone(), sample_rate, chunk_size)
        }
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

// Headphones keep speaker output out of the mic, so AEC only costs CPU and can degrade near-end
// speech. The source restarts on default output changes, so this is re-evaluated per stream.
fn aec_enabled() -> bool {
    if std::env::var("NO_AEC").as_deref() == Ok("1") {
        return false;
    }

    match anlg_audio_device::default_output_headphone() {
        Some(device) => {
            tracing::info!(
                device = %device.name,
                transport = ?device.transport_type,
                "aec_disabled_headphone_output"
            );
            false
        }
        None => true,
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
