use std::path::Path;

use owhisper_interface::ListenParams;
use owhisper_interface::batch::{
    Alternatives as BatchAlternatives, Channel as BatchChannel, Response as BatchResponse,
    Results as BatchResults, Word as BatchWord,
};
use serde::{Deserialize, Serialize};

use super::SmallestAIAdapter;
use crate::adapter::parsing::parse_speaker_id;
use crate::adapter::{BatchFuture, BatchSttAdapter, ClientWithMiddleware, MIXED_CAPTURE_CHANNEL};
use crate::error::Error;

impl BatchSttAdapter for SmallestAIAdapter {
    fn provider_name(&self) -> &'static str {
        "smallestai"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        model: Option<&str>,
    ) -> bool {
        SmallestAIAdapter::is_supported_languages_batch(languages, model)
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
        Box::pin(async move {
            SmallestAIAdapter::do_transcribe_file(client, api_base, api_key, params, &path).await
        })
    }
}

impl SmallestAIAdapter {
    async fn do_transcribe_file(
        client: &ClientWithMiddleware,
        api_base: &str,
        api_key: &str,
        params: &ListenParams,
        file_path: &Path,
    ) -> Result<BatchResponse, Error> {
        let file_bytes = tokio::fs::read(file_path).await.map_err(|error| {
            Error::AudioProcessing(format!(
                "failed to read file {}: {}",
                file_path.display(),
                error
            ))
        })?;

        let (mut url, existing_params) = SmallestAIAdapter::batch_api_url(api_base);
        {
            let mut query_pairs = url.query_pairs_mut();

            for (key, value) in &existing_params {
                query_pairs.append_pair(key, value);
            }

            query_pairs.append_pair("word_timestamps", "true");
            query_pairs.append_pair("diarize", "true");
            query_pairs.append_pair(
                "language",
                &SmallestAIAdapter::language_query_value(&params.languages),
            );
        }

        let response = client
            .post(url)
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/octet-stream")
            .body(file_bytes)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(Error::UnexpectedStatus { status, body });
        }

        let response: SmallestBatchResponse = response.json().await?;
        Ok(Self::to_batch_response(response))
    }

    fn to_batch_response(response: SmallestBatchResponse) -> BatchResponse {
        let transcript = response
            .transcript
            .clone()
            .or(response.text.clone())
            .or(response.full_transcript.clone())
            .unwrap_or_default();

        let words: Vec<BatchWord> = response
            .words
            .iter()
            .map(SmallestBatchWord::to_batch_word)
            .collect();

        let alternatives = BatchAlternatives {
            transcript,
            confidence: 1.0,
            words,
        };

        let channel = BatchChannel {
            alternatives: vec![alternatives],
        };

        BatchResponse {
            metadata: response.metadata(),
            results: BatchResults {
                channels: vec![channel],
            },
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct SmallestBatchResponse {
    #[serde(default, rename = "type")]
    message_type: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default, alias = "transcription")]
    transcript: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    full_transcript: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    languages: Option<Vec<String>>,
    #[serde(default)]
    words: Vec<SmallestBatchWord>,
    #[serde(default)]
    utterances: Vec<SmallestBatchUtterance>,
    #[serde(default)]
    redacted_entities: Option<Vec<String>>,
}

impl SmallestBatchResponse {
    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({
            "type": self.message_type,
            "status": self.status,
            "session_id": self.session_id,
            "full_transcript": self.full_transcript,
            "language": self.language,
            "languages": self.languages,
            "utterances": self.utterances,
            "redacted_entities": self.redacted_entities,
        })
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct SmallestBatchUtterance {
    #[serde(default)]
    text: String,
    #[serde(default)]
    start: f64,
    #[serde(default)]
    end: f64,
    #[serde(default)]
    speaker: Option<SmallestSpeaker>,
}

#[derive(Debug, Default, Deserialize)]
struct SmallestBatchWord {
    #[serde(default)]
    word: String,
    #[serde(default)]
    start: f64,
    #[serde(default)]
    end: f64,
    #[serde(default)]
    confidence: Option<f64>,
    #[serde(default)]
    speaker: Option<SmallestSpeaker>,
}

impl SmallestBatchWord {
    fn to_batch_word(&self) -> BatchWord {
        let speaker = self.speaker.as_ref().and_then(SmallestSpeaker::as_usize);
        BatchWord {
            word: self.word.clone(),
            start: self.start,
            end: self.end,
            confidence: self.confidence.unwrap_or(1.0),
            channel: if speaker.is_some() {
                MIXED_CAPTURE_CHANNEL
            } else {
                0
            },
            speaker,
            punctuated_word: Some(self.word.clone()),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum SmallestSpeaker {
    Int(i32),
    String(String),
}

impl SmallestSpeaker {
    fn as_usize(&self) -> Option<usize> {
        match self {
            Self::Int(value) => (*value >= 0).then_some(*value as usize),
            Self::String(value) => parse_speaker_id(value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_batch_response_uses_transcript_and_words() {
        let response: SmallestBatchResponse = serde_json::from_value(serde_json::json!({
            "type": "transcription",
            "status": "success",
            "transcript": "Hello there",
            "language": "en",
            "languages": ["en"],
            "words": [
                {"word": "Hello", "start": 0.0, "end": 0.4, "confidence": 0.99, "speaker": 0},
                {"word": "there", "start": 0.5, "end": 0.9, "confidence": 0.98, "speaker": "speaker_1"}
            ]
        }))
        .unwrap();

        let batch = SmallestAIAdapter::to_batch_response(response);
        let alternative = &batch.results.channels[0].alternatives[0];

        assert_eq!(alternative.transcript, "Hello there");
        assert_eq!(alternative.words.len(), 2);
        assert_eq!(alternative.words[0].speaker, Some(0));
        assert_eq!(alternative.words[0].channel, MIXED_CAPTURE_CHANNEL);
        assert_eq!(alternative.words[1].speaker, Some(1));
        assert_eq!(alternative.words[1].channel, MIXED_CAPTURE_CHANNEL);
        assert_eq!(batch.metadata["language"], "en");
    }

    #[test]
    fn test_to_batch_response_falls_back_to_text() {
        let response: SmallestBatchResponse = serde_json::from_value(serde_json::json!({
            "status": "success",
            "text": "Fallback transcript",
            "utterances": [
                {"text": "Fallback transcript", "start": 0.0, "end": 1.2, "speaker": 0}
            ]
        }))
        .unwrap();

        let batch = SmallestAIAdapter::to_batch_response(response);
        let alternative = &batch.results.channels[0].alternatives[0];

        assert_eq!(alternative.transcript, "Fallback transcript");
        assert!(alternative.words.is_empty());
        assert_eq!(
            batch.metadata["utterances"][0]["text"],
            "Fallback transcript"
        );
    }
}
