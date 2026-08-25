use std::path::{Path, PathBuf};

use owhisper_interface::ListenParams;
use owhisper_interface::batch::{Alternatives, Channel, Response as BatchResponse, Results, Word};
use reqwest::multipart::{Form, Part};

use crate::adapter::{
    BatchFuture, BatchSttAdapter, ClientWithMiddleware, MIXED_CAPTURE_CHANNEL,
    append_path_if_missing,
};
use crate::error::Error;

use super::MistralAdapter;

use crate::providers::{Provider, is_meta_model};

const DEFAULT_API_BASE: &str = "https://api.mistral.ai/v1";
const TIMESTAMP_GRANULARITY: &str = "segment";

impl BatchSttAdapter for MistralAdapter {
    fn provider_name(&self) -> &'static str {
        "mistral"
    }

    fn is_supported_languages(
        &self,
        languages: &[anlg_language::Language],
        _model: Option<&str>,
    ) -> bool {
        MistralAdapter::is_supported_languages_batch(languages)
    }

    fn transcribe_file<'a, P: AsRef<Path> + Send + 'a>(
        &'a self,
        client: &'a ClientWithMiddleware,
        api_base: &'a str,
        api_key: &'a str,
        params: &'a ListenParams,
        file_path: P,
    ) -> BatchFuture<'a> {
        let path = file_path.as_ref().to_path_buf();
        Box::pin(do_transcribe_file(client, api_base, api_key, params, path))
    }
}

#[derive(Debug, serde::Deserialize)]
struct MistralSegment {
    text: String,
    start: f64,
    end: f64,
    #[serde(default)]
    speaker_id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct MistralWord {
    word: String,
    start: f64,
    end: f64,
}

#[derive(Debug, serde::Deserialize)]
struct MistralBatchResponse {
    #[allow(dead_code)]
    model: Option<String>,
    language: Option<String>,
    text: String,
    #[serde(default)]
    words: Vec<MistralWord>,
    #[serde(default)]
    segments: Vec<MistralSegment>,
}

async fn do_transcribe_file(
    client: &ClientWithMiddleware,
    api_base: &str,
    api_key: &str,
    params: &ListenParams,
    file_path: PathBuf,
) -> Result<BatchResponse, Error> {
    let fallback_name = match file_path.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("audio.{}", ext),
        None => "audio".to_string(),
    };

    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or(fallback_name);

    let file_part = Part::file(&file_path)
        .await
        .map_err(|e| Error::AudioProcessing(e.to_string()))?
        .file_name(file_name);

    let mime_type = mime_type_from_extension(&file_path);

    let file_part = file_part
        .mime_str(mime_type)
        .map_err(|e| Error::AudioProcessing(e.to_string()))?;

    let mut form = Form::new().part("file", file_part);
    for (name, value) in multipart_text_fields(params) {
        form = form.text(name, value);
    }

    let mut url: url::Url = if api_base.is_empty() {
        DEFAULT_API_BASE
            .parse()
            .expect("invalid_default_mistral_api_base")
    } else {
        api_base.parse().map_err(|e: url::ParseError| {
            Error::AudioProcessing(format!("invalid api_base: {e}"))
        })?
    };
    append_path_if_missing(&mut url, "audio/transcriptions");

    let response = client
        .post(url.to_string())
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await?;

    let status = response.status();
    if status.is_success() {
        let mistral_response: MistralBatchResponse = response.json().await?;
        Ok(convert_response(mistral_response))
    } else {
        Err(Error::UnexpectedStatus {
            status,
            body: crate::adapter::http::error_body(response).await,
        })
    }
}

use crate::adapter::http::mime_type_from_extension;

fn strip_punctuation(s: &str) -> String {
    s.trim_matches(|c: char| c.is_ascii_punctuation())
        .to_string()
}

