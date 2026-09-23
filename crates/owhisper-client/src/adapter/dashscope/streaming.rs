use anlg_ws_client::client::Message;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{Alternatives, Channel, Metadata, StreamResponse, Word};
use serde::{Deserialize, Serialize};

use super::DashScopeAdapter;
use crate::adapter::RealtimeSttAdapter;
use crate::providers::Provider;

pub const DASHSCOPE_STREAMING_MODEL: &str = "qwen-audio-3.1-asr-flash-streaming";
const INFERENCE_WS_PATH: &str = "/api-ws/v1/inference";
const DEFAULT_SAMPLE_RATE: u32 = 16000;

/// DashScope task-based duplex protocol (`run-task` / `task-started` / `result-generated` /
/// `finish-task` / `task-finished`) used by the Qwen-Audio 3.1 streaming ASR models.
#[derive(Clone)]
pub struct DashScopeStreamingAdapter {
    task_id: String,
}

impl Default for DashScopeStreamingAdapter {
    fn default() -> Self {
        Self {
            task_id: uuid::Uuid::new_v4().simple().to_string(),
        }
    }
}

impl DashScopeStreamingAdapter {
    pub fn is_model(model: &str) -> bool {
        model.starts_with("qwen-audio-") && model.contains("-asr-") && model.ends_with("-streaming")
    }

    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    fn resolve_model(params: &ListenParams) -> &str {
        match params.model.as_deref() {
            Some(m) if Self::is_model(m) => m,
            _ => DASHSCOPE_STREAMING_MODEL,
        }
    }

    fn build_transcript_response(sentence: Sentence) -> Vec<StreamResponse> {
        if sentence.text.is_empty() {
            return vec![];
        }

        let start = sentence.begin_time.unwrap_or(0) as f64 / 1000.0;
        let end = sentence
            .end_time
            .map(|t| t as f64 / 1000.0)
            .unwrap_or(start);
        let words: Vec<Word> = sentence
            .words
            .into_iter()
            .filter(|w| !w.text.is_empty())
            .map(|w| Word {
                punctuated_word: Some(format!("{}{}", w.text, w.punctuation)),
                word: w.text,
                start: w.begin_time as f64 / 1000.0,
                end: w.end_time as f64 / 1000.0,
                confidence: 1.0,
                speaker: None,
                language: None,
            })
            .collect();

        vec![StreamResponse::TranscriptResponse {
            is_final: sentence.sentence_end,
            speech_final: sentence.sentence_end,
            from_finalize: false,
            start,
            duration: (end - start).max(0.0),
            channel: Channel {
                alternatives: vec![Alternatives {
                    transcript: sentence.text,
                    words,
                    confidence: 1.0,
                    languages: vec![],
                }],
            },
            metadata: Metadata::default(),
            channel_index: vec![0, 1],
        }]
    }

    /// Empty finalization marker so consumers waiting on `from_finalize` observe `task-finished`.
    fn finalization_marker() -> StreamResponse {
        StreamResponse::TranscriptResponse {
            is_final: true,
            speech_final: true,
            from_finalize: true,
            start: 0.0,
            duration: 0.0,
            channel: Channel {
                alternatives: vec![Alternatives {
                    transcript: String::new(),
                    words: vec![],
                    confidence: 1.0,
                    languages: vec![],
                }],
            },
            metadata: Metadata::default(),
            channel_index: vec![0, 1],
        }
    }
}

