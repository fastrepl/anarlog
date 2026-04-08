use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use openai_transcription::batch::{
    CreateTranscriptionOptions, CreateTranscriptionResponse, DiarizedTranscriptionResponse,
    ParsedTranscriptionStreamEvent, TranscriptionStreamEventParser, TranscriptionUsage,
};
use owhisper_interface::ListenParams;
use owhisper_interface::batch::{Alternatives, Channel, Response as BatchResponse, Results, Word};
use owhisper_interface::batch_stream::BatchStreamEvent;
use reqwest::multipart::{Form, Part};

use crate::adapter::{
    BatchFuture, BatchSttAdapter, ClientWithMiddleware, MIXED_CAPTURE_CHANNEL, StreamingBatchEvent,
    StreamingBatchStream, append_path_if_missing,
};
use crate::error::Error;

use super::OpenAIAdapter;

const DEFAULT_API_BASE: &str = "https://api.openai.com/v1";
const OPENAI_PROGRESS_CAP: f64 = 0.99;

impl BatchSttAdapter for OpenAIAdapter {
    fn provider_name(&self) -> &'static str {
        "openai"
    }

    fn is_supported_languages(
        &self,
        languages: &[hypr_language::Language],
        _model: Option<&str>,
    ) -> bool {
        OpenAIAdapter::is_supported_languages_batch(languages)
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

impl OpenAIAdapter {
    pub async fn transcribe_file_streaming(
        api_base: &str,
        api_key: &str,
        params: &ListenParams,
        file_path: impl AsRef<Path>,
    ) -> Result<StreamingBatchStream, Error> {
        let file_path = file_path.as_ref().to_path_buf();
        let file_part = build_file_part(&file_path).await?;

        let options = build_transcription_options(params, false, true);
        let form = build_multipart_form(file_part, options)?;
        let url = transcription_url(api_base)?;

        let response = reqwest::Client::new()
            .post(url.to_string())
            .header("Authorization", format!("Bearer {}", api_key))
            .multipart(form)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            return Err(Error::UnexpectedStatus {
                status,
                body: response.text().await.unwrap_or_default(),
            });
        }

        let byte_stream = response.bytes_stream();
        let event_stream = futures_util::stream::unfold(
            OpenAISseParserState::new(byte_stream),
            |mut state| async move {
                loop {
                    if let Some(event) = state.pending_events.pop_front() {
                        return Some((event, state));
                    }

                    match state.stream.next().await {
                        Some(Ok(chunk)) => {
                            state.buffer.extend_from_slice(&chunk);
                            state.parse_buffer();
                        }
                        Some(Err(error)) => {
                            return Some((
                                Err(Error::WebSocket(format!("stream error: {:?}", error))),
                                state,
                            ));
                        }
                        None => {
                            if !state.buffer.is_empty() {
                                state.parse_buffer();
                                if let Some(event) = state.pending_events.pop_front() {
                                    return Some((event, state));
                                }
                            }
                            return None;
                        }
                    }
                }
            },
        );

        Ok(Box::pin(event_stream))
    }
}

async fn do_transcribe_file(
    client: &ClientWithMiddleware,
    api_base: &str,
    api_key: &str,
    params: &ListenParams,
    file_path: PathBuf,
) -> Result<BatchResponse, Error> {
    let file_part = build_file_part(&file_path).await?;
    let options = build_transcription_options(params, true, false);
    let form = build_multipart_form(file_part, options)?;
    let url = transcription_url(api_base)?;

    let response = client
        .post(url.to_string())
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await?;

    let status = response.status();
    if status.is_success() {
        let openai_response: CreateTranscriptionResponse = response.json().await?;
        Ok(convert_response(openai_response))
    } else {
        Err(Error::UnexpectedStatus {
            status,
            body: response.text().await.unwrap_or_default(),
        })
    }
}

use crate::adapter::http::mime_type_from_extension;

async fn build_file_part(file_path: &Path) -> Result<Part, Error> {
    let fallback_name = match file_path.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("audio.{}", ext),
        None => "audio".to_string(),
    };

    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or(fallback_name);

    let file_bytes = tokio::fs::read(file_path)
        .await
        .map_err(|e| Error::AudioProcessing(e.to_string()))?;

    let mime_type = mime_type_from_extension(file_path);

    Part::bytes(file_bytes)
        .file_name(file_name)
        .mime_str(mime_type)
        .map_err(|e| Error::AudioProcessing(e.to_string()))
}

