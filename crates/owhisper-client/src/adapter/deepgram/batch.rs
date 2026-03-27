use std::path::{Path, PathBuf};

use owhisper_interface::ListenParams;
use owhisper_interface::batch::{
    Alternatives as BatchAlternatives, Channel as BatchChannel, Response as BatchResponse,
    Results as BatchResults, Word as BatchWord,
};
use serde::Deserialize;

use crate::adapter::deepgram_compat::build_batch_url;
use crate::adapter::{BatchFuture, BatchSttAdapter, ClientWithMiddleware};
use crate::error::Error;

use super::{
    DeepgramAdapter, keywords::DeepgramKeywordStrategy, language::DeepgramLanguageStrategy,
};

impl BatchSttAdapter for DeepgramAdapter {
    fn provider_name(&self) -> &'static str {
        "deepgram"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> bool {
        DeepgramAdapter::is_supported_languages_batch(languages, model)
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

use crate::adapter::http::mime_type_from_extension;

async fn do_transcribe_file(
    client: &ClientWithMiddleware,
    api_base: &str,
    api_key: &str,
    params: &ListenParams,
    file_path: PathBuf,
) -> Result<BatchResponse, Error> {
    let audio_data = tokio::fs::read(&file_path)
        .await
        .map_err(|e| Error::AudioProcessing(format!("failed to read file: {}", e)))?;

    let content_type = mime_type_from_extension(&file_path);

    let url = build_batch_url(
        api_base,
        params,
        &DeepgramLanguageStrategy,
        &DeepgramKeywordStrategy,
    );

    let response = client
        .post(url)
        .header("Authorization", format!("Token {}", api_key))
        .header("Accept", "application/json")
        .header("Content-Type", content_type)
        .body(audio_data)
        .send()
        .await?;

    let status = response.status();
    if status.is_success() {
        let legacy: DeepgramBatchResponse = response.json().await?;
        Ok(convert_response(legacy))
    } else {
        Err(Error::UnexpectedStatus {
            status,
            body: response.text().await.unwrap_or_default(),
        })
    }
}

#[derive(Debug, Deserialize)]
struct DeepgramBatchResponse {
    metadata: serde_json::Value,
    results: DeepgramBatchResults,
}

#[derive(Debug, Deserialize)]
struct DeepgramBatchResults {
    channels: Vec<DeepgramBatchChannel>,
}

#[derive(Debug, Deserialize)]
struct DeepgramBatchChannel {
    alternatives: Vec<DeepgramBatchAlternatives>,
}

#[derive(Debug, Deserialize)]
struct DeepgramBatchAlternatives {
    transcript: String,
    confidence: f64,
    #[serde(default)]
    words: Vec<DeepgramBatchWord>,
}

#[derive(Debug, Deserialize)]
struct DeepgramBatchWord {
    word: String,
    start: f64,
    end: f64,
    confidence: f64,
    #[serde(default)]
    speaker: Option<usize>,
    #[serde(default)]
    punctuated_word: Option<String>,
}

fn convert_response(response: DeepgramBatchResponse) -> BatchResponse {
    let channels = response
        .results
        .channels
        .into_iter()
        .enumerate()
        .map(|(channel_idx, channel)| BatchChannel {
            alternatives: channel
                .alternatives
                .into_iter()
                .map(|alt| BatchAlternatives {
                    transcript: alt.transcript,
                    confidence: alt.confidence,
                    words: alt
                        .words
                        .into_iter()
                        .map(|word| BatchWord {
                            word: word.word,
                            start: word.start,
                            end: word.end,
                            confidence: word.confidence,
                            channel: channel_idx as i32,
                            speaker: word.speaker,
                            punctuated_word: word.punctuated_word,
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect();

    BatchResponse {
        metadata: response.metadata,
        results: BatchResults { channels },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::deepgram_compat::{
        KeywordQueryStrategy, LanguageQueryStrategy, TranscriptionMode,
    };
    use crate::http_client::create_client;
    use url::UrlQuery;
    use url::form_urlencoded::Serializer;

    struct NoLanguageStrategy;

    impl LanguageQueryStrategy for NoLanguageStrategy {
        fn append_language_query<'a>(
            &self,
            _query_pairs: &mut Serializer<'a, UrlQuery>,
            _params: &ListenParams,
            _mode: TranscriptionMode,
        ) {
        }
    }

    struct NoKeywordStrategy;

    impl KeywordQueryStrategy for NoKeywordStrategy {
        fn append_keyword_query<'a>(
            &self,
            _query_pairs: &mut Serializer<'a, UrlQuery>,
            _params: &ListenParams,
        ) {
        }
    }

    #[test]
    fn preserves_channel_identity_for_multichannel_batch_words() {
        let response = DeepgramBatchResponse {
            metadata: serde_json::json!({ "channels": 2 }),
            results: DeepgramBatchResults {
                channels: vec![
                    DeepgramBatchChannel {
                        alternatives: vec![DeepgramBatchAlternatives {
                            transcript: "left".to_string(),
                            confidence: 0.9,
                            words: vec![DeepgramBatchWord {
                                word: "left".to_string(),
                                start: 0.0,
                                end: 1.0,
                                confidence: 0.9,
                                speaker: None,
                                punctuated_word: Some("left".to_string()),
                            }],
                        }],
                    },
                    DeepgramBatchChannel {
                        alternatives: vec![DeepgramBatchAlternatives {
                            transcript: "right".to_string(),
                            confidence: 0.8,
                            words: vec![DeepgramBatchWord {
                                word: "right".to_string(),
                                start: 0.0,
                                end: 1.0,
                                confidence: 0.8,
                                speaker: None,
                                punctuated_word: Some("right".to_string()),
                            }],
                        }],
                    },
                ],
            },
        };

        let converted = convert_response(response);

        assert_eq!(converted.results.channels.len(), 2);
        assert_eq!(
            converted.results.channels[0].alternatives[0].words[0].channel,
            0
        );
        assert_eq!(
            converted.results.channels[1].alternatives[0].words[0].channel,
            1
        );
    }

    #[test]
    fn batch_url_enables_multichannel_for_stereo_audio() {
        let params = ListenParams {
            channels: 2,
            ..Default::default()
        };

        let url = build_batch_url(
            "https://api.deepgram.com/v1",
            &params,
            &NoLanguageStrategy,
            &NoKeywordStrategy,
        );

        let query = url.query().unwrap_or_default();
        assert!(query.contains("multichannel=true"));
    }

    #[tokio::test]
    #[ignore]
    async fn test_deepgram_batch_transcription() {
        let api_key = std::env::var("DEEPGRAM_API_KEY").expect("DEEPGRAM_API_KEY not set");
        let client = create_client();
        let adapter = DeepgramAdapter::default();
        let params = ListenParams {
            model: Some("nova-2".to_string()),
            ..Default::default()
        };

        let audio_path = std::path::PathBuf::from(hypr_data::english_1::AUDIO_PATH);

        let result = adapter
            .transcribe_file(
                &client,
                "https://api.deepgram.com/v1",
                &api_key,
                &params,
                &audio_path,
            )
            .await
            .expect("transcription failed");

        assert!(!result.results.channels.is_empty());
        assert!(!result.results.channels[0].alternatives.is_empty());
        assert!(
            !result.results.channels[0].alternatives[0]
                .transcript
                .is_empty()
        );
    }
}