impl RealtimeSttAdapter for DashScopeStreamingAdapter {
    fn provider_name(&self) -> &'static str {
        "dashscope"
    }

    fn is_supported_languages(
        &self,
        languages: &[anlg_language::Language],
        _model: Option<&str>,
    ) -> bool {
        DashScopeAdapter::is_supported_languages_live(languages)
    }

    fn supports_native_multichannel(&self) -> bool {
        false
    }

    fn build_ws_url(&self, api_base: &str, params: &ListenParams, _channels: u8) -> url::Url {
        let (mut url, existing_params) = DashScopeAdapter::build_ws_url_from_base(api_base);

        if url
            .host_str()
            .is_some_and(|host| Provider::DashScope.is_host(host))
        {
            url.set_path(INFERENCE_WS_PATH);
        }

        {
            let mut query_pairs = url.query_pairs_mut();
            query_pairs.append_pair("model", Self::resolve_model(params));
            for (key, value) in &existing_params {
                query_pairs.append_pair(key, value);
            }
        }

        url
    }

    fn build_auth_header(&self, api_key: Option<&str>) -> Option<(&'static str, String)> {
        api_key.and_then(|k| Provider::DashScope.build_auth_header(k))
    }

    fn fork_session(&self) -> Self {
        Self::default()
    }

    fn initial_response_type(&self) -> Option<&'static str> {
        Some("task-started")
    }

    fn initial_response_field(&self) -> &'static str {
        "/header/event"
    }

    fn keep_alive_message(&self) -> Option<Message> {
        None
    }

    fn initial_message(
        &self,
        _api_key: Option<&str>,
        params: &ListenParams,
        _channels: u8,
    ) -> Option<Message> {
        let sample_rate = if params.sample_rate == 0 {
            DEFAULT_SAMPLE_RATE
        } else {
            params.sample_rate
        };

        let run_task = TaskRequest {
            header: TaskHeader {
                action: "run-task",
                task_id: &self.task_id,
                streaming: "duplex",
            },
            payload: RunTaskPayload {
                task_group: "audio",
                task: "asr",
                function: "recognition",
                model: Self::resolve_model(params),
                parameters: RunTaskParameters {
                    format: "pcm",
                    sample_rate,
                },
                input: EmptyInput {},
            },
        };

        let json = serde_json::to_string(&run_task).ok()?;
        tracing::debug!(
            anarlog.payload.size_bytes = json.len() as u64,
            "dashscope_run_task_payload"
        );
        Some(Message::Text(json.into()))
    }

    fn finalize_message(&self) -> Message {
        let finish = TaskRequest {
            header: TaskHeader {
                action: "finish-task",
                task_id: &self.task_id,
                streaming: "duplex",
            },
            payload: FinishTaskPayload {
                input: EmptyInput {},
            },
        };
        Message::Text(serde_json::to_string(&finish).unwrap().into())
    }

    fn parse_response(&self, raw: &str) -> Vec<StreamResponse> {
        let event: TaskEvent = match serde_json::from_str(raw) {
            Ok(e) => e,
            Err(_e) => {
                tracing::warn!(
                    error.type = "invalid_provider_payload",
                    anarlog.payload.size_bytes = raw.len() as u64,
                    "dashscope_streaming_json_parse_failed"
                );
                return vec![];
            }
        };

        match event.header.event.as_str() {
            "task-started" => {
                tracing::debug!("dashscope_task_started");
                vec![]
            }
            "result-generated" => match event
                .payload
                .and_then(|p| p.output)
                .and_then(|o| o.sentence)
            {
                Some(sentence) => Self::build_transcript_response(sentence),
                None => vec![],
            },
            "task-finished" => {
                tracing::debug!("dashscope_task_finished");
                vec![Self::finalization_marker()]
            }
            "task-failed" => {
                let code = event.header.error_code.unwrap_or_default();
                let message = event.header.error_message.unwrap_or_default();
                crate::log_provider_failure("dashscope", &code, None, &message);
                vec![StreamResponse::ErrorResponse {
                    error_code: None,
                    error_message: format!("{code}: {message}"),
                    provider: "dashscope".to_string(),
                }]
            }
            _ => {
                tracing::debug!(
                    anarlog.payload.size_bytes = raw.len() as u64,
                    "dashscope_streaming_unknown_event"
                );
                vec![]
            }
        }
    }
}

#[derive(Debug, Serialize)]
struct TaskRequest<'a, P: Serialize> {
    header: TaskHeader<'a>,
    payload: P,
}

#[derive(Debug, Serialize)]
struct TaskHeader<'a> {
    action: &'static str,
    task_id: &'a str,
    streaming: &'static str,
}

#[derive(Debug, Serialize)]
struct RunTaskPayload<'a> {
    task_group: &'static str,
    task: &'static str,
    function: &'static str,
    model: &'a str,
    parameters: RunTaskParameters,
    input: EmptyInput,
}

