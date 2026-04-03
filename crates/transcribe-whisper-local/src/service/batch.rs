use std::io::Write;
use std::path::Path;

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use hypr_model_manager::ModelManager;
use hypr_transcribe_core::{
    ProgressTracker, batch_sse_response, channel_duration_sec, chunk_channel_audio,
    initial_resolved_until, json_error_response, next_resolved_until, split_resampled_channels,
};
use owhisper_interface::ListenParams;
use owhisper_interface::batch;
use owhisper_interface::batch_sse::BatchSseMessage;
use rodio::Source;
use tokio::sync::mpsc;

use super::response::{TranscriptKind, build_batch_words, build_transcript_response};
use super::{TARGET_SAMPLE_RATE, build_metadata, build_model, transcribe_chunk};

pub(super) async fn handle_batch(
    body: Bytes,
    content_type: &str,
    params: &ListenParams,
    manager: &ModelManager<hypr_whisper_local::LoadedWhisper>,
    model_path: &Path,
) -> Response {
    let model = match manager.get(None).await {
        Ok(model) => model,
        Err(error) => {
            tracing::error!(error = %error, "failed_to_load_model");
            return json_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "model_load_failed",
                error.to_string(),
            );
        }
    };

    let model = model.clone();
    let model_path = model_path.to_path_buf();
    let content_type = content_type.to_string();
    let params = params.clone();

    match tokio::task::spawn_blocking(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            transcribe_batch(
                &body,
                &content_type,
                &params,
                model.as_ref(),
                &model_path,
                None,
            )
        }))
    })
    .await
    {
        Ok(Ok(Ok(response))) => Json(response).into_response(),
        Ok(Ok(Err(error))) => {
            tracing::error!(error = %error, "batch_transcription_failed");
            json_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "transcription_failed",
                error.to_string(),
            )
        }
        Ok(Err(_)) | Err(_) => json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "transcription_failed",
            "task panicked",
        ),
    }
}

pub(super) async fn handle_batch_sse(
    body: Bytes,
    content_type: &str,
    params: &ListenParams,
    manager: &ModelManager<hypr_whisper_local::LoadedWhisper>,
    model_path: &Path,
) -> Response {
    let model = match manager.get(None).await {
        Ok(model) => model,
        Err(error) => {
            tracing::error!(error = %error, "failed_to_load_model");
            return json_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "model_load_failed",
                error.to_string(),
            );
        }
    };

    let model = model.clone();
    let model_path = model_path.to_path_buf();
    let content_type = content_type.to_string();
    let params = params.clone();
    let (event_tx, event_rx) = mpsc::unbounded_channel::<BatchSseMessage>();

    tokio::task::spawn_blocking(move || {
        let message = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            transcribe_batch(
                &body,
                &content_type,
                &params,
                model.as_ref(),
                &model_path,
                Some(event_tx.clone()),
            )
        })) {
            Ok(Ok(response)) => BatchSseMessage::Result { response },
            Ok(Err(error)) => BatchSseMessage::Error {
                error: "transcription_failed".to_string(),
                detail: error.to_string(),
            },
            Err(_) => BatchSseMessage::Error {
                error: "transcription_failed".to_string(),
                detail: "task panicked".to_string(),
            },
        };

        let _ = event_tx.send(message);
    });

    batch_sse_response(event_rx)
}

fn transcribe_batch(
    audio_data: &[u8],
    content_type: &str,
    params: &ListenParams,
    loaded_model: &hypr_whisper_local::LoadedWhisper,
    model_path: &Path,
    event_tx: Option<mpsc::UnboundedSender<BatchSseMessage>>,
) -> Result<batch::Response, crate::Error> {
    let extension = hypr_audio_utils::content_type_to_extension(content_type);
    let mut temp_file = tempfile::Builder::new()
        .prefix("whisper_local_batch_")
        .suffix(&format!(".{}", extension))
        .tempfile()?;

    temp_file.write_all(audio_data)?;
    temp_file.flush()?;

    let source = hypr_audio_utils::source_from_path(temp_file.path())?;
    transcribe_source(source, params, loaded_model, model_path, event_tx)
}

pub(super) fn transcribe_recorded_file(
    loaded_model: &hypr_whisper_local::LoadedWhisper,
    model_path: &Path,
    audio_path: &Path,
) -> Result<Vec<owhisper_interface::Word2>, crate::Error> {
    let source = hypr_audio_utils::source_from_path(audio_path)?;
    let response = transcribe_source(
        source,
        &ListenParams::default(),
        loaded_model,
        model_path,
        None,
    )?;
    let words = response
        .results
        .channels
        .into_iter()
        .flat_map(|channel| channel.alternatives.into_iter())
        .flat_map(|alt| alt.words.into_iter())
        .map(|word| owhisper_interface::Word2 {
            text: word.punctuated_word.unwrap_or(word.word),
            speaker: word
                .speaker
                .map(|speaker| owhisper_interface::SpeakerIdentity::Unassigned {
                    index: speaker as u8,
                }),
            confidence: Some(word.confidence as f32),
            start_ms: Some((word.start * 1000.0) as u64),
            end_ms: Some((word.end * 1000.0) as u64),
        })
        .collect();
    Ok(words)
}

