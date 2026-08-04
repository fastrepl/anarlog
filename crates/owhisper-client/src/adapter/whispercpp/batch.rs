use std::path::Path;

use futures_util::StreamExt;
use owhisper_interface::ListenParams;
use owhisper_interface::batch_sse::{BatchSseMessage, EVENT_NAME as BATCH_EVENT};
use owhisper_interface::batch_stream::BatchStreamEvent;
use owhisper_interface::progress::InferenceProgress;
use owhisper_interface::stream::StreamResponse;

use crate::adapter::{StreamingBatchEvent, StreamingBatchStream};
use crate::error::Error;

use super::WhisperCppAdapter;

const MAX_SSE_FRAME_BYTES: usize = 8 * 1_024 * 1_024;
const MAX_SSE_DELIMITER_BYTES: usize = 4;
const SSE_READ_AHEAD_BYTES: usize = 64 * 1_024;

impl WhisperCppAdapter {
    pub async fn transcribe_file_streaming(
        api_base: &str,
        params: &ListenParams,
        file_path: impl AsRef<Path>,
    ) -> Result<StreamingBatchStream, Error> {
        let path = file_path.as_ref().to_path_buf();
        tracing::info!(
            anarlog.file.path = %path.display(),
            url.full = %api_base,
            "starting_whispercpp_batch_stream"
        );

        let metadata_path = path.clone();
        let (content_type, audio_duration_secs) =
            tokio::task::spawn_blocking(move || load_audio_metadata(&metadata_path))
                .await
                .map_err(|e| Error::AudioProcessing(format!("task panicked: {:?}", e)))??;
        let audio_file = tokio::fs::File::open(&path)
            .await
            .map_err(|e| Error::AudioProcessing(format!("failed to open file: {e}")))?;
        let content_length = audio_file
            .metadata()
            .await
            .map_err(|e| Error::AudioProcessing(format!("failed to inspect file: {e}")))?
            .len();

        let url = build_batch_url(api_base, params);

        let response = reqwest::Client::new()
            .post(url.as_str())
            .header("Content-Type", &content_type)
            .header("Accept", "text/event-stream")
            .header("Content-Length", content_length)
            .body(audio_file)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = crate::adapter::http::error_body(response).await;
            return Err(Error::UnexpectedStatus { status, body });
        }

        let byte_stream = response.bytes_stream();

