use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use owhisper_interface::ListenParams;
use owhisper_interface::batch_sse::{BatchSseMessage, EVENT_NAME as BATCH_EVENT};
use owhisper_interface::progress::InferenceProgress;
use owhisper_interface::stream::StreamResponse;

use crate::adapter::{StreamingBatchEvent, StreamingBatchStream};
use crate::error::Error;

use super::CactusAdapter;

impl CactusAdapter {
    pub async fn transcribe_file_streaming(
        api_base: &str,
        params: &ListenParams,
        file_path: impl AsRef<Path>,
    ) -> Result<StreamingBatchStream, Error> {
        let path = file_path.as_ref().to_path_buf();
        tracing::info!(
            hyprnote.file.path = %path.display(),
            url.full = %api_base,
            "starting_cactus_batch_stream"
        );

        let (audio_data, content_type, audio_duration_secs) =
            tokio::task::spawn_blocking(move || load_audio_file(path))
                .await
                .map_err(|e| Error::AudioProcessing(format!("task panicked: {:?}", e)))??;

        let url = build_cactus_batch_url(api_base, params);

        let client = reqwest::Client::new();
        let response = client
            .post(url)
            .header("Content-Type", &content_type)
            .header("Accept", "text/event-stream")
            .body(audio_data)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            tracing::error!(
                http.response.status_code = status.as_u16(),
                hyprnote.http.response.body = %body,
                "unexpected_response_status"
            );
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

                    match state.stream.next().await {
                        Some(Ok(chunk)) => {
                            state.buffer.extend_from_slice(&chunk);
                            state.parse_buffer();
                        }
                        Some(Err(e)) => {
                            return Some((
                                Err(Error::WebSocket(format!("stream error: {:?}", e))),
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

fn load_audio_file(path: PathBuf) -> Result<(Vec<u8>, String, f64), Error> {
    let data =
        std::fs::read(&path).map_err(|e| Error::AudioProcessing(format!("read failed: {e}")))?;

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

    Ok((data, content_type, duration))
}

fn audio_duration_secs(path: &Path) -> f64 {
    use hypr_audio_utils::Source;
    let Ok(source) = hypr_audio_utils::source_from_path(path) else {
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

fn build_cactus_batch_url(api_base: &str, params: &ListenParams) -> url::Url {
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
    audio_duration_secs: f64,
    last_percentage: f64,
}

impl<S> SseParserState<S> {
    fn new(stream: S, audio_duration_secs: f64) -> Self {
        Self {
            stream,
            buffer: Vec::new(),
            pending_events: std::collections::VecDeque::new(),
            audio_duration_secs,
            last_percentage: 0.0,
        }
    }

    fn parse_buffer(&mut self) {
        while let Ok(text) = std::str::from_utf8(&self.buffer) {
            let Some(end) = text.find("\n\n") else {
                break;
            };

            let block = text[..end].to_string();
            self.buffer.drain(..end + 2);

            if let Some(event) = self.parse_sse_block(&block) {
                self.pending_events.push_back(event);
            }
        }
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
            BATCH_EVENT | "cactus_batch" => {
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
        self.last_percentage = progress.percentage;

        let response = StreamResponse::TranscriptResponse {
            start: 0.0,
            duration: self.audio_duration_secs * progress.percentage,
            is_final: false,
            speech_final: false,
            from_finalize: false,
            channel: owhisper_interface::stream::Channel {
                alternatives: vec![owhisper_interface::stream::Alternatives {
                    transcript: progress.partial_text.clone().unwrap_or_default(),
                    languages: vec![],
                    words: vec![],
                    confidence: 0.0,
                }],
            },
            metadata: owhisper_interface::stream::Metadata::default(),
            channel_index: vec![0, 1],
        };

        Some(Ok(StreamingBatchEvent {
            response,
            percentage: progress.percentage,
            final_batch_response: None,
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

        Some(Ok(StreamingBatchEvent {
            response,
            percentage: self.last_percentage,
            final_batch_response: None,
        }))
    }

    fn handle_result(
        &mut self,
        batch_response: owhisper_interface::batch::Response,
    ) -> Option<Result<StreamingBatchEvent, Error>> {
        let transcript = batch_response
            .results
            .channels
            .first()
            .and_then(|c| c.alternatives.first())
            .map(|a| a.transcript.clone())
            .unwrap_or_default();

        let duration = batch_response
            .metadata
            .get("duration")
            .and_then(|v| v.as_f64())
            .unwrap_or(self.audio_duration_secs);

        let response = StreamResponse::TranscriptResponse {
            start: 0.0,
            duration,
            is_final: true,
            speech_final: true,
            from_finalize: true,
            channel: owhisper_interface::stream::Channel {
                alternatives: vec![owhisper_interface::stream::Alternatives {
                    transcript,
                    languages: vec![],
                    words: vec![],
                    confidence: 0.0,
                }],
            },
            metadata: owhisper_interface::stream::Metadata::default(),
            channel_index: vec![0, 1],
        };

        Some(Ok(StreamingBatchEvent {
            response,
            percentage: 1.0,
            final_batch_response: Some(batch_response),
        }))
    }
}