fn transcribe_source<S>(
    source: S,
    params: &ListenParams,
    loaded_model: &hypr_whisper_local::LoadedWhisper,
    model_path: &Path,
    event_tx: Option<mpsc::UnboundedSender<BatchSseMessage>>,
) -> Result<batch::Response, crate::Error>
where
    S: Source<Item = f32>,
{
    let channel_count = u16::from(source.channels()).max(1) as usize;
    let resampled = hypr_audio_utils::resample_audio(source, TARGET_SAMPLE_RATE)?;
    let channel_samples = split_resampled_channels(&resampled, channel_count);
    let total_duration = channel_samples
        .iter()
        .map(|samples| channel_duration_sec(samples))
        .fold(0.0_f64, f64::max);

    let metadata = build_metadata(model_path);
    let mut model = build_model(loaded_model, params)?;
    let channel_durations = channel_samples
        .iter()
        .map(|samples| channel_duration_sec(samples))
        .collect::<Vec<_>>();
    let channel_chunks = channel_samples
        .iter()
        .map(|samples| chunk_channel_audio::<crate::Error>(samples))
        .collect::<Result<Vec<_>, _>>()?;
    let resolved_until = channel_chunks
        .iter()
        .zip(channel_durations.iter().copied())
        .map(|(chunks, channel_duration)| initial_resolved_until(chunks, channel_duration))
        .collect::<Vec<_>>();
    let mut response_channels = Vec::with_capacity(channel_chunks.len().max(1));
    let mut progress = ProgressTracker::new(resolved_until, total_duration, event_tx);
    progress.emit(None);

    for (channel_idx, chunks) in channel_chunks.iter().enumerate() {
        let channel_index = [channel_idx as i32, channel_chunks.len() as i32];
        let channel_duration = channel_durations[channel_idx];

        let (words, transcript, avg_confidence) = transcribe_chunks(
            channel_idx,
            chunks,
            channel_duration,
            &mut model,
            &mut progress,
            &metadata,
            &channel_index,
        )?;

        response_channels.push(batch::Channel {
            alternatives: vec![batch::Alternatives {
                transcript,
                confidence: avg_confidence,
                words,
            }],
        });
    }

    let mut metadata_json = serde_json::to_value(&metadata).unwrap_or_default();
    if let Some(obj) = metadata_json.as_object_mut() {
        obj.insert("duration".to_string(), serde_json::json!(total_duration));
        obj.insert(
            "channels".to_string(),
            serde_json::json!(response_channels.len()),
        );
    }

    Ok(batch::Response {
        metadata: metadata_json,
        results: batch::Results {
            channels: response_channels,
        },
    })
}

fn transcribe_chunks(
    channel_idx: usize,
    chunks: &[hypr_audio_chunking::AudioChunk],
    channel_duration: f64,
    model: &mut hypr_whisper_local::Whisper,
    progress: &mut ProgressTracker,
    metadata: &owhisper_interface::stream::Metadata,
    channel_index: &[i32],
) -> Result<(Vec<batch::Word>, String, f64), crate::Error> {
    let mut all_words = Vec::new();
    let mut all_segments = Vec::new();
    let mut cumulative_confidence = 0.0;
    let mut segment_count = 0usize;

    for (chunk_idx, chunk) in chunks.iter().enumerate() {
        let chunk_start_sec = chunk.sample_start as f64 / TARGET_SAMPLE_RATE as f64;
        progress.update_channel(channel_idx, chunk_start_sec);

        let segments = transcribe_chunk(model, &chunk.samples, chunk_start_sec)?;
        for segment in segments {
            cumulative_confidence += segment.confidence;
            segment_count += 1;
            all_words.extend(build_batch_words(&segment, channel_idx as i32));

            if let Some(tx) = progress.event_tx() {
                let _ = tx.send(BatchSseMessage::Segment {
                    response: build_transcript_response(
                        &segment,
                        TranscriptKind::Confirmed,
                        metadata,
                        channel_index,
                    ),
                });
            }

            all_segments.push(segment);
        }

        progress.update_channel(
            channel_idx,
            next_resolved_until(chunks, chunk_idx, channel_duration),
        );
        progress.emit(Some(join_transcript(&all_segments)));
    }

    let avg_confidence = if segment_count == 0 {
        0.0
    } else {
        cumulative_confidence / segment_count as f64
    };

    Ok((all_words, join_transcript(&all_segments), avg_confidence))
}

fn join_transcript(segments: &[crate::service::Segment]) -> String {
    segments
        .iter()
        .map(|segment| segment.text.as_str())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
