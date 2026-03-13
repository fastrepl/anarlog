use futures_util::StreamExt;
use ractor::{ActorProcessingErr, ActorRef};
use tokio_util::sync::CancellationToken;

use crate::{SessionProgressEvent, actors::ChannelMode};
use hypr_audio::{AudioInput, CaptureConfig, CaptureFrame, CaptureStream};
use hypr_audio_utils::chunk_size_for_stt;

use super::{SourceFrame, SourceMsg, SourceState};

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

    let stream_cancel_token = CancellationToken::new();
    st.stream_cancel_token = Some(stream_cancel_token.clone());

    let handle = tokio::spawn(async move {
        let ctx = StreamContext {
            actor: myself2,
            cancel_token: stream_cancel_token,
            mic_muted,
            mic_device,
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
                enable_aec: std::env::var("NO_AEC").as_deref() != Ok("1"),
            };
            AudioInput::from_mic_and_speaker(config)
        }
        ChannelMode::SpeakerOnly => AudioInput::from_speaker_capture(sample_rate, chunk_size),
        ChannelMode::MicOnly => {
            AudioInput::from_mic_capture(ctx.mic_device.clone(), sample_rate, chunk_size)
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
            item = capture_stream.next() => handle_capture_item(&ctx, item)
        };

        if matches!(result, StreamResult::Stop) {
            return;
        }
    }
}

fn handle_capture_item(
    ctx: &StreamContext,
    item: Option<Result<CaptureFrame, hypr_audio::Error>>,
) -> StreamResult {
    match item {
        Some(Ok(frame)) => {
            let frame = SourceFrame {
                capture: frame,
                mic_muted: ctx.mic_muted.load(std::sync::atomic::Ordering::Relaxed),
            };
            if ctx.actor.cast(SourceMsg::Frame(frame)).is_err() {
                if !ctx.is_cancelled() {
                    tracing::debug!("failed_to_cast_capture_frame");
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
        None => StreamResult::Stop,
    }
}