fn resolve_batch_model(model: Option<&str>) -> &str {
    let default = Provider::Mistral.default_batch_model();
    match model {
        Some(m) if is_meta_model(m) => default,
        Some("voxtral-mini-transcribe-realtime-2602") => default,
        Some(m) => m,
        None => default,
    }
}

fn multipart_text_fields(params: &ListenParams) -> Vec<(&'static str, String)> {
    // Mistral's timestamp granularities are incompatible with the language hint.
    vec![
        (
            "model",
            resolve_batch_model(params.model.as_deref()).to_string(),
        ),
        ("response_format", "verbose_json".to_string()),
        ("diarize", "true".to_string()),
        ("timestamp_granularities", TIMESTAMP_GRANULARITY.to_string()),
    ]
}

fn convert_response(response: MistralBatchResponse) -> BatchResponse {
    let has_diarized_segments = response.segments.iter().any(|segment| {
        segment
            .speaker_id
            .as_deref()
            .is_some_and(|label| !label.is_empty())
    });

    let (words, speaker_labels, timing_source): (Vec<Word>, Vec<String>, &str) =
        if has_diarized_segments {
            let (words, speaker_labels) = convert_segments(&response.segments, true);
            (words, speaker_labels, "provider_segment_interpolated")
        } else if !response.words.is_empty() {
            (
                response
                    .words
                    .into_iter()
                    .map(|w| {
                        let normalized = strip_punctuation(&w.word);
                        Word {
                            word: if normalized.is_empty() {
                                w.word.clone()
                            } else {
                                normalized
                            },
                            start: w.start,
                            end: w.end,
                            confidence: 1.0,
                            channel: 0,
                            speaker: None,
                            punctuated_word: Some(w.word),
                        }
                    })
                    .collect(),
                Vec::new(),
                "provider_word",
            )
        } else if !response.segments.is_empty() {
            let (words, speaker_labels) = convert_segments(&response.segments, false);
            (words, speaker_labels, "provider_segment_interpolated")
        } else {
            (Vec::new(), Vec::new(), "synthetic_text")
        };

    let alternatives = Alternatives {
        transcript: response.text.trim().to_string(),
        confidence: 1.0,
        words,
    };

    let channel = Channel {
        alternatives: vec![alternatives],
    };

    let metadata = serde_json::json!({
        "language": response.language,
        "speaker_labels": speaker_labels,
        "timing_source": timing_source,
    });

    BatchResponse {
        metadata,
        results: Results {
            channels: vec![channel],
        },
    }
}