fn build_transcription_options(
    params: &ListenParams,
    use_response_format: bool,
    enable_streaming: bool,
) -> CreateTranscriptionOptions {
    let model = OpenAIAdapter::resolve_batch_model(params.model.as_deref());
    let mut options =
        CreateTranscriptionOptions::for_model(model, use_response_format, enable_streaming);

    if let Some(lang) = params.languages.first() {
        options.push_language(lang.iso639().code().to_string());
    }

    options
}

fn build_multipart_form(
    file_part: Part,
    options: CreateTranscriptionOptions,
) -> Result<Form, Error> {
    let mut form = Form::new().part("file", file_part);

    for field in options
        .multipart_text_fields()
        .map_err(|e| Error::AudioProcessing(e.to_string()))?
    {
        form = form.text(field.name, field.value);
    }

    Ok(form)
}

fn transcription_url(api_base: &str) -> Result<url::Url, Error> {
    let mut url: url::Url = if api_base.is_empty() {
        DEFAULT_API_BASE
            .parse()
            .expect("invalid_default_openai_api_base")
    } else {
        api_base.parse().map_err(|e: url::ParseError| {
            Error::AudioProcessing(format!("invalid api_base: {e}"))
        })?
    };
    append_path_if_missing(&mut url, "audio/transcriptions");
    Ok(url)
}

struct OpenAISseParserState<S> {
    stream: S,
    buffer: Vec<u8>,
    pending_events: std::collections::VecDeque<Result<StreamingBatchEvent, Error>>,
    parser: TranscriptionStreamEventParser,
    progress: OpenAISyntheticProgress,
}

impl<S> OpenAISseParserState<S> {
    fn new(stream: S) -> Self {
        Self {
            stream,
            buffer: Vec::new(),
            pending_events: std::collections::VecDeque::new(),
            parser: TranscriptionStreamEventParser::new(),
            progress: OpenAISyntheticProgress::default(),
        }
    }

    fn parse_buffer(&mut self) {
        while let Ok(text) = std::str::from_utf8(&self.buffer) {
            let Some((end, delimiter_len)) = find_sse_block_end(text) else {
                break;
            };

            let block = text[..end].to_string();
            self.buffer.drain(..end + delimiter_len);

            if let Some(event) = self.parse_sse_block(&block) {
                self.pending_events.push_back(event);
            }
        }
    }

