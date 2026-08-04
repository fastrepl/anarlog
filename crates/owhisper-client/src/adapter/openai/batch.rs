use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use openai_transcription::batch::{
    CreateTranscriptionOptions, CreateTranscriptionResponse, DiarizedTranscriptionResponse,
    ParsedTranscriptionStreamEvent, TimestampGranularity, TranscriptionStreamEventParser,
    TranscriptionUsage,
};
use owhisper_interface::ListenParams;
use owhisper_interface::batch::{Alternatives, Channel, Response as BatchResponse, Results, Word};
use owhisper_interface::batch_stream::BatchStreamEvent;
use reqwest::multipart::{Form, Part};

use crate::adapter::http::streaming_file_part;
use crate::adapter::{
    BatchFuture, BatchSttAdapter, ClientWithMiddleware, MIXED_CAPTURE_CHANNEL, StreamingBatchEvent,
    StreamingBatchStream, append_path_if_missing,
};
use crate::error::Error;

use super::OpenAIAdapter;

const DEFAULT_API_BASE: &str = "https://api.openai.com/v1";
const OPENAI_PROGRESS_CAP: f64 = 0.99;
const MAX_SSE_FRAME_BYTES: usize = 8 * 1_024 * 1_024;
const MAX_SSE_DELIMITER_BYTES: usize = 4;
const SSE_READ_AHEAD_BYTES: usize = 64 * 1_024;

impl BatchSttAdapter for OpenAIAdapter {
    fn provider_name(&self) -> &'static str {
        "openai"
    }

    fn is_supported_languages(
        &self,
        languages: &[anlg_language::Language],
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
                body: crate::adapter::http::error_body(response).await,
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
                    state.fill_pending_event();
                    if let Some(event) = state.pending_events.pop_front() {
                        return Some((event, state));
                    }
                    if state.terminated {
                        return None;
                    }
                    if state.stream_finished {
                        state.finish();
                        if let Some(event) = state.pending_events.pop_front() {
                            return Some((event, state));
                        }
                        return None;
                    }

                    match state.stream.next().await {
                        Some(Ok(chunk)) => {
                            state.push_chunk(chunk);
                        }
                        Some(Err(error)) => {
                            return Some((
                                Err(Error::WebSocket(format!("stream error: {:?}", error))),
                                state,
                            ));
                        }
                        None => {
                            state.stream_finished = true;
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
            body: crate::adapter::http::error_body(response).await,
        })
    }
}

async fn build_file_part(file_path: &Path) -> Result<Part, Error> {
    streaming_file_part(file_path).await
}

