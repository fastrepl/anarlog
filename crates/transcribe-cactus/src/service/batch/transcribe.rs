use std::io::Write;
use std::num::NonZeroU8;
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
    let channels = source.channels();
    let resampled = hypr_audio_utils::resample_audio(source, TARGET_SAMPLE_RATE)?;
    let mono = hypr_audio_utils::mix_down_to_mono(
        &resampled,
        NonZeroU8::new(channels as u8).unwrap_or(NonZeroU8::new(1).unwrap()),
    );

    let total_duration = mono.len() as f64 / TARGET_SAMPLE_RATE as f64;

    let chunks = match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.block_on(chunk_mono_audio(&mono))?,
        Err(_) => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(chunk_mono_audio(&mono))?,
    };

    let model = match hypr_cactus::Model::new(model_path) {
        Ok(m) => m,
        Err(e) => {
            tracing::error!(error.message = %e, "failed_to_load_model");
            return Err(e.into());
        }
    };

    let custom_vocabulary = if params.keywords.is_empty() {
        None
    } else {
        Some(params.keywords.clone())
    };

    let options = hypr_cactus::TranscribeOptions {
        language: hypr_cactus::constrain_to(&params.languages),
        custom_vocabulary,
        ..Default::default()
    };

    let metadata = crate::service::build_metadata(model_path);
    let channel_index = [0, 1];

    let (all_words, transcript, avg_confidence) = if chunks.is_empty() {
        (vec![], String::new(), 0.0)
    } else {
        transcribe_chunks(
            &chunks,
            &model,
            &options,
            total_duration,
            event_tx,
            &metadata,
            &channel_index,
        )?
    };

    let mut metadata_json = serde_json::to_value(&metadata).unwrap_or_default();
    if let Some(obj) = metadata_json.as_object_mut() {
        obj.insert("duration".to_string(), serde_json::json!(total_duration));
        obj.insert("channels".to_string(), serde_json::json!(1));
    }

    Ok(batch::Response {
        metadata: metadata_json,
        results: batch::Results {
            channels: vec![batch::Channel {
                alternatives: vec![batch::Alternatives {
                    transcript,
                    confidence: avg_confidence,
                    words: all_words,
                }],
            }],
        },
    })
}

fn transcribe_chunks(
    chunks: &[hypr_vad_chunking::AudioChunk],
    model: &hypr_cactus::Model,
    options: &hypr_cactus::TranscribeOptions,
    total_duration: f64,
    event_tx: Option<mpsc::UnboundedSender<BatchSseMessage>>,
    metadata: &owhisper_interface::stream::Metadata,
    channel_index: &[i32],
) -> Result<(Vec<batch::Word>, String, f64), crate::Error> {
    let mut all_words = Vec::new();
    let mut all_transcripts = Vec::new();
    let mut cumulative_confidence = 0.0;

    for (i, chunk) in chunks.iter().enumerate() {
        let pcm_i16 = hypr_audio_utils::f32_to_i16_samples(&chunk.samples);
        let pcm_bytes: Vec<u8> = pcm_i16.iter().flat_map(|s| s.to_le_bytes()).collect();

        let chunk_start_sec = chunk.start_timestamp_ms as f64 / 1000.0;
        let chunk_duration_sec =
            (chunk.end_timestamp_ms - chunk.start_timestamp_ms) as f64 / 1000.0;

        let cactus_response = if let Some(ref tx) = event_tx {
            let tx = tx.clone();
            let completed_text: String = all_transcripts.join(" ");
            let chunks_before_sec: f64 = if i > 0 {
                chunks[i - 1].end_timestamp_ms as f64 / 1000.0
            } else {
                0.0
            };

            model.transcribe_pcm_with_callback(&pcm_bytes, options, |token| {
                let mut partial = completed_text.clone();

                if !token.starts_with("<|") || !token.ends_with("|>") {
                    if !partial.is_empty() {
                        partial.push(' ');
                    }
                    partial.push_str(token);
                }

                let chunk_progress = if chunk_duration_sec > 0.0 {
                    if let Some(ts) = parse_timestamp_token(token) {
                        (ts / chunk_duration_sec).clamp(0.0, 1.0)
                    } else {
                        0.0
                    }
                } else {
                    0.0
                };

                let elapsed_sec = chunks_before_sec + chunk_progress * chunk_duration_sec;
                let percentage = if total_duration > 0.0 {
                    (elapsed_sec / total_duration).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                let _ = tx.send(BatchSseMessage::Progress {
                    progress: InferenceProgress {
                        percentage,
                        partial_text: Some(partial),
                        phase: InferencePhase::Decoding,
                    },
                });

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
        cumulative_confidence += cactus_response.confidence as f64;
    }

    let transcript = all_transcripts.join(" ");
    let avg_confidence = cumulative_confidence / chunks.len() as f64;

    Ok((all_words, transcript, avg_confidence))
}

fn parse_timestamp_token(token: &str) -> Option<f64> {
    token
        .strip_prefix("<|")
        .and_then(|s| s.strip_suffix("|>"))
        .and_then(|s| s.parse::<f64>().ok())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use hypr_language::ISO639;
    use owhisper_interface::ListenParams;

    use super::*;

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
