use std::io::Write;
use std::path::Path;

use owhisper_interface::ListenParams;
use owhisper_interface::batch;
use owhisper_interface::batch_sse::BatchSseMessage;
use owhisper_interface::progress::{InferencePhase, InferenceProgress};
use rodio::Source;
use tokio::sync::mpsc;

use super::chunk::{TARGET_SAMPLE_RATE, chunk_mono_audio};
use super::response::{build_batch_words, build_segment_stream_response};
use hypr_audio_utils::content_type_to_extension;

#[tracing::instrument(
    skip(audio_data, event_tx),
    fields(
        hyprnote.audio.size_bytes = audio_data.len(),
        hyprnote.file.mime_type = content_type,
        hyprnote.model.path = %model_path.display()
    )
)]
pub(super) fn transcribe_batch(
    audio_data: &[u8],
    content_type: &str,
    params: &ListenParams,
    model_path: &Path,
    event_tx: Option<mpsc::UnboundedSender<BatchSseMessage>>,
) -> Result<batch::Response, crate::Error> {
    let extension = content_type_to_extension(content_type);
    let mut temp_file = tempfile::Builder::new()
        .prefix("cactus_batch_")
        .suffix(&format!(".{}", extension))
        .tempfile()?;

    temp_file.write_all(audio_data)?;
    temp_file.flush()?;

    let source = hypr_audio_utils::source_from_path(temp_file.path())?;
    let channel_count = source.channels().max(1) as usize;
    let resampled = hypr_audio_utils::resample_audio(source, TARGET_SAMPLE_RATE)?;
    let channel_samples = split_resampled_channels(&resampled, channel_count);
    let total_duration = channel_samples
        .iter()
        .map(|samples| channel_duration_sec(samples))
        .fold(0.0_f64, f64::max);

    let model = match crate::service::build_model(model_path, &params.keywords) {
        Ok(m) => m,
        Err(e) => {
            tracing::error!(error.message = %e, "failed_to_load_model");
            return Err(e.into());
        }
    };

    let options = hypr_cactus::TranscribeOptions {
        language: hypr_cactus::constrain_to(&params.languages),
        ..Default::default()
    };

    let metadata = crate::service::build_metadata(model_path);
    let channel_durations = channel_samples
        .iter()
        .map(|samples| channel_duration_sec(samples))
        .collect::<Vec<_>>();
    let channel_chunks = channel_samples
        .iter()
        .map(|samples| chunk_channel_audio(samples))
        .collect::<Result<Vec<_>, _>>()?;
    let mut resolved_until = channel_chunks
        .iter()
        .zip(channel_durations.iter().copied())
        .map(|(chunks, channel_duration)| initial_resolved_until(chunks, channel_duration))
        .collect::<Vec<_>>();
    let mut response_channels = Vec::with_capacity(channel_samples.len().max(1));
    let mut last_progress = 0.0;

    if let Some(ref tx) = event_tx {
        emit_progress_update(
            tx,
            &mut last_progress,
            overall_resolved_audio(&resolved_until),
            total_duration,
            None,
        );
    }

    for (channel_idx, chunks) in channel_chunks.iter().enumerate() {
        let channel_index = [channel_idx as i32, channel_samples.len() as i32];
        let channel_duration = channel_durations[channel_idx];

        let (all_words, transcript, avg_confidence) = if chunks.is_empty() {
            (vec![], String::new(), 0.0)
        } else {
            transcribe_chunks(
                channel_idx,
                chunks,
                channel_duration,
                &model,
                &options,
                &mut resolved_until,
                total_duration,
                &mut last_progress,
                event_tx.clone(),
                &metadata,
                &channel_index,
            )?
        };

        response_channels.push(batch::Channel {
            alternatives: vec![batch::Alternatives {
                transcript,
                confidence: avg_confidence,
                words: all_words,
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

fn split_resampled_channels(samples: &[f32], channel_count: usize) -> Vec<Vec<f32>> {
    if channel_count <= 1 {
        return vec![samples.to_vec()];
    }

    hypr_audio_utils::deinterleave(samples, channel_count)
}

fn channel_duration_sec(samples: &[f32]) -> f64 {
    samples.len() as f64 / TARGET_SAMPLE_RATE as f64
}

fn chunk_channel_audio(
    samples: &[f32],
) -> Result<Vec<hypr_vad_chunking::AudioChunk>, crate::Error> {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.block_on(chunk_mono_audio(samples)),
        Err(_) => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(chunk_mono_audio(samples)),
    }
}

fn transcribe_chunks(
    channel_idx: usize,
    chunks: &[hypr_vad_chunking::AudioChunk],
    channel_duration: f64,
    model: &hypr_cactus::Model,
    options: &hypr_cactus::TranscribeOptions,
    resolved_until: &mut [f64],
    total_duration: f64,
    last_progress: &mut f64,
    event_tx: Option<mpsc::UnboundedSender<BatchSseMessage>>,
    metadata: &owhisper_interface::stream::Metadata,
    channel_index: &[i32],
) -> Result<(Vec<batch::Word>, String, f64), crate::Error> {
    let mut all_words = Vec::new();
    let mut all_transcripts = Vec::new();
    let mut cumulative_confidence = 0.0;

    for (chunk_idx, chunk) in chunks.iter().enumerate() {
        let pcm_i16 = hypr_audio_utils::f32_to_i16_samples(&chunk.samples);
        let pcm_bytes: Vec<u8> = pcm_i16.iter().flat_map(|s| s.to_le_bytes()).collect();

        let chunk_start_sec = chunk.start_timestamp_ms as f64 / 1000.0;
        let chunk_duration_sec =
            (chunk.end_timestamp_ms - chunk.start_timestamp_ms) as f64 / 1000.0;
        resolved_until[channel_idx] = chunk_start_sec;

        let cactus_response = if let Some(ref tx) = event_tx {
            let tx = tx.clone();
            let completed_text: String = all_transcripts.join(" ");

            model.transcribe_pcm_with_callback(&pcm_bytes, options, |token| {
                let mut partial = completed_text.clone();

                if !token.is_empty() {
                    if !partial.is_empty() {
                        partial.push(' ');
                    }
                    partial.push_str(token);
                }

                emit_progress_update(
                    &tx,
                    last_progress,
                    overall_resolved_with_channel(
                        resolved_until,
                        channel_idx,
                        resolved_audio_for_chunk_progress(
                            chunk_start_sec,
                            chunk_duration_sec,
                            ChunkProgress::Start,
                        ),
                    ),
                    total_duration,
                    Some(partial),
                );

                true
            })?
        } else {
            model.transcribe_pcm(&pcm_bytes, options)?
        };

        let chunk_text = cactus_response.text.trim().to_string();
        if !chunk_text.is_empty() {
            let mut words = build_batch_words(
                &chunk_text,
                chunk_duration_sec,
                cactus_response.confidence as f64,
            );
            for w in &mut words {
                w.start += chunk_start_sec;
                w.end += chunk_start_sec;
            }
            all_words.extend(words);

            if let Some(ref tx) = event_tx {
                let segment_resp = build_segment_stream_response(
                    &chunk_text,
                    chunk_start_sec,
                    chunk_duration_sec,
                    cactus_response.confidence as f64,
                    metadata,
                    channel_index,
                );
                let _ = tx.send(BatchSseMessage::Segment {
                    response: segment_resp,
                });
            }

            all_transcripts.push(chunk_text);
        }

        resolved_until[channel_idx] = next_resolved_until(chunks, chunk_idx, channel_duration);
        if let Some(ref tx) = event_tx {
            emit_progress_update(
                tx,
                last_progress,
                overall_resolved_audio(resolved_until),
                total_duration,
                Some(all_transcripts.join(" ")),
            );
        }

        cumulative_confidence += cactus_response.confidence as f64;
    }

    let transcript = all_transcripts.join(" ");
    let avg_confidence = cumulative_confidence / chunks.len() as f64;

    Ok((all_words, transcript, avg_confidence))
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ChunkProgress {
    Start,
    #[allow(dead_code)]
    WithinChunk(f64),
}

fn resolved_audio_for_chunk_progress(
    chunk_start_sec: f64,
    chunk_duration_sec: f64,
    progress: ChunkProgress,
) -> f64 {
    match progress {
        ChunkProgress::Start => chunk_start_sec,
        ChunkProgress::WithinChunk(progress) => {
            chunk_start_sec + progress.clamp(0.0, 1.0) * chunk_duration_sec
        }
    }
}

fn initial_resolved_until(chunks: &[hypr_vad_chunking::AudioChunk], channel_duration: f64) -> f64 {
    chunks
        .first()
        .map(|chunk| chunk.start_timestamp_ms as f64 / 1000.0)
        .unwrap_or(channel_duration)
}

fn next_resolved_until(
    chunks: &[hypr_vad_chunking::AudioChunk],
    chunk_idx: usize,
    channel_duration: f64,
) -> f64 {
    chunks
        .get(chunk_idx + 1)
        .map(|chunk| chunk.start_timestamp_ms as f64 / 1000.0)
        .unwrap_or(channel_duration)
}

fn overall_resolved_audio(resolved_until: &[f64]) -> f64 {
    resolved_until
        .iter()
        .copied()
        .reduce(f64::min)
        .unwrap_or(0.0)
}

fn overall_resolved_with_channel(resolved_until: &[f64], channel_idx: usize, resolved: f64) -> f64 {
    resolved_until
        .iter()
        .enumerate()
        .map(|(idx, value)| if idx == channel_idx { resolved } else { *value })
        .reduce(f64::min)
        .unwrap_or(resolved)
}

fn record_progress(resolved_audio: f64, total_duration: f64, last_progress: &mut f64) -> f64 {
    let raw = if total_duration > 0.0 {
        (resolved_audio / total_duration).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let progress = raw.max(*last_progress).min(0.99);
    *last_progress = progress;
    progress
}

fn emit_progress_update(
    tx: &mpsc::UnboundedSender<BatchSseMessage>,
    last_progress: &mut f64,
    resolved_audio: f64,
    total_duration: f64,
    partial_text: Option<String>,
) {
    let previous = *last_progress;
    let percentage = record_progress(resolved_audio, total_duration, last_progress);
    if percentage <= previous {
        return;
    }

    let _ = tx.send(BatchSseMessage::Progress {
        progress: InferenceProgress {
            percentage,
            partial_text,
            phase: InferencePhase::Decoding,
        },
    });
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use hypr_language::ISO639;
    use owhisper_interface::ListenParams;

    use super::*;

    #[test]
    fn split_resampled_channels_preserves_stereo() {
        let samples = vec![0.1, 0.9, 0.2, 0.8, 0.3, 0.7];
        let channels = split_resampled_channels(&samples, 2);

        assert_eq!(channels, vec![vec![0.1, 0.2, 0.3], vec![0.9, 0.8, 0.7]]);
    }

    #[test]
    fn split_resampled_channels_keeps_mono() {
        let samples = vec![0.1, 0.2, 0.3];
        let channels = split_resampled_channels(&samples, 1);

        assert_eq!(channels, vec![samples]);
    }

    #[test]
    fn initial_resolved_until_uses_leading_silence() {
        let chunks = vec![hypr_vad_chunking::AudioChunk {
            samples: vec![],
            start_timestamp_ms: 12_000,
            end_timestamp_ms: 15_000,
        }];

        let progress = initial_resolved_until(&chunks, 40.0);

        assert_eq!(progress, 12.0);
    }

    #[test]
    fn initial_resolved_until_marks_empty_channel_complete() {
        let progress = initial_resolved_until(&[], 40.0);

        assert_eq!(progress, 40.0);
    }

    #[test]
    fn overall_resolved_audio_uses_slowest_channel() {
        let resolved = overall_resolved_audio(&[40.0, 18.0, 25.0]);

        assert_eq!(resolved, 18.0);
    }

    #[test]
    fn overall_resolved_with_channel_substitutes_current_channel() {
        let resolved = overall_resolved_with_channel(&[40.0, 10.0], 1, 22.0);

        assert_eq!(resolved, 22.0);
    }

    #[test]
    fn resolved_audio_for_chunk_progress_starts_at_chunk_boundary() {
        let resolved = resolved_audio_for_chunk_progress(12.0, 8.0, ChunkProgress::Start);

        assert_eq!(resolved, 12.0);
    }

    #[test]
    fn resolved_audio_for_chunk_progress_supports_future_intra_chunk_updates() {
        let resolved =
            resolved_audio_for_chunk_progress(12.0, 8.0, ChunkProgress::WithinChunk(0.25));

        assert_eq!(resolved, 14.0);
    }

    #[test]
    fn record_progress_uses_wall_clock_duration() {
        let mut last = 0.0;

        let progress = record_progress(20.0, 40.0, &mut last);

        assert_eq!(progress, 0.5);
        assert_eq!(last, 0.5);
    }

    #[test]
    fn record_progress_stays_monotonic_across_channels() {
        let mut last = 0.75;

        let progress = record_progress(2.0, 20.0, &mut last);

        assert_eq!(progress, 0.75);
        assert_eq!(last, 0.75);
    }

    #[test]
    fn record_progress_caps_below_complete_until_final_result() {
        let mut last = 0.0;

        let progress = record_progress(40.0, 40.0, &mut last);

        assert_eq!(progress, 0.99);
        assert_eq!(last, 0.99);
    }

    #[ignore = "requires local cactus model files"]
    #[test]
    fn e2e_transcribe_with_real_model_inference() {
        let model_path_str = std::env::var("CACTUS_STT_MODEL").unwrap_or_else(|_| {
            dirs::data_dir()
                .expect("could not find data dir")
                .join("com.hyprnote.dev/models/cactus/whisper-small-int8-apple")
                .to_string_lossy()
                .into_owned()
        });
        let model_path = Path::new(&model_path_str);
        assert!(
            model_path.exists(),
            "model path does not exist: {}",
            model_path.display()
        );

        let wav_bytes = std::fs::read(hypr_data::english_1::AUDIO_PATH)
            .unwrap_or_else(|e| panic!("failed to read fixture wav: {e}"));

        let params = ListenParams {
            languages: vec![ISO639::En.into()],
            ..Default::default()
        };

        let response = transcribe_batch(&wav_bytes, "audio/wav", &params, model_path, None)
            .unwrap_or_else(|e| panic!("real-model batch transcription failed: {e}"));

        let Some(channel) = response.results.channels.first() else {
            panic!("expected at least one channel in response");
        };
        let Some(alternative) = channel.alternatives.first() else {
            panic!("expected at least one alternative in response");
        };

        println!("\n--- BATCH TRANSCRIPT ---");
        println!("{}", alternative.transcript.trim());
        println!("--- END (confidence={:.2}) ---\n", alternative.confidence);

        let transcript = alternative.transcript.trim().to_lowercase();
        assert!(!transcript.is_empty(), "expected non-empty transcript");
        assert!(
            transcript.contains("maybe")
                || transcript.contains("this")
                || transcript.contains("talking"),
            "transcript looks like a hallucination (got: {:?})",
            transcript
        );
        assert!(
            alternative.confidence.is_finite(),
            "expected finite confidence"
        );
        assert!(
            response
                .metadata
                .get("duration")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or_default()
                > 0.0,
            "expected positive duration metadata"
        );
    }
}