fn convert_segments(segments: &[MistralSegment], diarized: bool) -> (Vec<Word>, Vec<String>) {
    let mut words = Vec::new();
    let mut speaker_labels = Vec::new();

    for segment in segments {
        let speaker = segment.speaker_id.as_ref().and_then(|label| {
            if label.is_empty() {
                return None;
            }

            Some(
                speaker_labels
                    .iter()
                    .position(|known| known == label)
                    .unwrap_or_else(|| {
                        speaker_labels.push(label.clone());
                        speaker_labels.len() - 1
                    }),
            )
        });
        let tokens = segment.text.split_whitespace().collect::<Vec<_>>();
        if tokens.is_empty() {
            continue;
        }

        let duration = (segment.end - segment.start).max(0.0);
        let word_duration = duration / tokens.len() as f64;
        for (index, token) in tokens.iter().enumerate() {
            let start = segment.start + word_duration * index as f64;
            let end = if index + 1 == tokens.len() {
                segment.end
            } else {
                segment.start + word_duration * (index + 1) as f64
            };
            let normalized = strip_punctuation(token);

            words.push(Word {
                word: if normalized.is_empty() {
                    (*token).to_string()
                } else {
                    normalized
                },
                start,
                end,
                confidence: 1.0,
                channel: if diarized { MIXED_CAPTURE_CHANNEL } else { 0 },
                speaker,
                punctuated_word: Some((*token).to_string()),
            });
        }
    }

    (words, speaker_labels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::BatchSttAdapter;
    use crate::http_client::create_client;

    #[test]
    fn batch_realtime_model_alias_uses_batch_model() {
        assert_eq!(
            resolve_batch_model(Some("voxtral-mini-transcribe-realtime-2602")),
            "voxtral-mini-2602"
        );
    }

    #[test]
    fn batch_fields_request_diarized_segments_without_language() {
        let fields = multipart_text_fields(&ListenParams {
            languages: vec![anlg_language::ISO639::En.into()],
            ..Default::default()
        });

        assert!(
            fields
                .iter()
                .any(|(name, value)| *name == "diarize" && value == "true")
        );
        assert!(
            fields
                .iter()
                .any(|(name, value)| { *name == "timestamp_granularities" && value == "segment" })
        );
        assert!(fields.iter().all(|(name, _)| *name != "language"));
    }

    #[test]
    fn convert_response_marks_segment_interpolated_words() {
        let response = convert_response(MistralBatchResponse {
            model: Some("voxtral-mini-latest".to_string()),
            language: Some("en".to_string()),
            text: "hello world".to_string(),
            words: Vec::new(),
            segments: vec![MistralSegment {
                text: "hello world".to_string(),
                start: 1.0,
                end: 3.0,
                speaker_id: None,
            }],
        });

        let alternative = &response.results.channels[0].alternatives[0];

        assert_eq!(alternative.words.len(), 2);
        assert_eq!(
            response.metadata["timing_source"],
            "provider_segment_interpolated"
        );
    }

    #[test]
    fn convert_response_preserves_diarized_speakers() {
        let response = convert_response(MistralBatchResponse {
            model: Some("voxtral-mini-2602".to_string()),
            language: Some("en".to_string()),
            text: "hello there welcome back".to_string(),
            words: Vec::new(),
            segments: vec![
                MistralSegment {
                    text: "hello there".to_string(),
                    start: 1.0,
                    end: 2.0,
                    speaker_id: Some("speaker_1".to_string()),
                },
                MistralSegment {
                    text: "welcome".to_string(),
                    start: 2.0,
                    end: 3.0,
                    speaker_id: Some("speaker_2".to_string()),
                },
                MistralSegment {
                    text: "back".to_string(),
                    start: 3.0,
                    end: 4.0,
                    speaker_id: Some("speaker_1".to_string()),
                },
            ],
        });

        let words = &response.results.channels[0].alternatives[0].words;

        assert_eq!(
            words.iter().map(|word| word.speaker).collect::<Vec<_>>(),
            vec![Some(0), Some(0), Some(1), Some(0)]
        );
        assert!(
            words
                .iter()
                .all(|word| word.channel == MIXED_CAPTURE_CHANNEL)
        );
        assert_eq!(words.last().map(|word| word.end), Some(4.0));
        assert_eq!(
            response.metadata["speaker_labels"],
            serde_json::json!(["speaker_1", "speaker_2"])
        );
    }

    #[tokio::test]
    #[ignore]
    async fn test_mistral_transcribe() {
        let api_key = std::env::var("MISTRAL_API_KEY").expect("MISTRAL_API_KEY not set");

        let adapter = MistralAdapter::default();
        let client = create_client();
        let api_base = "https://api.mistral.ai/v1";

        let params = ListenParams::default();

        let audio_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/data/src/english_1/audio.wav");

        let result = adapter
            .transcribe_file(&client, api_base, &api_key, &params, &audio_path)
            .await;

        let response = result.expect("transcription should succeed");

        assert!(!response.results.channels.is_empty());
        let channel = &response.results.channels[0];
        assert!(!channel.alternatives.is_empty());
        let alt = &channel.alternatives[0];
        assert!(!alt.transcript.is_empty());
        println!("Transcript: {}", alt.transcript);
        println!("Word count: {}", alt.words.len());
    }
}
