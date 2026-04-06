use std::io::Write;
use std::path::Path;

use rodio::Source;
use tokio::sync::mpsc;

use hypr_audio_utils::content_type_to_extension;
use hypr_transcribe_core::{
    ProgressTracker, TARGET_SAMPLE_RATE, channel_duration_sec, chunk_channel_audio,
    initial_resolved_until, next_resolved_until, split_resampled_channels,
};
use owhisper_interface::ListenParams;
use owhisper_interface::batch;
use owhisper_interface::batch_sse::BatchSseMessage;

use super::response::{build_batch_words, build_segment_stream_response};

#[tracing::instrument(
    skip(audio_data, model, event_tx),
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
    model: &hypr_cactus::Model,
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
    let channel_count = u16::from(source.channels()).max(1) as usize;
    let resampled = hypr_audio_utils::resample_audio(source, TARGET_SAMPLE_RATE)?;
    let channel_samples = split_resampled_channels(&resampled, channel_count);
    let total_duration = channel_samples
        .iter()
        .map(|samples| channel_duration_sec(samples))
        .fold(0.0_f64, f64::max);

    let options = crate::service::build_transcribe_options(params, None);

    let metadata = crate::service::build_metadata(model_path);
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
    let mut response_channels = Vec::with_capacity(channel_samples.len().max(1));
    let mut progress = ProgressTracker::new(resolved_until, total_duration, event_tx);
    progress.emit(None);

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
                model,
                &options,
                &mut progress,
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

#[allow(clippy::too_many_arguments)]
fn transcribe_chunks(
    channel_idx: usize,
    chunks: &[hypr_audio_chunking::AudioChunk],
    channel_duration: f64,
    model: &hypr_cactus::Model,
    options: &hypr_cactus::TranscribeOptions,
    progress: &mut ProgressTracker,
    metadata: &owhisper_interface::stream::Metadata,
    channel_index: &[i32],
) -> Result<(Vec<batch::Word>, String, f64), crate::Error> {
    let mut all_words = Vec::new();
    let mut all_transcripts = Vec::new();
    let mut cumulative_confidence = 0.0;

    for (chunk_idx, chunk) in chunks.iter().enumerate() {
        let pcm_i16 = hypr_audio_utils::f32_to_i16_samples(&chunk.samples);
        let pcm_bytes: Vec<u8> = pcm_i16.iter().flat_map(|s| s.to_le_bytes()).collect();

        let chunk_start_sec = chunk.sample_start as f64 / TARGET_SAMPLE_RATE as f64;
        let chunk_duration_sec =
            (chunk.sample_end - chunk.sample_start) as f64 / TARGET_SAMPLE_RATE as f64;
        progress.update_channel(channel_idx, chunk_start_sec);

        let cactus_response = if progress.has_tx() {
            let completed_text: String = all_transcripts.join(" ");

            model.transcribe_pcm_with_callback(&pcm_bytes, options, |token| {
                let mut partial = completed_text.clone();

                if !token.is_empty() {
                    if !partial.is_empty() {
                        partial.push(' ');
                    }
                    partial.push_str(token);
                }

                let resolved = resolved_audio_for_chunk_progress(
                    chunk_start_sec,
                    chunk_duration_sec,
                    ChunkProgress::Start,
                );
                progress.emit_for_channel(channel_idx, resolved, Some(partial));

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
                channel_idx as i32,
            );
            for w in &mut words {
                w.start += chunk_start_sec;
                w.end += chunk_start_sec;
            }
            all_words.extend(words);

            if progress.has_tx() {
                let seg = crate::service::Segment {
                    text: &chunk_text,
                    start: chunk_start_sec,
                    duration: chunk_duration_sec,
                    confidence: cactus_response.confidence as f64,
                };
                let segment_resp = build_segment_stream_response(&seg, metadata, channel_index);
                if let Some(tx) = progress.event_tx() {
                    let _ = tx.send(BatchSseMessage::Segment {
                        response: segment_resp,
                    });
                }
            }

            all_transcripts.push(chunk_text);
        }

        progress.update_channel(
            channel_idx,
            next_resolved_until(chunks, chunk_idx, channel_duration),
        );
        progress.emit(Some(all_transcripts.join(" ")));

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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use hypr_language::ISO639;
    use hypr_transcribe_core::{
        overall_resolved_audio, overall_resolved_with_channel, record_progress,
    };
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
        let chunks = vec![hypr_audio_chunking::AudioChunk {
            samples: vec![],
            sample_start: 12 * TARGET_SAMPLE_RATE as usize,
            sample_end: 15 * TARGET_SAMPLE_RATE as usize,
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
    fn overall_resolved_audio_averages_channels() {
        let resolved = overall_resolved_audio(&[40.0, 18.0, 25.0]);

        assert!((resolved - 83.0 / 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn overall_resolved_with_channel_substitutes_current_channel() {
        let resolved = overall_resolved_with_channel(&[40.0, 10.0], 1, 22.0);

        assert!((resolved - 31.0).abs() < f64::EPSILON);
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

        let model = hypr_cactus::Model::new(model_path)
            .unwrap_or_else(|e| panic!("failed to load model: {e}"));

        let response = transcribe_batch(&wav_bytes, "audio/wav", &params, &model, model_path, None)
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