fn build_transcription_options(
    params: &ListenParams,
    use_response_format: bool,
    enable_streaming: bool,
) -> CreateTranscriptionOptions {
    let model = OpenAIAdapter::resolve_batch_model(params.model.as_deref());
    let mut options =
        CreateTranscriptionOptions::for_model(model, use_response_format, enable_streaming);

    if let CreateTranscriptionOptions::Whisper(options) = &mut options {
        if options.response_format.is_some() {
            options
                .timestamp_granularities
                .push(TimestampGranularity::Word);
        }
    }

    if model == openai_transcription::batch::AudioModel::GptTranscribe {
        for language in &params.languages {
            options.push_language(language.iso639().code().to_string());
        }
        for keyword in &params.keywords {
            options.push_keyword(keyword);
        }
    } else if let Some(language) = params.languages.first() {
        options.push_language(language.iso639().code().to_string());
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
    pending_chunk: Option<bytes::Bytes>,
    pending_chunk_offset: usize,
    parser: TranscriptionStreamEventParser,
    progress: OpenAISyntheticProgress,
    stream_finished: bool,
    terminated: bool,
}

impl<S> OpenAISseParserState<S> {
    fn new(stream: S) -> Self {
        Self {
            stream,
            buffer: Vec::new(),
            pending_events: std::collections::VecDeque::new(),
            pending_chunk: None,
            pending_chunk_offset: 0,
            parser: TranscriptionStreamEventParser::new(),
            progress: OpenAISyntheticProgress::default(),
            stream_finished: false,
            terminated: false,
        }
    }

    fn push_chunk(&mut self, chunk: bytes::Bytes) {
        if self.terminated {
            return;
        }
        debug_assert!(self.pending_chunk.is_none());
        self.pending_chunk = Some(chunk);
        self.pending_chunk_offset = 0;
        self.fill_pending_event();
    }

    fn fill_pending_event(&mut self) {
        while self.pending_events.is_empty() && !self.terminated {
            if let Some(event) = self.parse_next_buffer_event() {
                self.pending_events.push_back(event);
                return;
            }

            let Some(chunk) = self.pending_chunk.as_ref() else {
                return;
            };
            if self.pending_chunk_offset >= chunk.len() {
                self.pending_chunk = None;
                self.pending_chunk_offset = 0;
                continue;
            }

            let capacity =
                (MAX_SSE_FRAME_BYTES + MAX_SSE_DELIMITER_BYTES).saturating_sub(self.buffer.len());
            if capacity == 0 {
                self.fail("OpenAI batch SSE frame exceeded 8 MiB");
                return;
            }

            let end = self.pending_chunk_offset + capacity.min(SSE_READ_AHEAD_BYTES);
            let end = end.min(chunk.len());
            self.buffer
                .extend_from_slice(&chunk[self.pending_chunk_offset..end]);
            self.pending_chunk_offset = end;
            if end == chunk.len() {
                self.pending_chunk = None;
                self.pending_chunk_offset = 0;
            }
        }
    }

    fn parse_next_buffer_event(&mut self) -> Option<Result<StreamingBatchEvent, Error>> {
        while !self.terminated {
            let text = match std::str::from_utf8(&self.buffer) {
                Ok(text) => text,
                Err(error) if error.error_len().is_none() => return None,
                Err(_) => {
                    self.fail("OpenAI batch SSE frame contained invalid UTF-8");
                    return None;
                }
            };
            let (end, delimiter_len) = find_sse_block_end(text)?;
            if end > MAX_SSE_FRAME_BYTES {
                self.fail("OpenAI batch SSE frame exceeded 8 MiB");
                return None;
            }

            let block = text[..end].to_string();
            self.buffer.drain(..end + delimiter_len);

            if let Some(event) = self.parse_sse_block(&block) {
                return Some(event);
            }
        }
        None
    }

    fn finish(&mut self) {
        if self.terminated {
            return;
        }
        self.fill_pending_event();
        if !self.pending_events.is_empty() {
            return;
        }
        if !self.buffer.is_empty() {
            self.fail("OpenAI batch SSE stream ended with an incomplete frame");
        } else {
            self.terminated = true;
        }
    }

    fn fail(&mut self, message: &'static str) {
        if self.terminated {
            return;
        }
        self.buffer = Vec::new();
        self.pending_chunk = None;
        self.pending_chunk_offset = 0;
        self.terminated = true;
        self.pending_events
            .push_back(Err(Error::WebSocket(message.to_string())));
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
                    response: convert_text_response(text.trim().to_string(), usage),
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

fn text_response_metadata(
    usage: Option<TranscriptionUsage>,
    duration: Option<f64>,
) -> serde_json::Value {
    let mut metadata = transcription_usage_metadata(usage);

    if let Some(duration) = duration {
        if let Some(object) = metadata.as_object_mut() {
            object.insert("duration".to_string(), serde_json::json!(duration));
        }
    }

    metadata
}

fn strip_punctuation(s: &str) -> String {
    s.trim_matches(|c: char| c.is_ascii_punctuation())
        .to_string()
}

fn usage_duration_seconds(usage: Option<&TranscriptionUsage>) -> Option<f64> {
    let seconds = match usage {
        Some(TranscriptionUsage::Duration(duration)) => duration.seconds,
        _ => return None,
    };

    seconds.is_finite().then_some(seconds).filter(|s| *s > 0.0)
}

fn convert_text_response(transcript: String, usage: Option<TranscriptionUsage>) -> BatchResponse {
    let usage_duration = usage_duration_seconds(usage.as_ref());
    let mut metadata = text_response_metadata(usage, usage_duration);
    insert_timing_source(&mut metadata, "synthetic_text");

    build_batch_response(transcript, Vec::new(), metadata)
}

fn convert_response(response: CreateTranscriptionResponse) -> BatchResponse {
    match response {
        CreateTranscriptionResponse::Standard(response) => {
            convert_text_response(response.text.trim().to_string(), response.usage)
        }
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
                    "timing_source": "provider_word",
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
                    "timing_source": "provider_segment_interpolated",
                }),
            )
        }
    }
}

