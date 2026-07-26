use std::path::Path;

use owhisper_interface::ListenParams;
use owhisper_interface::batch::{
    Alternatives as BatchAlternatives, Channel as BatchChannel, Response as BatchResponse,
    Results as BatchResults, Word as BatchWord,
};

use super::{SonioxAdapter, words};
use crate::adapter::parsing::ms_to_secs_opt;
use crate::adapter::{BatchFuture, BatchSttAdapter, ClientWithMiddleware, MIXED_CAPTURE_CHANNEL};
use crate::error::Error;

impl SonioxAdapter {
    async fn do_transcribe_file(
        api_key: &str,
        params: &ListenParams,
        file_path: &Path,
    ) -> Result<BatchResponse, Error> {
        let client = reqwest::Client::new();

        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio.wav")
            .to_string();

        let file_bytes = tokio::fs::read(file_path).await.map_err(|e| {
            Error::AudioProcessing(format!(
                "failed to read file {}: {}",
                file_path.display(),
                e
            ))
        })?;

        tracing::info!(hyprnote.file.path = %file_path.display(), "uploading_file_to_soniox");
        let file_id = soniox::upload_file(&client, &file_name, file_bytes, api_key)
            .await
            .map_err(soniox_err)?;

        tracing::info!(hyprnote.file.id = %file_id, "soniox_file_uploaded");
        let result = Self::transcribe_and_fetch(&client, api_key, params, &file_id).await;

        if let Err(e) = soniox::delete_file(&client, &file_id, api_key).await {
            tracing::warn!(
                hyprnote.file.id = %file_id,
                error = %e,
                "failed_to_delete_soniox_file"
            );
        }

        result
    }

    async fn transcribe_and_fetch(
        client: &reqwest::Client,
        api_key: &str,
        params: &ListenParams,
        file_id: &str,
    ) -> Result<BatchResponse, Error> {
        let model = SonioxAdapter::resolve_model(params.model.as_deref()).batch_model();

        let mut body = serde_json::json!({
            "model": model,
            "file_id": file_id,
            "enable_speaker_diarization": true,
            "enable_language_identification": true,
        });

        let language_hints: Vec<String> = params
            .languages
            .iter()
            .map(|lang| lang.iso639().code().to_string())
            .collect();
        if !language_hints.is_empty() {
            body["language_hints"] = serde_json::json!(language_hints);
        }
        if !params.keywords.is_empty() {
            body["context"] = serde_json::json!({ "terms": params.keywords });
        }

        let transcription_id = soniox::create_transcription(client, &body, api_key)
            .await
            .map_err(soniox_err)?;
        tracing::info!(
            hyprnote.stt.job.id = %transcription_id,
            "soniox_transcription_created"
        );

        soniox::wait_for_completion(client, &transcription_id, api_key)
            .await
            .map_err(soniox_err)?;
        tracing::info!(
            hyprnote.stt.job.id = %transcription_id,
            "soniox_transcription_completed"
        );

        let transcript = soniox::fetch_transcript(client, &transcription_id, api_key)
            .await
            .map_err(soniox_err)?;
        tracing::info!("transcript fetched successfully");

        Ok(Self::to_batch_response(transcript))
    }

    fn to_batch_response(transcript: soniox::TranscriptResponse) -> BatchResponse {
        let tokens = transcript.tokens.iter().collect::<Vec<_>>();
        let words: Vec<BatchWord> = words::build_token_words(&tokens)
            .into_iter()
            .map(|word| BatchWord {
                word: word.text.clone(),
                start: ms_to_secs_opt(word.start_ms),
                end: ms_to_secs_opt(word.end_ms),
                confidence: word.confidence,
                channel: MIXED_CAPTURE_CHANNEL,
                speaker: word.speaker.and_then(|speaker| speaker.try_into().ok()),
                punctuated_word: Some(word.text),
            })
            .collect();

        let alternatives = BatchAlternatives {
            transcript: transcript.text,
            confidence: 1.0,
            words,
        };

        let channel = BatchChannel {
            alternatives: vec![alternatives],
        };

        BatchResponse {
            metadata: serde_json::json!({}),
            results: BatchResults {
                channels: vec![channel],
            },
        }
    }
}

fn soniox_err(e: soniox::Error) -> Error {
    Error::provider_failure(e.message, e.is_retryable)
}