    fn parse_sse_block(&mut self, block: &str) -> Option<Result<StreamingBatchEvent, Error>> {
        let event = match self.parser.parse_sse_block(block) {
            Ok(Some(event)) => event,
            Ok(None) => return None,
            Err(error) => {
                return Some(Err(Error::WebSocket(format!(
                    "failed to parse OpenAI batch stream event: {error}"
                ))));
            }
        };

        match event {
            ParsedTranscriptionStreamEvent::TextDelta { partial_text, .. } => {
                Some(Ok(BatchStreamEvent::Progress {
                    percentage: self.progress.observe_delta(&partial_text),
                    partial_text: Some(partial_text),
                }))
            }
            ParsedTranscriptionStreamEvent::TextDone { text, usage, .. } => {
                Some(Ok(BatchStreamEvent::Result {
                    response: build_batch_response(
                        text.trim().to_string(),
                        Vec::new(),
                        transcription_usage_metadata(usage),
                    ),
                }))
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
struct OpenAISyntheticProgress {
    last_percentage: f64,
    delta_count: usize,
}

impl OpenAISyntheticProgress {
    const CHAR_SCALE: f64 = 160.0;
    const CHAR_PROGRESS_CAP: f64 = 0.85;
    const TRICKLE_SCALE: f64 = 32.0;
    const TRICKLE_PROGRESS_CAP: f64 = 0.9;

    fn observe_delta(&mut self, partial_text: &str) -> f64 {
        self.delta_count += 1;

        let char_count = partial_text.chars().count() as f64;
        let char_progress =
            (char_count / (char_count + Self::CHAR_SCALE)).min(Self::CHAR_PROGRESS_CAP);
        let trickle_progress = ((self.delta_count as f64)
            / (self.delta_count as f64 + Self::TRICKLE_SCALE))
            .min(Self::TRICKLE_PROGRESS_CAP);

        self.last_percentage = char_progress
            .max(trickle_progress)
            .max(self.last_percentage)
            .min(OPENAI_PROGRESS_CAP);
        self.last_percentage
    }
}

fn find_sse_block_end(text: &str) -> Option<(usize, usize)> {
    let lf = text.find("\n\n").map(|index| (index, 2));
    let crlf = text.find("\r\n\r\n").map(|index| (index, 4));

    match (lf, crlf) {
        (Some(left), Some(right)) => Some(if left.0 <= right.0 { left } else { right }),
        (Some(found), None) | (None, Some(found)) => Some(found),
        (None, None) => None,
    }
}

fn transcription_usage_metadata(usage: Option<TranscriptionUsage>) -> serde_json::Value {
    match usage {
        Some(usage) => serde_json::json!({ "usage": usage }),
        None => serde_json::json!({}),
    }
}

fn strip_punctuation(s: &str) -> String {
    s.trim_matches(|c: char| c.is_ascii_punctuation())
        .to_string()
}

fn convert_response(response: CreateTranscriptionResponse) -> BatchResponse {
    match response {
        CreateTranscriptionResponse::Standard(response) => build_batch_response(
            response.text.trim().to_string(),
            Vec::new(),
            serde_json::json!({}),
        ),
        CreateTranscriptionResponse::Verbose(response) => {
            let words = response
                .words
                .iter()
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
                        punctuated_word: Some(w.word.clone()),
                    }
                })
                .collect();

            build_batch_response(
                response.text.trim().to_string(),
                words,
                serde_json::json!({
                    "language": response.language,
                    "duration": response.duration,
                }),
            )
        }
        CreateTranscriptionResponse::Diarized(response) => {
            let (words, speaker_labels) = convert_diarized_words(&response);
            let speaker_segments = response
                .segments
                .iter()
                .map(|segment| {
                    serde_json::json!({
                        "id": segment.id,
                        "speaker": segment.speaker,
                        "start": segment.start,
                        "end": segment.end,
                        "text": segment.text,
                        "type": segment.segment_type,
                    })
                })
                .collect::<Vec<_>>();

            build_batch_response(
                response.text.trim().to_string(),
                words,
                serde_json::json!({
                    "duration": response.duration,
                    "speaker_labels": speaker_labels,
                    "speaker_segments": speaker_segments,
                }),
            )
        }
    }
}

fn convert_diarized_words(response: &DiarizedTranscriptionResponse) -> (Vec<Word>, Vec<String>) {
    let mut speaker_labels = Vec::new();
    let mut words = Vec::new();

    for segment in &response.segments {
        let tokens = segment.text.split_whitespace().collect::<Vec<_>>();
        if tokens.is_empty() {
            continue;
        }

        let speaker_index = speaker_labels
            .iter()
            .position(|label| label == &segment.speaker)
            .unwrap_or_else(|| {
                speaker_labels.push(segment.speaker.clone());
                speaker_labels.len() - 1
            });

        let segment_duration = (segment.end - segment.start).max(0.0);
        let word_duration = segment_duration / tokens.len() as f64;

        for (index, token) in tokens.iter().enumerate() {
            let normalized = strip_punctuation(token);
            let start = segment.start + word_duration * index as f64;
            let end = if index + 1 == tokens.len() {
                segment.end
            } else {
                segment.start + word_duration * (index + 1) as f64
            };

            words.push(Word {
                word: if normalized.is_empty() {
                    (*token).to_string()
                } else {
                    normalized
                },
                start,
                end,
                confidence: 1.0,
                channel: MIXED_CAPTURE_CHANNEL,
                speaker: Some(speaker_index),
                punctuated_word: Some((*token).to_string()),
            });
        }
    }

    (words, speaker_labels)
}

fn build_batch_response(
    transcript: String,
    words: Vec<Word>,
    metadata: serde_json::Value,
) -> BatchResponse {
    let alternatives = Alternatives {
        transcript,
        confidence: 1.0,
        words,
    };

    let channel = Channel {
        alternatives: vec![alternatives],
    };

    BatchResponse {
        metadata,
        results: Results {
            channels: vec![channel],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::BatchSttAdapter;
    use crate::http_client::create_client;

    #[test]
    fn build_transcription_options_defaults_to_diarized_json_for_diarize_model() {
        let options = build_transcription_options(&ListenParams::default(), true, false);

        let fields = options
            .multipart_text_fields()
            .expect("serialize multipart");
        assert!(
            fields
                .iter()
                .any(|field| { field.name == "response_format" && field.value == "diarized_json" })
        );
        assert!(!fields.iter().any(|field| field.name == "stream"));
    }

    #[test]
    fn build_transcription_options_omits_stream_for_whisper() {
        let options = build_transcription_options(
            &ListenParams {
                model: Some("whisper-1".to_string()),
                ..Default::default()
            },
            false,
            true,
        );

        let fields = options
            .multipart_text_fields()
            .expect("serialize multipart");
        assert!(matches!(options, CreateTranscriptionOptions::Whisper(_)));
        assert!(!fields.iter().any(|field| field.name == "stream"));
    }

    #[test]
    fn parse_sse_delta_accumulates_partial_text() {
        let mut state = OpenAISseParserState::new(());
        let event = state
            .parse_sse_block(r#"data: {"type":"transcript.text.delta","delta":"hello"}"#)
            .expect("expected progress event")
            .expect("expected valid progress event");

        assert_eq!(state.parser.partial_text(), "hello");
        assert!(matches!(
            event,
            BatchStreamEvent::Progress {
                percentage,
                partial_text: Some(ref text),
                ..
            } if text == "hello" && percentage > 0.0
        ));
    }

    #[test]
    fn parse_sse_done_emits_final_result() {
        let mut state = OpenAISseParserState::new(());
        state
            .parser
            .parse_sse_block(r#"data: {"type":"transcript.text.delta","delta":"hello"}"#)
            .expect("seed parser");
        let event = state
            .parse_sse_block(
                r#"data: {"type":"transcript.text.done","text":"hello world","usage":{"type":"tokens","input_tokens":1,"output_tokens":2,"total_tokens":3}}"#,
            )
            .expect("expected result event")
            .expect("expected valid result event");

        let BatchStreamEvent::Result { response } = event else {
            panic!("expected result event");
        };

        assert_eq!(state.parser.partial_text(), "hello world");
        assert_eq!(
            response.results.channels[0].alternatives[0].transcript,
            "hello world"
        );
        assert_eq!(response.metadata["usage"]["type"], "tokens");
    }

    #[test]
    fn parse_buffer_handles_crlf_delimited_sse_blocks() {
        let mut state = OpenAISseParserState::new(());
        state.buffer =
            b"data: {\"type\":\"transcript.text.delta\",\"delta\":\"hello\"}\r\n\r\n".to_vec();

        state.parse_buffer();

        let event = state
            .pending_events
            .pop_front()
            .expect("expected parsed event")
            .expect("expected valid event");

        assert!(state.buffer.is_empty());
        assert!(matches!(
            event,
            BatchStreamEvent::Progress {
                percentage,
                partial_text: Some(ref text),
                ..
            } if text == "hello" && percentage > 0.0
        ));
    }

    #[test]
    fn estimate_openai_progress_is_monotonic_and_capped() {
        let mut progress = OpenAISyntheticProgress::default();
        let first = progress.observe_delta("hello");
        let second = progress.observe_delta("hello world from openai");
        let capped = progress.observe_delta(&"a".repeat(10_000));

        assert!(first > 0.0);
        assert!(second >= first);
        assert!(capped <= OPENAI_PROGRESS_CAP);
    }

    #[test]
    fn convert_diarized_response_preserves_speaker_segments() {
        let response: CreateTranscriptionResponse = serde_json::from_str(
            r#"{
                "duration": 4.0,
                "task": "transcribe",
                "text": "hello there general kenobi",
                "segments": [
                    {
                        "id": "seg_1",
                        "speaker": "agent",
                        "start": 0.0,
                        "end": 2.0,
                        "text": "hello there",
                        "type": "transcript.text.segment"
                    },
                    {
                        "id": "seg_2",
                        "speaker": "customer",
                        "start": 2.0,
                        "end": 4.0,
                        "text": "general kenobi",
                        "type": "transcript.text.segment"
                    }
                ]
            }"#,
        )
        .expect("parse diarized response");

        let batch = convert_response(response);
        let words = &batch.results.channels[0].alternatives[0].words;

        assert_eq!(words.len(), 4);
        assert_eq!(words[0].channel, MIXED_CAPTURE_CHANNEL);
        assert_eq!(words[0].speaker, Some(0));
        assert_eq!(words[2].speaker, Some(1));
        assert_eq!(batch.metadata["speaker_labels"][0], "agent");
        assert_eq!(
            batch.metadata["speaker_segments"].as_array().map(Vec::len),
            Some(2)
        );
    }

    #[tokio::test]
    #[ignore]
    async fn test_openai_transcribe() {
        let api_key = std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");

        let adapter = OpenAIAdapter::default();
        let client = create_client();
        let api_base = "https://api.openai.com/v1";

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