        let event_stream = futures_util::stream::unfold(
            SseParserState::new(byte_stream, audio_duration_secs),
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
                        Some(Err(e)) => {
                            return Some((
                                Err(Error::WebSocket(format!("stream error: {:?}", e))),
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

fn load_audio_metadata(path: &Path) -> Result<(String, f64), Error> {
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("wav");
    let content_type = match extension {
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "flac" => "audio/flac",
        "m4a" => "audio/mp4",
        "webm" => "audio/webm",
        _ => "application/octet-stream",
    }
    .to_string();

    let duration = audio_duration_secs(&path);

    Ok((content_type, duration))
}

fn audio_duration_secs(path: &Path) -> f64 {
    use anlg_audio_utils::Source;
    let Ok(source) = anlg_audio_utils::source_from_path(path) else {
        return 0.0;
    };
    if let Some(d) = source.total_duration() {
        return d.as_secs_f64();
    }
    let sample_rate = u32::from(source.sample_rate()) as f64;
    let channels = u16::from(source.channels()).max(1) as f64;
    let count = source.count() as f64;
    count / channels / sample_rate
}

fn build_batch_url(api_base: &str, params: &ListenParams) -> url::Url {
    let mut url: url::Url = api_base.parse().expect("invalid api_base URL");

    if !url.path().ends_with("/listen") {
        let path = url.path().trim_end_matches('/').to_string();
        url.set_path(&format!("{}/listen", path));
    }

    for lang in &params.languages {
        url.query_pairs_mut()
            .append_pair("language", lang.iso639().code());
    }
    if !params.keywords.is_empty() {
        for kw in &params.keywords {
            url.query_pairs_mut().append_pair("keywords", kw);
        }
    }
    if let Some(ref model) = params.model {
        url.query_pairs_mut().append_pair("model", model);
    }

    url
}

struct SseParserState<S> {
    stream: S,
    buffer: Vec<u8>,
    pending_events: std::collections::VecDeque<Result<StreamingBatchEvent, Error>>,
    pending_chunk: Option<bytes::Bytes>,
    pending_chunk_offset: usize,
    audio_duration_secs: f64,
    last_percentage: f64,
    stream_finished: bool,
    terminated: bool,
}

impl<S> SseParserState<S> {
    fn new(stream: S, audio_duration_secs: f64) -> Self {
        Self {
            stream,
            buffer: Vec::new(),
            pending_events: std::collections::VecDeque::new(),
            pending_chunk: None,
            pending_chunk_offset: 0,
            audio_duration_secs,
            last_percentage: 0.0,
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
                self.fail("Whisper.cpp batch SSE frame exceeded 8 MiB");
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
                    self.fail("Whisper.cpp batch SSE frame contained invalid UTF-8");
                    return None;
                }
            };
            let (end, delimiter_len) = find_sse_block_end(text)?;
            if end > MAX_SSE_FRAME_BYTES {
                self.fail("Whisper.cpp batch SSE frame exceeded 8 MiB");
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
            self.fail("Whisper.cpp batch SSE stream ended with an incomplete frame");
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
        let mut event_type = String::new();
        let mut data = String::new();

        for line in block.lines() {
            if let Some(rest) = line.strip_prefix("event:") {
                event_type = rest.trim().to_string();
            } else if let Some(rest) = line.strip_prefix("data:") {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(rest.trim());
            } else if line.starts_with(':') {
                // comment, skip
            }
        }

        if data.is_empty() {
            return None;
        }

        match event_type.as_str() {
            BATCH_EVENT => {
                let msg: BatchSseMessage = match serde_json::from_str(&data) {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::warn!(
                            raw_data = %data,
                            "failed to parse batch SSE event: {e}"
                        );
                        return None;
                    }
                };

                match msg {
                    BatchSseMessage::Progress { progress } => self.handle_progress(progress),
                    BatchSseMessage::Segment { response } => self.handle_segment(response),
                    BatchSseMessage::Result { response } => self.handle_result(response),
                    BatchSseMessage::Error { detail, .. } => {
                        tracing::error!(detail = %detail, "server returned error event");
                        Some(Err(Error::WebSocket(format!("server error: {}", detail))))
                    }
                }
            }
            "progress" => {
                let progress: InferenceProgress = match serde_json::from_str(&data) {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::warn!(raw_data = %data, "failed to parse progress event: {e}");
                        return None;
                    }
                };
                self.handle_progress(progress)
            }
            "segment" => {
                let response: StreamResponse = match serde_json::from_str(&data) {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!(raw_data = %data, "failed to parse segment event: {e}");
                        return None;
                    }
                };
                self.handle_segment(response)
            }
            "result" => {
                let batch_response: owhisper_interface::batch::Response =
                    match serde_json::from_str(&data) {
                        Ok(r) => r,
                        Err(e) => {
                            tracing::error!(raw_data = %data, "failed to parse result event: {e}");
                            return Some(Err(Error::WebSocket(format!(
                                "failed to parse result: {e}"
                            ))));
                        }
                    };

                self.handle_result(batch_response)
            }
            "error" => {
                let error_data: serde_json::Value = serde_json::from_str(&data).unwrap_or_default();
                let detail = error_data
                    .get("detail")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error");
                tracing::error!(detail = %detail, raw_data = %data, "server returned error event");
                Some(Err(Error::WebSocket(format!("server error: {}", detail))))
            }
            _ => None,
        }
    }

    fn handle_progress(
        &mut self,
        progress: InferenceProgress,
    ) -> Option<Result<StreamingBatchEvent, Error>> {
        self.last_percentage = self.last_percentage.max(progress.percentage);

        Some(Ok(BatchStreamEvent::Progress {
            percentage: progress.percentage,
            partial_text: progress.partial_text,
        }))
    }

    fn handle_segment(
        &mut self,
        response: StreamResponse,
    ) -> Option<Result<StreamingBatchEvent, Error>> {
        let segment_end = match &response {
            StreamResponse::TranscriptResponse {
                start, duration, ..
            } => start + duration,
            _ => 0.0,
        };

        let percentage = if self.audio_duration_secs > 0.0 {
            (segment_end / self.audio_duration_secs).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.last_percentage = self.last_percentage.max(percentage);

        let event = match response {
            StreamResponse::TranscriptResponse { .. } => BatchStreamEvent::Segment {
                response,
                percentage: self.last_percentage,
            },
            StreamResponse::TerminalResponse {
                request_id,
                created,
                duration,
                channels,
            } => BatchStreamEvent::Terminal {
                request_id,
                created,
                duration,
                channels,
            },
            StreamResponse::ErrorResponse {
                error_code,
                error_message,
                provider,
            } => BatchStreamEvent::Error {
                error_code,
                error_message,
                provider,
            },
            other => BatchStreamEvent::Segment {
                response: other,
                percentage: self.last_percentage,
            },
        };

        Some(Ok(event))
    }

    fn handle_result(
        &mut self,
        batch_response: owhisper_interface::batch::Response,
    ) -> Option<Result<StreamingBatchEvent, Error>> {
        Some(Ok(BatchStreamEvent::Result {
            response: batch_response,
        }))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incomplete_sse_frame_at_eof_is_a_terminal_error() {
        let mut state = SseParserState::new((), 0.0);
        state.push_chunk(bytes::Bytes::from_static(
            b"event: progress\ndata: {\"percentage\":0.5}",
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
        let mut state = SseParserState::new((), 0.0);
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
        state.push_chunk(bytes::Bytes::from_static(b"event: progress\ndata: {}\n\n"));
        assert!(state.pending_events.is_empty());
    }

    #[test]
    fn crlf_delimited_frame_is_parsed() {
        let mut state = SseParserState::new((), 0.0);
        state.push_chunk(bytes::Bytes::from_static(
            b"event: progress\r\ndata: {\"percentage\":0.5,\"partial_text\":null,\"phase\":\"transcribing\"}\r\n\r\n",
        ));

        let event = state
            .pending_events
            .pop_front()
            .expect("expected progress event")
            .expect("valid progress event");
        assert!(matches!(
            event,
            BatchStreamEvent::Progress { percentage, .. } if percentage == 0.5
        ));
    }

    #[test]
    fn one_chunk_queues_only_one_sse_event_at_a_time() {
        let mut state = SseParserState::new((), 0.0);
        let frame = "event: progress\ndata: {\"percentage\":0.5,\"partial_text\":null,\"phase\":\"transcribing\"}\n\n";
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
}
