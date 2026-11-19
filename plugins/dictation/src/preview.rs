//! The live stream's provider settings, the adapter that streams each provider, and the words a
//! stream shows while the person speaks.
use owhisper_interface::{ListenParams, stream::StreamResponse};

use crate::engine::DictationUpdate;

#[derive(Clone, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PreviewConfig {
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub params: ListenParams,
}

/// Calls `$listen::<Adapter>($args)` for the adapter that streams `$config.provider`.
macro_rules! with_realtime_adapter {
    ($config:ident, $listen:ident ( $($arg:expr),* )) => {{
        use owhisper_client::*;
        match $config.provider.as_str() {
            "anarlog" => $listen::<AnarlogAdapter>($($arg),*).await,
            "deepgram"
                if $config
                    .params
                    .model
                    .as_deref()
                    .is_some_and(DeepgramFluxAdapter::is_model) =>
            {
                $listen::<DeepgramFluxAdapter>($($arg),*).await
            }
            "deepgram" => $listen::<DeepgramAdapter>($($arg),*).await,
            "soniox" => $listen::<SonioxAdapter>($($arg),*).await,
            "assemblyai" => $listen::<AssemblyAIAdapter>($($arg),*).await,
            "openai" => $listen::<OpenAIAdapter>($($arg),*).await,
            "cartesia" => $listen::<CartesiaAdapter>($($arg),*).await,
            "elevenlabs" => $listen::<ElevenLabsAdapter>($($arg),*).await,
            "gladia" => $listen::<GladiaAdapter>($($arg),*).await,
            "meta" => $listen::<MetaAdapter>($($arg),*).await,
            "dashscope"
                if $config
                    .params
                    .model
                    .as_deref()
                    .is_some_and(DashScopeStreamingAdapter::is_model) =>
            {
                $listen::<DashScopeStreamingAdapter>($($arg),*).await
            }
            "dashscope" => $listen::<DashScopeAdapter>($($arg),*).await,
            "wisprflow" => $listen::<WisprFlowAdapter>($($arg),*).await,
            "smallestai" => $listen::<SmallestAIAdapter>($($arg),*).await,
            "fireworks" => $listen::<FireworksAdapter>($($arg),*).await,
            "mistral" => $listen::<MistralAdapter>($($arg),*).await,
            "xai" => $listen::<XaiAdapter>($($arg),*).await,
            "argmax" => $listen::<ArgmaxAdapter>($($arg),*).await,
            "nari" => $listen::<NariAdapter>($($arg),*).await,
            "google_generative_ai" => {
                $listen::<GoogleGenerativeAiAdapter>($($arg),*).await
            }
            _ => Err(()),
        }
    }};
}
pub(crate) use with_realtime_adapter;

pub(crate) fn is_supported_url(base_url: &str) -> bool {
    url::Url::parse(base_url).is_ok_and(|url| {
        matches!(url.scheme(), "http" | "https" | "ws" | "wss") && url.host_str().is_some()
    })
}

#[derive(Default)]
pub(crate) struct PreviewTranscript {
    pub(crate) append_turns: bool,
    pub(crate) segments: Vec<(f64, String)>,
    pub(crate) partial: Option<(f64, String)>,
}

impl PreviewTranscript {
    pub(crate) fn text(&self) -> String {
        self.segments
            .iter()
            .map(|segment| segment.1.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub(crate) fn update(&mut self, response: StreamResponse) -> Option<DictationUpdate> {
        let StreamResponse::TranscriptResponse {
            start,
            is_final,
            channel,
            ..
        } = response
        else {
            return None;
        };
        let text = channel.alternatives.first()?.transcript.trim().to_string();
        if !start.is_finite() {
            return None;
        }
        // Cartesia emits ordered turns with a zero start and a connection-wide request ID.
        let start = if self.append_turns {
            self.segments.len() as f64
        } else {
            start
        };
        if is_final && !text.is_empty() {
            if let Some(segment) = self.segments.iter_mut().find(|segment| segment.0 == start) {
                segment.1 = text.clone();
            } else {
                self.segments.push((start, text.clone()));
                self.segments.sort_by(|a, b| a.0.total_cmp(&b.0));
            }
        }
        if is_final && !text.is_empty() {
            if self
                .partial
                .as_ref()
                .is_some_and(|partial| partial.0 <= start)
            {
                self.partial = None;
            }
        } else if !self.segments.iter().any(|segment| segment.0 == start) {
            self.partial = Some((start, text));
        }
        Some(DictationUpdate::Transcript {
            text: self
                .segments
                .iter()
                .map(|segment| segment.1.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            partial: self
                .partial
                .as_ref()
                .map(|partial| partial.1.clone())
                .unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use owhisper_interface::stream::{Alternatives, Channel as TranscriptChannel, Metadata};

    fn response(start: f64, text: &str, is_final: bool) -> StreamResponse {
        StreamResponse::TranscriptResponse {
            start,
            duration: 1.0,
            is_final,
            speech_final: is_final,
            from_finalize: false,
            channel: TranscriptChannel {
                alternatives: vec![Alternatives {
                    transcript: text.into(),
                    words: vec![],
                    confidence: 1.0,
                    languages: vec![],
                }],
            },
            metadata: Metadata::default(),
            channel_index: vec![0, 1],
        }
    }

    #[test]
    fn cartesia_appends_turns_that_reuse_the_same_start() {
        let mut transcript = PreviewTranscript {
            append_turns: true,
            ..Default::default()
        };
        transcript.update(response(0.0, "first", false));
        transcript.update(response(0.0, "First.", true));
        let Some(DictationUpdate::Transcript { text, partial }) =
            transcript.update(response(0.0, "second", false))
        else {
            panic!()
        };
        assert_eq!(text, "First.");
        assert_eq!(partial, "second");
        let Some(DictationUpdate::Transcript { text, partial }) =
            transcript.update(response(0.0, "Second.", true))
        else {
            panic!()
        };
        assert_eq!(text, "First. Second.");
        assert!(partial.is_empty());
    }

    #[test]
    fn partials_are_replaced_and_final_retries_are_not_duplicated() {
        let mut transcript = PreviewTranscript::default();
        transcript.update(response(0.0, "hel", false));
        transcript.update(response(0.0, "hello", false));
        transcript.update(response(0.0, "Hello.", true));
        transcript.update(response(0.0, "Hello!", true));
        let Some(DictationUpdate::Transcript { text, partial }) =
            transcript.update(response(1.0, "next", false))
        else {
            panic!()
        };
        assert_eq!(text, "Hello!");
        assert_eq!(partial, "next");
        let Some(DictationUpdate::Transcript { text, partial }) =
            transcript.update(response(0.0, "Hello!", true))
        else {
            panic!()
        };
        assert_eq!(text, "Hello!");
        assert_eq!(partial, "next");
    }
}
