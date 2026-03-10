use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use futures_util::StreamExt;
use ractor::{ActorProcessingErr, ActorRef};
use tokio_util::sync::CancellationToken;

use crate::{
    SessionProgressEvent,
    actors::{AudioChunk, ChannelMode},
};
use hypr_audio::AudioInput;
use hypr_audio_utils::chunk_size_for_stt;
use hypr_resampler::ResampleExtDynamicNew;

use super::{SourceMsg, SourceState};

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
    mic_muted: Arc<AtomicBool>,
    mic_device: Option<String>,
}

impl StreamContext {
    fn report_failure(&self, reason: &str) {
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

    let mic_stream = if mode.uses_mic() {
        match setup_mic_stream(&ctx) {
            Ok(stream) => Some(stream),
            Err(()) => return,
        }
    } else {
        None
    };

    if mode == ChannelMode::MicAndSpeaker {
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    let spk_stream = if mode.uses_speaker() {
        match setup_speaker_stream(&ctx) {
            Ok(stream) => Some(stream),
            Err(()) => return,
        }
    } else {
        None
    };

    tokio::pin!(mic_stream);
    tokio::pin!(spk_stream);

    loop {
        let result = tokio::select! {
            _ = ctx.cancel_token.cancelled() => StreamResult::Stop,
            item = async { mic_stream.as_mut().as_pin_mut()?.next().await }, if mic_stream.is_some() => {
                handle_mic_item(&ctx, item)
            }
            item = async { spk_stream.as_mut().as_pin_mut()?.next().await }, if spk_stream.is_some() => {
                handle_speaker_item(&ctx, item)
            }
        };

        if matches!(result, StreamResult::Stop) {
            return;
        }
    }
}

fn setup_mic_stream(
    ctx: &StreamContext,
) -> Result<impl futures_util::Stream<Item = Result<Vec<f32>, hypr_resampler::Error>>, ()> {
    let mut mic_input = match AudioInput::from_mic(ctx.mic_device.clone()) {
        Ok(input) => input,
        Err(err) => {
            tracing::error!(
                error.message = ?err,
                hyprnote.audio.device = ?ctx.mic_device,
                "mic_open_failed"
            );
            ctx.report_failure("mic_open_failed");
            return Err(());
        }
    };

    let chunk_size = chunk_size_for_stt(crate::actors::SAMPLE_RATE);
    match mic_input
        .stream()
        .resampled_chunks(crate::actors::SAMPLE_RATE, chunk_size)
    {
        Ok(stream) => Ok(stream),
        Err(err) => {
            tracing::error!(
                error.message = ?err,
                hyprnote.audio.device = ?ctx.mic_device,
                "mic_stream_setup_failed"
            );
            ctx.report_failure("mic_stream_setup_failed");
            Err(())
        }
    }
}

fn setup_speaker_stream(
    ctx: &StreamContext,
) -> Result<impl futures_util::Stream<Item = Result<Vec<f32>, hypr_resampler::Error>>, ()> {
    let mut spk_input = hypr_audio::AudioInput::from_speaker();
    let chunk_size = chunk_size_for_stt(crate::actors::SAMPLE_RATE);
    match spk_input
        .stream()
        .resampled_chunks(crate::actors::SAMPLE_RATE, chunk_size)
    {
        Ok(stream) => Ok(stream),
        Err(err) => {
            tracing::error!(error.message = ?err, "speaker_stream_setup_failed");
            ctx.report_failure("speaker_stream_setup_failed");
            Err(())
        }
    }
}

fn handle_mic_item(
    ctx: &StreamContext,
    item: Option<Result<Vec<f32>, hypr_resampler::Error>>,
) -> StreamResult {
    match item {
        Some(Ok(data)) => {
            let output_data = if ctx.mic_muted.load(Ordering::Relaxed) {
                vec![0.0; data.len()]
            } else {
                data
            };
            if ctx
                .actor
                .cast(SourceMsg::MicChunk(AudioChunk { data: output_data }))
                .is_err()
            {
                if !ctx.is_cancelled() {
                    tracing::debug!("failed_to_cast_mic_chunk");
                }
                return StreamResult::Stop;
            }
            StreamResult::Continue
        }
        Some(Err(err)) => {
            tracing::error!(
                error.message = ?err,
                hyprnote.audio.device = ?ctx.mic_device,
                "mic_resample_failed"
            );
            ctx.report_failure("mic_resample_failed");
            StreamResult::Stop
        }
        None => {
            if !ctx.is_cancelled() {
                tracing::error!(hyprnote.audio.device = ?ctx.mic_device, "mic_stream_ended");
                ctx.report_failure("mic_stream_ended");
            }
            StreamResult::Stop
        }
    }
}

fn handle_speaker_item(
    ctx: &StreamContext,
    item: Option<Result<Vec<f32>, hypr_resampler::Error>>,
) -> StreamResult {
    match item {
        Some(Ok(data)) => {
            if ctx
                .actor
                .cast(SourceMsg::SpeakerChunk(AudioChunk { data }))
                .is_err()
            {
                if !ctx.is_cancelled() {
                    tracing::debug!("failed_to_cast_speaker_chunk");
                }
                return StreamResult::Stop;
            }
            StreamResult::Continue
        }
        Some(Err(err)) => {
            tracing::error!(error.message = ?err, "speaker_resample_failed");
            ctx.report_failure("speaker_resample_failed");
            StreamResult::Stop
        }
        None => {
            if !ctx.is_cancelled() {
                tracing::error!("speaker_stream_ended");
                ctx.report_failure("speaker_stream_ended");
            }
            StreamResult::Stop
        }
    }
}