impl BatchSttAdapter for SonioxAdapter {
    fn provider_name(&self) -> &'static str {
        "soniox"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        _model: Option<&str>,
    ) -> bool {
        SonioxAdapter::is_supported_languages_batch(languages)
    }

    fn transcribe_file<'a, P: AsRef<Path> + Send + 'a>(
        &'a self,
        _client: &'a ClientWithMiddleware,
        _api_base: &'a str,
        api_key: &'a str,
        params: &'a ListenParams,
        file_path: P,
    ) -> BatchFuture<'a> {
        let path = file_path.as_ref().to_path_buf();
        Box::pin(async move { Self::do_transcribe_file(api_key, params, &path).await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_client::create_client;

    #[test]
    fn speaker_labeled_tokens_use_mixed_capture_channel() {
        let transcript = soniox::TranscriptResponse {
            text: "hello there".to_string(),
            tokens: vec![
                soniox::Token {
                    text: "hello".to_string(),
                    start_ms: Some(0),
                    end_ms: Some(400),
                    confidence: Some(0.9),
                    is_final: Some(true),
                    speaker: Some(soniox::SpeakerId::Str("speaker_0".to_string())),
                    language: Some("en".to_string()),
                },
                soniox::Token {
                    text: " there".to_string(),
                    start_ms: Some(400),
                    end_ms: Some(900),
                    confidence: Some(0.8),
                    is_final: Some(true),
                    speaker: Some(soniox::SpeakerId::Num(1)),
                    language: Some("en".to_string()),
                },
            ],
        };

        let batch = SonioxAdapter::to_batch_response(transcript);
        let words = &batch.results.channels[0].alternatives[0].words;

        assert_eq!(words.len(), 2);
        assert_eq!(words[0].word, "hello");
        assert_eq!(words[0].channel, MIXED_CAPTURE_CHANNEL);
        assert_eq!(words[0].speaker, Some(0));
        assert_eq!(words[1].word, "there");
        assert_eq!(words[1].channel, MIXED_CAPTURE_CHANNEL);
        assert_eq!(words[1].speaker, Some(1));
    }

    #[test]
    fn subword_tokens_are_grouped_into_batch_words() {
        let transcript = soniox::TranscriptResponse {
            text: "What would it require?".to_string(),
            tokens: vec![
                soniox::Token {
                    text: "Wh".to_string(),
                    start_ms: Some(0),
                    end_ms: Some(40),
                    confidence: Some(0.9),
                    is_final: Some(true),
                    speaker: None,
                    language: Some("en".to_string()),
                },
                soniox::Token {
                    text: "at".to_string(),
                    start_ms: Some(40),
                    end_ms: Some(80),
                    confidence: Some(0.8),
                    is_final: Some(true),
                    speaker: None,
                    language: Some("en".to_string()),
                },
                soniox::Token {
                    text: " would".to_string(),
                    start_ms: Some(120),
                    end_ms: Some(240),
                    confidence: Some(0.95),
                    is_final: Some(true),
                    speaker: None,
                    language: Some("en".to_string()),
                },
                soniox::Token {
                    text: " it".to_string(),
                    start_ms: Some(260),
                    end_ms: Some(300),
                    confidence: Some(0.95),
                    is_final: Some(true),
                    speaker: None,
                    language: Some("en".to_string()),
                },
                soniox::Token {
                    text: " re".to_string(),
                    start_ms: Some(320),
                    end_ms: Some(360),
                    confidence: Some(0.9),
                    is_final: Some(true),
                    speaker: None,
                    language: Some("en".to_string()),
                },
                soniox::Token {
                    text: "qu".to_string(),
                    start_ms: Some(360),
                    end_ms: Some(400),
                    confidence: Some(0.8),
                    is_final: Some(true),
                    speaker: None,
                    language: Some("en".to_string()),
                },
                soniox::Token {
                    text: "ire".to_string(),
                    start_ms: Some(400),
                    end_ms: Some(480),
                    confidence: Some(0.7),
                    is_final: Some(true),
                    speaker: None,
                    language: Some("en".to_string()),
                },
                soniox::Token {
                    text: "?".to_string(),
                    start_ms: Some(480),
                    end_ms: Some(500),
                    confidence: Some(1.0),
                    is_final: Some(true),
                    speaker: None,
                    language: Some("en".to_string()),
                },
            ],
        };

        let batch = SonioxAdapter::to_batch_response(transcript);
        let alternative = &batch.results.channels[0].alternatives[0];
        let words = &alternative.words;

        assert_eq!(alternative.transcript, "What would it require?");
        assert_eq!(
            words
                .iter()
                .map(|word| word.word.as_str())
                .collect::<Vec<_>>(),
            vec!["What", "would", "it", "require?"]
        );
        assert_eq!(words[0].start, 0.0);
        assert_eq!(words[0].end, 0.08);
        assert_eq!(words[3].start, 0.32);
        assert_eq!(words[3].end, 0.5);
        assert_eq!(words[3].punctuated_word.as_deref(), Some("require?"));
    }

    #[tokio::test]
    #[ignore]
    async fn test_soniox_batch_transcription() {
        let api_key = std::env::var("SONIOX_API_KEY").expect("SONIOX_API_KEY not set");
        let client = create_client();
        let adapter = SonioxAdapter::default();
        let params = ListenParams::default();

        let audio_path = std::path::PathBuf::from(hypr_data::english_1::AUDIO_PATH);

        let result = adapter
            .transcribe_file(&client, "", &api_key, &params, &audio_path)
            .await
            .expect("transcription failed");

        assert!(!result.results.channels.is_empty());
        assert!(!result.results.channels[0].alternatives.is_empty());
        assert!(
            !result.results.channels[0].alternatives[0]
                .transcript
                .is_empty()
        );
        assert!(!result.results.channels[0].alternatives[0].words.is_empty());
    }
}