#[derive(Debug, Serialize)]
struct RunTaskParameters {
    format: &'static str,
    sample_rate: u32,
}

#[derive(Debug, Serialize)]
struct FinishTaskPayload {
    input: EmptyInput,
}

#[derive(Debug, Serialize)]
struct EmptyInput {}

#[derive(Debug, Deserialize)]
struct TaskEvent {
    header: TaskEventHeader,
    #[serde(default)]
    payload: Option<TaskEventPayload>,
}

#[derive(Debug, Deserialize)]
struct TaskEventHeader {
    #[serde(default)]
    event: String,
    #[serde(default)]
    error_code: Option<String>,
    #[serde(default)]
    error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TaskEventPayload {
    #[serde(default)]
    output: Option<TaskOutput>,
}

#[derive(Debug, Deserialize)]
struct TaskOutput {
    #[serde(default)]
    sentence: Option<Sentence>,
}

#[derive(Debug, Deserialize)]
struct Sentence {
    #[serde(default)]
    text: String,
    #[serde(default)]
    begin_time: Option<u64>,
    #[serde(default)]
    end_time: Option<u64>,
    #[serde(default)]
    sentence_end: bool,
    #[serde(default)]
    words: Vec<SentenceWord>,
}

#[derive(Debug, Deserialize)]
struct SentenceWord {
    #[serde(default)]
    text: String,
    #[serde(default)]
    begin_time: u64,
    #[serde(default)]
    end_time: u64,
    #[serde(default)]
    punctuation: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_streaming_models() {
        assert!(DashScopeStreamingAdapter::is_model(
            "qwen-audio-3.1-asr-flash-streaming"
        ));
        assert!(!DashScopeStreamingAdapter::is_model(
            "qwen3-asr-flash-realtime"
        ));
        assert!(!DashScopeStreamingAdapter::is_model(
            "qwen-audio-3.1-asr-flash-filetrans"
        ));
    }

    #[test]
    fn builds_inference_url_for_dashscope_hosts() {
        let params = ListenParams {
            model: Some(DASHSCOPE_STREAMING_MODEL.to_string()),
            ..Default::default()
        };
        let adapter = DashScopeStreamingAdapter::default();

        let url = adapter.build_ws_url("", &params, 1);
        assert_eq!(
            url.as_str(),
            "wss://dashscope-intl.aliyuncs.com/api-ws/v1/inference?model=qwen-audio-3.1-asr-flash-streaming"
        );

        let url = adapter.build_ws_url("wss://dashscope.aliyuncs.com", &params, 1);
        assert_eq!(url.path(), "/api-ws/v1/inference");
        assert_eq!(url.host_str(), Some("dashscope.aliyuncs.com"));

        let url = adapter.build_ws_url("https://api.anarlog.so?provider=dashscope", &params, 1);
        assert_eq!(url.path(), "/listen");
        assert!(url.as_str().contains("provider=dashscope"));
        assert!(
            url.as_str()
                .contains("model=qwen-audio-3.1-asr-flash-streaming")
        );
    }

    #[test]
    fn run_task_and_finish_task_share_task_id() {
        let adapter = DashScopeStreamingAdapter::default();
        let params = ListenParams {
            model: Some(DASHSCOPE_STREAMING_MODEL.to_string()),
            sample_rate: 16000,
            ..Default::default()
        };

        let Some(Message::Text(run)) = adapter.initial_message(None, &params, 1) else {
            panic!("expected run-task message");
        };
        let run: serde_json::Value = serde_json::from_str(&run).unwrap();
        assert_eq!(run["header"]["action"], "run-task");
        assert_eq!(run["header"]["streaming"], "duplex");
        assert_eq!(run["header"]["task_id"], adapter.task_id());
        assert_eq!(run["payload"]["task_group"], "audio");
        assert_eq!(run["payload"]["task"], "asr");
        assert_eq!(run["payload"]["function"], "recognition");
        assert_eq!(run["payload"]["model"], DASHSCOPE_STREAMING_MODEL);
        assert_eq!(run["payload"]["parameters"]["format"], "pcm");
        assert_eq!(run["payload"]["parameters"]["sample_rate"], 16000);
        assert!(run["payload"]["input"].is_object());

        let Message::Text(finish) = adapter.finalize_message() else {
            panic!("expected finish-task message");
        };
        let finish: serde_json::Value = serde_json::from_str(&finish).unwrap();
        assert_eq!(finish["header"]["action"], "finish-task");
        assert_eq!(finish["header"]["task_id"], adapter.task_id());
    }

