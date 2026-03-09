use std::pin::Pin;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, Stream, StreamExt};
use owhisper_interface::stream::{Metadata, StreamResponse};
use owhisper_interface::{ControlMessage, ListenParams};

use hypr_ws_utils::ConnectionGuard;

use super::message::{AudioExtract, IncomingMessage, process_incoming_message};
use super::response::{
    WsSender, build_transcript_response, format_timestamp_now, send_ws, send_ws_best_effort,
};

pub(super) const SAMPLE_RATE: u32 = 16_000;

#[derive(Default)]
struct ChannelState {
    last_confirmed_sent: String,
    last_pending_sent: String,
    audio_offset: f64,
    segment_start: f64,
    speech_started: bool,
    pending_text: String,
    pending_language: Option<String>,
    pending_confidence: f64,
    pending_cloud_job_id: u64,
    cloud_handoff_segment_start: f64,
}

enum LoopAction {
    Continue,
    StopReceivingInput,
    Break(SessionExit),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionExit {
    Clean,
    Replaced,
    Error,
    TransportClosed,
}

impl SessionExit {
    fn should_emit_terminal(self) -> bool {
        matches!(self, Self::Clean)
    }
}

type TaggedEvent = (
    usize,
    Result<hypr_cactus::TranscribeEvent, hypr_cactus::Error>,
);

pub(super) async fn handle_websocket(
    socket: WebSocket,
    params: ListenParams,
    model: Arc<hypr_cactus::Model>,
    metadata: Metadata,
    cactus_config: crate::CactusConfig,
    guard: ConnectionGuard,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    let total_channels = (params.channels as i32).max(1) as usize;
    let chunk_size_ms = 300;

    let options = hypr_cactus::TranscribeOptions {
        language: hypr_cactus::constrain_to(&params.languages),
        min_chunk_size: Some((cactus_config.min_chunk_sec * SAMPLE_RATE as f32) as u32),
        ..Default::default()
    };

    type TaggedStream = Pin<Box<dyn Stream<Item = TaggedEvent> + Send>>;

    let mut audio_txs: Option<Vec<tokio::sync::mpsc::Sender<Vec<f32>>>> =
        Some(Vec::with_capacity(total_channels));
    let mut cancel_tokens = Vec::with_capacity(total_channels);
    let mut event_streams: futures_util::stream::SelectAll<TaggedStream> =
        futures_util::stream::SelectAll::new();

    for ch_idx in 0..total_channels {
        let cloud_config = cactus_config.cloud.clone();
        let session = hypr_cactus::transcribe_stream(
            model.clone(),
            options.clone(),
            cloud_config,
            chunk_size_ms,
            SAMPLE_RATE,
        );
        audio_txs.as_mut().unwrap().push(session.audio_tx().clone());
        cancel_tokens.push(session.cancellation_token().clone());
        event_streams.push(Box::pin(session.map(move |e| (ch_idx, e))));
    }

    let mut channel_states: Vec<ChannelState> = (0..total_channels)
        .map(|_| ChannelState::default())
        .collect();
    let mut receiving_input = true;

    let exit = loop {
        let action = tokio::select! {
            _ = guard.cancelled() => {
                tracing::info!("cactus_websocket_cancelled_by_new_connection");
                for ct in &cancel_tokens {
                    ct.cancel();
                }
                LoopAction::Break(SessionExit::Replaced)
            }
            event = event_streams.next() => {
                handle_transcribe_event(
                    &mut ws_sender, event, &mut channel_states, total_channels, &metadata,
                ).await
            }
            msg = ws_receiver.next(), if receiving_input => {
                handle_ws_message(
                    &mut ws_sender, msg, params.channels, audio_txs.as_deref().unwrap_or(&[]),
                    &mut channel_states, total_channels, &metadata,
                ).await
            }
        };
        match action {
            LoopAction::Continue => {}
            LoopAction::StopReceivingInput => {
                receiving_input = false;
                drop(audio_txs.take());
            }
            LoopAction::Break(exit) => break exit,
        }
    };

    drop(audio_txs);
    drop(event_streams);

    let total_audio_offset = channel_states.first().map_or(0.0, |s| s.audio_offset);

    if exit.should_emit_terminal() {
        send_ws_best_effort(
            &mut ws_sender,
            &StreamResponse::TerminalResponse {
                request_id: metadata.request_id.clone(),
                created: format_timestamp_now(),
                duration: total_audio_offset,
                channels: total_channels as u32,
            },
        )
        .await;
    }

    let _ = ws_sender.close().await;
}

async fn handle_transcribe_event(
    ws_sender: &mut WsSender,
    event: Option<TaggedEvent>,
    channel_states: &mut [ChannelState],
    total_channels: usize,
    metadata: &Metadata,
) -> LoopAction {
    let Some((ch_idx, event)) = event else {
        return LoopAction::Break(SessionExit::Clean);
    };

    match event {
        Err(e) => {
            send_ws_best_effort(
                ws_sender,
                &StreamResponse::ErrorResponse {
                    error_code: None,
                    error_message: e.to_string(),
                    provider: "cactus".to_string(),
                },
            )
            .await;
            LoopAction::Break(SessionExit::Error)
        }
        Ok(hypr_cactus::TranscribeEvent {
            result,
            chunk_duration_secs,
        }) => {
            let channel_index = vec![ch_idx as i32, total_channels as i32];
            let channel_u8 = vec![ch_idx as u8];
            let state = &mut channel_states[ch_idx];

            state.audio_offset += chunk_duration_secs;

            let seg_dur = if result.buffer_duration_ms > 0.0 {
                result.buffer_duration_ms / 1000.0
            } else {
                state.audio_offset - state.segment_start
            };
            let seg_start = (state.audio_offset - seg_dur).max(state.segment_start);

            let confidence = result.confidence as f64;
            let confirmed_text = result.confirmed.trim();

            let metrics = stream_result_metrics(&result);

            state.pending_text = result.pending.clone();
            state.pending_language = result.language.clone();
            state.pending_confidence = confidence;

            if result.cloud_handoff && result.cloud_job_id != 0 {
                state.pending_cloud_job_id = result.cloud_job_id;
                state.cloud_handoff_segment_start = state.segment_start;
            }

            if result.cloud_result_job_id != 0 && !result.cloud_result.is_empty() {
                let cloud_text = result.cloud_result.trim();
                let job_id = result.cloud_result_job_id;
                let seg_start = state.cloud_handoff_segment_start;
                let seg_duration = state.audio_offset - seg_start;
                let mut keys = metrics.clone();
                keys.insert("cloud_corrected".to_string(), serde_json::Value::Bool(true));
                keys.insert(
                    "cloud_job_id".to_string(),
                    serde_json::Value::Number(job_id.into()),
                );
                tracing::info!(
                    hyprnote.transcript.text = cloud_text,
                    hyprnote.stt.job.id = job_id,
                    hyprnote.audio.channel_index = ch_idx,
                    "cactus_cloud_correction"
                );
                if !send_ws(
                    ws_sender,
                    &build_transcript_response(
                        cloud_text,
                        seg_start,
                        seg_duration,
                        confidence,
                        result.language.as_deref(),
                        true,
                        true,
                        false,
                        metadata,
                        &channel_index,
                        Some(keys),
                    ),
                )
                .await
                {
                    return LoopAction::Break(SessionExit::TransportClosed);
                }
                state.pending_cloud_job_id = 0;
            }

            if !confirmed_text.is_empty() && confirmed_text != state.last_confirmed_sent {
                if !state.speech_started {
                    if !send_ws(
                        ws_sender,
                        &StreamResponse::SpeechStartedResponse {
                            channel: channel_u8.clone(),
                            timestamp: seg_start,
                        },
                    )
                    .await
                    {
                        return LoopAction::Break(SessionExit::TransportClosed);
                    }
                }

                let handoff_extra = {
                    let mut keys = metrics.clone();
                    if result.cloud_handoff && result.cloud_job_id != 0 {
                        keys.insert("cloud_handoff".to_string(), serde_json::Value::Bool(true));
                        keys.insert(
                            "cloud_job_id".to_string(),
                            serde_json::Value::Number(result.cloud_job_id.into()),
                        );
                    }
                    Some(keys)
                };

                tracing::info!(
                    hyprnote.transcript.text = confirmed_text,
                    hyprnote.audio.channel_index = ch_idx,
                    "cactus_confirmed_text"
                );
                if !send_ws(
                    ws_sender,
                    &build_transcript_response(
                        confirmed_text,
                        seg_start,
                        seg_dur,
                        confidence,
                        result.language.as_deref(),
                        true,
                        true,
                        false,
                        metadata,
                        &channel_index,
                        handoff_extra,
                    ),
                )
                .await
                {
                    return LoopAction::Break(SessionExit::TransportClosed);
                }
                if !send_ws(
                    ws_sender,
                    &StreamResponse::UtteranceEndResponse {
                        channel: channel_u8,
                        last_word_end: state.audio_offset,
                    },
                )
                .await
                {
                    return LoopAction::Break(SessionExit::TransportClosed);
                }

                state.last_confirmed_sent.clear();
                state.last_confirmed_sent.push_str(confirmed_text);
                state.last_pending_sent.clear();
                state.segment_start = state.audio_offset;
                state.speech_started = false;
                return LoopAction::Continue;
            }

            let pending_text = result.pending.trim();
            if pending_text.is_empty()
                || pending_text == state.last_pending_sent
                || pending_text == state.last_confirmed_sent
            {
                return LoopAction::Continue;
            }

            if !state.speech_started {
                state.speech_started = true;
                if !send_ws(
                    ws_sender,
                    &StreamResponse::SpeechStartedResponse {
                        channel: channel_u8.clone(),
                        timestamp: seg_start,
                    },
                )
                .await
                {
                    return LoopAction::Break(SessionExit::TransportClosed);
                }
            }

            let pending_handoff_extra = {
                let mut keys = metrics;
                if result.cloud_handoff && result.cloud_job_id != 0 {
                    keys.insert("cloud_handoff".to_string(), serde_json::Value::Bool(true));
                    keys.insert(
                        "cloud_job_id".to_string(),
                        serde_json::Value::Number(result.cloud_job_id.into()),
                    );
                }
                Some(keys)
            };

            if !send_ws(
                ws_sender,
                &build_transcript_response(
                    pending_text,
                    seg_start,
                    seg_dur,
                    confidence,
                    result.language.as_deref(),
                    false,
                    false,
                    false,
                    metadata,
                    &channel_index,
                    pending_handoff_extra,
                ),
            )
            .await
            {
                return LoopAction::Break(SessionExit::TransportClosed);
            }
            state.last_pending_sent.clear();
            state.last_pending_sent.push_str(pending_text);
            LoopAction::Continue
        }
    }
}

async fn handle_ws_message(
    ws_sender: &mut WsSender,
    msg: Option<Result<Message, axum::Error>>,
    channels: u8,
    audio_txs: &[tokio::sync::mpsc::Sender<Vec<f32>>],
    channel_states: &mut [ChannelState],
    total_channels: usize,
    metadata: &Metadata,
) -> LoopAction {
    let Some(msg) = msg else {
        tracing::info!("websocket_stream_ended");
        return LoopAction::StopReceivingInput;
    };
    let msg = match msg {
        Ok(msg) => msg,
        Err(e) => {
            tracing::warn!("websocket_receive_error: {}", e);
            send_ws_best_effort(
                ws_sender,
                &StreamResponse::ErrorResponse {
                    error_code: None,
                    error_message: format!("websocket receive error: {e}"),
                    provider: "cactus".to_string(),
                },
            )
            .await;
            return LoopAction::Break(SessionExit::Error);
        }
    };

    match process_incoming_message(&msg, channels) {
        Ok(IncomingMessage::Audio(AudioExtract::Mono(s))) if !s.is_empty() => {
            if audio_txs[0].send(s).await.is_err() {
                send_ws_best_effort(
                    ws_sender,
                    &StreamResponse::ErrorResponse {
                        error_code: None,
                        error_message: "audio pipeline closed unexpectedly".to_string(),
                        provider: "cactus".to_string(),
                    },
                )
                .await;
                return LoopAction::Break(SessionExit::Error);
            }
        }
        Ok(IncomingMessage::Audio(AudioExtract::Dual { ch0, ch1 })) => {
            if audio_txs.len() >= 2 {
                if audio_txs[0].send(ch0).await.is_err() || audio_txs[1].send(ch1).await.is_err() {
                    send_ws_best_effort(
                        ws_sender,
                        &StreamResponse::ErrorResponse {
                            error_code: None,
                            error_message: "audio pipeline closed unexpectedly".to_string(),
                            provider: "cactus".to_string(),
                        },
                    )
                    .await;
                    return LoopAction::Break(SessionExit::Error);
                }
            } else {
                let mixed = hypr_audio_utils::mix_audio_f32(&ch0, &ch1);
                if !mixed.is_empty() && audio_txs[0].send(mixed).await.is_err() {
                    send_ws_best_effort(
                        ws_sender,
                        &StreamResponse::ErrorResponse {
                            error_code: None,
                            error_message: "audio pipeline closed unexpectedly".to_string(),
                            provider: "cactus".to_string(),
                        },
                    )
                    .await;
                    return LoopAction::Break(SessionExit::Error);
                }
            }
        }
        Ok(IncomingMessage::Audio(AudioExtract::End)) => {
            return LoopAction::StopReceivingInput;
        }
        Ok(IncomingMessage::Control(ControlMessage::KeepAlive)) => {}
        Ok(IncomingMessage::Control(ControlMessage::Finalize)) => {
            if handle_finalize(ws_sender, channel_states, total_channels, metadata).await {
                return LoopAction::Break(SessionExit::TransportClosed);
            }
        }
        Ok(IncomingMessage::Control(ControlMessage::CloseStream)) => {
            return LoopAction::StopReceivingInput;
        }
        Ok(_) => {}
        Err(error) => {
            send_ws_best_effort(
                ws_sender,
                &StreamResponse::ErrorResponse {
                    error_code: None,
                    error_message: error.to_string(),
                    provider: "cactus".to_string(),
                },
            )
            .await;
            return LoopAction::Break(SessionExit::Error);
        }
    }

    LoopAction::Continue
}

fn stream_result_metrics(
    result: &hypr_cactus::StreamResult,
) -> std::collections::HashMap<String, serde_json::Value> {
    let mut m = std::collections::HashMap::new();
    m.insert(
        "decode_tps".to_string(),
        serde_json::json!(result.decode_tps),
    );
    m.insert(
        "prefill_tps".to_string(),
        serde_json::json!(result.prefill_tps),
    );
    m.insert(
        "time_to_first_token_ms".to_string(),
        serde_json::json!(result.time_to_first_token_ms),
    );
    m.insert(
        "total_time_ms".to_string(),
        serde_json::json!(result.total_time_ms),
    );
    m.insert(
        "decode_tokens".to_string(),
        serde_json::json!(result.decode_tokens),
    );
    m.insert(
        "prefill_tokens".to_string(),
        serde_json::json!(result.prefill_tokens),
    );
    m.insert(
        "total_tokens".to_string(),
        serde_json::json!(result.total_tokens),
    );
    m.insert(
        "buffer_duration_ms".to_string(),
        serde_json::json!(result.buffer_duration_ms),
    );
    m
}

async fn handle_finalize(
    ws_sender: &mut WsSender,
    channel_states: &mut [ChannelState],
    total_channels: usize,
    metadata: &Metadata,
) -> bool {
    for ch_idx in 0..total_channels {
        let (pending_text, pending_confidence, pending_language, segment_start, audio_offset) = {
            let state = &channel_states[ch_idx];
            (
                state.pending_text.trim().to_string(),
                state.pending_confidence,
                state.pending_language.clone(),
                state.segment_start,
                state.audio_offset,
            )
        };
        if !pending_text.is_empty() {
            let channel_index = vec![ch_idx as i32, total_channels as i32];
            let channel_u8 = vec![ch_idx as u8];
            let duration = audio_offset - segment_start;
            if !send_ws(
                ws_sender,
                &build_transcript_response(
                    &pending_text,
                    segment_start,
                    duration,
                    pending_confidence,
                    pending_language.as_deref(),
                    true,
                    true,
                    true,
                    metadata,
                    &channel_index,
                    None,
                ),
            )
            .await
            {
                return true;
            }
            if !send_ws(
                ws_sender,
                &StreamResponse::UtteranceEndResponse {
                    channel: channel_u8,
                    last_word_end: segment_start + duration,
                },
            )
            .await
            {
                return true;
            }
        }
    }
    for state in channel_states.iter_mut() {
        state.segment_start = state.audio_offset;
        state.speech_started = false;
        state.last_confirmed_sent.clear();
        state.last_pending_sent.clear();
        state.pending_text.clear();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::SessionExit;

    #[test]
    fn terminal_metadata_only_on_clean_exit() {
        assert!(SessionExit::Clean.should_emit_terminal());
        assert!(!SessionExit::Error.should_emit_terminal());
        assert!(!SessionExit::Replaced.should_emit_terminal());
        assert!(!SessionExit::TransportClosed.should_emit_terminal());
    }
}