fn insert_timing_source(metadata: &mut serde_json::Value, source: &'static str) {
    if let Some(object) = metadata.as_object_mut() {
        object.insert("timing_source".to_string(), serde_json::json!(source));
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
    fn build_transcription_options_defaults_to_gpt_transcribe_json() {
        let options = build_transcription_options(&ListenParams::default(), true, false);

        let fields = options
            .multipart_text_fields()
            .expect("serialize multipart");
        assert!(
            fields
                .iter()
                .any(|field| { field.name == "response_format" && field.value == "json" })
        );
        assert!(!fields.iter().any(|field| field.name == "stream"));
    }

    #[test]
    fn gpt_transcribe_preserves_all_language_hints() {
        let options = build_transcription_options(
            &ListenParams {
                languages: vec![
                    anlg_language::ISO639::En.into(),
                    anlg_language::ISO639::Ko.into(),
                ],
                ..Default::default()
            },
            true,
            false,
        );

        let fields = options
            .multipart_text_fields()
            .expect("serialize multipart");
        let languages = fields
            .iter()
            .filter(|field| field.name == "languages[]")
            .map(|field| field.value.as_str())
            .collect::<Vec<_>>();

        assert_eq!(languages, vec!["en", "ko"]);
        assert!(!fields.iter().any(|field| field.name == "language"));
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
        assert!(
            !fields
                .iter()
                .any(|field| field.name == "timestamp_granularities[]")
        );
    }

    #[test]
    fn build_transcription_options_requests_word_timestamps_for_whisper_batch() {
        let options = build_transcription_options(
            &ListenParams {
                model: Some("whisper-1".to_string()),
                ..Default::default()
            },
            true,
            false,
        );

        let fields = options
            .multipart_text_fields()
            .expect("serialize multipart");
        assert!(
            fields.iter().any(|field| {
                field.name == "timestamp_granularities[]" && field.value == "word"
            })
        );
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
        let words = &response.results.channels[0].alternatives[0].words;
        assert!(words.is_empty());
        assert_eq!(response.metadata["usage"]["type"], "tokens");
        assert_eq!(response.metadata["timing_source"], "synthetic_text");
    }

    #[test]
    fn parse_buffer_handles_crlf_delimited_sse_blocks() {
        let mut state = OpenAISseParserState::new(());
        state.buffer =
            b"data: {\"type\":\"transcript.text.delta\",\"delta\":\"hello\"}\r\n\r\n".to_vec();

        state.fill_pending_event();

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
    fn incomplete_sse_frame_at_eof_is_a_terminal_error() {
        let mut state = OpenAISseParserState::new(());
        state.push_chunk(bytes::Bytes::from_static(
            b"data: {\"type\":\"transcript.text.delta\",\"delta\":\"hello\"}",
        ));

        state.finish();

        assert!(state.terminated);
        assert!(state.buffer.is_empty());
        let error = state
            .pending_events
            .pop_front()
            .expect("expected terminal error")
            .expect_err("incomplete frame must fail");
        assert!(error.to_string().contains("incomplete frame"));
    }

    #[test]
    fn oversized_sse_frame_is_rejected_without_retaining_the_frame() {
        let mut state = OpenAISseParserState::new(());
        let oversized = vec![b'x'; MAX_SSE_FRAME_BYTES + MAX_SSE_DELIMITER_BYTES + 1];

        state.push_chunk(oversized.into());

        assert!(state.terminated);
        assert!(state.buffer.is_empty());
        let error = state
            .pending_events
            .pop_front()
            .expect("expected terminal error")
            .expect_err("oversized frame must fail");
        assert!(error.to_string().contains("exceeded 8 MiB"));
        state.push_chunk(bytes::Bytes::from_static(b"data: ignored\n\n"));
        assert!(state.pending_events.is_empty());
    }

    #[test]
    fn one_chunk_queues_only_one_sse_event_at_a_time() {
        let mut state = OpenAISseParserState::new(());
        let frame = "data: {\"type\":\"transcript.text.delta\",\"delta\":\"x\"}\n\n";
        let event_count = 2_048;
        state.push_chunk(frame.repeat(event_count).into());
        assert!(state.pending_chunk.is_some());
        assert!(state.buffer.len() <= SSE_READ_AHEAD_BYTES);

        for _ in 0..event_count {
            assert_eq!(state.pending_events.len(), 1);
            state.pending_events.pop_front().unwrap().unwrap();
            state.fill_pending_event();
        }

        assert!(state.pending_events.is_empty());
        assert!(state.buffer.is_empty());
        assert!(state.pending_chunk.is_none());
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
    fn convert_standard_response_preserves_text_without_words() {
        let response: CreateTranscriptionResponse = serde_json::from_str(
            r#"{
                "text": " hello, world! ",
                "usage": {
                    "type": "duration",
                    "seconds": 2.0
                }
            }"#,
        )
        .expect("parse standard response");

        let batch = convert_response(response);
        let alternative = &batch.results.channels[0].alternatives[0];
        let words = &alternative.words;

        assert_eq!(alternative.transcript, "hello, world!");
        assert!(words.is_empty());
        assert_eq!(batch.metadata["usage"]["type"], "duration");
        assert_eq!(batch.metadata["duration"], 2.0);
        assert_eq!(batch.metadata["timing_source"], "synthetic_text");
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
        assert_eq!(
            batch.metadata["timing_source"],
            "provider_segment_interpolated"
        );
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