    #[test]
    fn audio_is_sent_as_binary() {
        let adapter = DashScopeStreamingAdapter::default();
        assert!(matches!(
            adapter.audio_to_message(bytes::Bytes::from_static(&[0, 1, 2])),
            Message::Binary(_)
        ));
    }

    #[test]
    fn parses_result_generated() {
        let adapter = DashScopeStreamingAdapter::default();
        let raw = r#"{
            "header": {"task_id": "abc", "event": "result-generated", "attributes": {}},
            "payload": {"output": {"sentence": {
                "begin_time": 170,
                "end_time": 920,
                "text": "hello world",
                "sentence_begin": true,
                "sentence_end": true,
                "sentence_id": 1,
                "words": [
                    {"begin_time": 170, "end_time": 500, "text": "hello", "punctuation": ""},
                    {"begin_time": 500, "end_time": 920, "text": "world", "punctuation": "."}
                ]
            }}}
        }"#;

        let responses = adapter.parse_response(raw);
        assert_eq!(responses.len(), 1);
        let StreamResponse::TranscriptResponse {
            start,
            duration,
            is_final,
            speech_final,
            channel,
            ..
        } = &responses[0]
        else {
            panic!("expected transcript response");
        };
        assert_eq!(*start, 0.17);
        assert!((*duration - 0.75).abs() < 1e-9);
        assert!(*is_final);
        assert!(*speech_final);
        assert_eq!(channel.alternatives[0].transcript, "hello world");
        assert_eq!(channel.alternatives[0].words.len(), 2);
        assert_eq!(
            channel.alternatives[0].words[1].punctuated_word.as_deref(),
            Some("world.")
        );
    }

    #[test]
    fn parses_partial_result_as_interim() {
        let adapter = DashScopeStreamingAdapter::default();
        let raw = r#"{
            "header": {"task_id": "abc", "event": "result-generated"},
            "payload": {"output": {"sentence": {"begin_time": 0, "text": "hel", "sentence_end": false}}}
        }"#;

        let responses = adapter.parse_response(raw);
        let StreamResponse::TranscriptResponse { is_final, .. } = &responses[0] else {
            panic!("expected transcript response");
        };
        assert!(!*is_final);
    }

    #[test]
    fn parses_task_lifecycle_events() {
        let adapter = DashScopeStreamingAdapter::default();

        assert!(
            adapter
                .parse_response(
                    r#"{"header": {"task_id": "abc", "event": "task-started"}, "payload": {}}"#
                )
                .is_empty()
        );
        let finished = adapter.parse_response(
            r#"{"header": {"task_id": "abc", "event": "task-finished"}, "payload": {"output": {}}}"#,
        );
        assert!(matches!(
            &finished[..],
            [StreamResponse::TranscriptResponse {
                from_finalize: true,
                is_final: true,
                ..
            }]
        ));

        let raw = r#"{"header": {"task_id": "abc", "event": "task-failed", "error_code": "InvalidParameter", "error_message": "bad format"}, "payload": {}}"#;
        let responses = adapter.parse_response(raw);
        let StreamResponse::ErrorResponse {
            error_message,
            provider,
            ..
        } = &responses[0]
        else {
            panic!("expected error response");
        };
        assert_eq!(error_message, "InvalidParameter: bad format");
        assert_eq!(provider, "dashscope");
    }

    #[test]
    fn fork_session_uses_distinct_task_id() {
        let adapter = DashScopeStreamingAdapter::default();
        let forked = adapter.fork_session();
        assert_ne!(adapter.task_id(), forked.task_id());
        assert_eq!(adapter.clone().task_id(), adapter.task_id());
    }
}
