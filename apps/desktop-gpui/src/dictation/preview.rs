//! Shipping's live dictation preview (`plugins/dictation/src/preview.rs`): streams the
//! recording to the selected realtime provider so words appear in the panel while speaking.
//! A failed preview never interrupts the WAV recording; the batch transcript is used instead.

use futures::StreamExt;
use owhisper_client::{FinalizeHandle, ListenClient, ListenClientInput, RealtimeSttAdapter};
use owhisper_interface::{ListenParams, stream::StreamResponse};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::recorder::{Update, Updates};

#[derive(Clone, Debug)]
pub struct PreviewConfig {
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub params: ListenParams,
}

pub struct Preview {
    sender: Option<mpsc::Sender<ListenClientInput>>,
    task: Option<JoinHandle<Result<String, ()>>>,
    cancellation: CancellationToken,
}

impl Preview {
    pub fn start(
        runtime: &tokio::runtime::Handle,
        config: PreviewConfig,
        updates: Updates,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(32);
        let cancellation = CancellationToken::new();
        let cancelled = cancellation.clone();
        let task = runtime.spawn(async move {
            tokio::select! {
                biased;
                _ = cancelled.cancelled() => Err(()),
                result = run(config, receiver, &updates) => {
                    if result.is_err() {
                        updates(Update::PreviewUnavailable);
                    }
                    result
                }
            }
        });
        Self {
            sender: Some(sender),
            task: Some(task),
            cancellation,
        }
    }

    /// Queues 16-bit PCM; `false` once the provider cannot keep up, ending the preview.
    pub fn send(&self, samples: &[f32]) -> bool {
        let bytes: Vec<u8> = samples
            .iter()
            .flat_map(|sample| {
                (((*sample).clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16).to_le_bytes()
            })
            .collect();
        self.sender.as_ref().is_some_and(|sender| {
            sender
                .try_send(ListenClientInput::Audio(bytes.into()))
                .is_ok()
        })
    }

    pub async fn finish(mut self) -> Option<String> {
        self.sender.take();
        let task = self.task.take()?;
        tokio::time::timeout(std::time::Duration::from_secs(15), task)
            .await
            .ok()?
            .ok()?
            .ok()
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

async fn run(
    config: PreviewConfig,
    receiver: mpsc::Receiver<ListenClientInput>,
    updates: &Updates,
) -> Result<String, ()> {
    use owhisper_client::*;
    let url = url::Url::parse(&config.base_url).map_err(|_| ())?;
    if !matches!(url.scheme(), "http" | "https" | "ws" | "wss") || url.host_str().is_none() {
        return Err(());
    }
    let model = config.params.model.as_deref();
    match config.provider.as_str() {
        "anarlog" => listen::<AnarlogAdapter>(config, receiver, updates).await,
        "deepgram" if model.is_some_and(DeepgramFluxAdapter::is_model) => {
            listen::<DeepgramFluxAdapter>(config, receiver, updates).await
        }
        "deepgram" => listen::<DeepgramAdapter>(config, receiver, updates).await,
        "soniox" => listen::<SonioxAdapter>(config, receiver, updates).await,
        "assemblyai" => listen::<AssemblyAIAdapter>(config, receiver, updates).await,
        "openai" => listen::<OpenAIAdapter>(config, receiver, updates).await,
        "cartesia" => listen::<CartesiaAdapter>(config, receiver, updates).await,
        "elevenlabs" => listen::<ElevenLabsAdapter>(config, receiver, updates).await,
        "gladia" => listen::<GladiaAdapter>(config, receiver, updates).await,
        "meta" => listen::<MetaAdapter>(config, receiver, updates).await,
        "dashscope" if model.is_some_and(DashScopeStreamingAdapter::is_model) => {
            listen::<DashScopeStreamingAdapter>(config, receiver, updates).await
        }
        "dashscope" => listen::<DashScopeAdapter>(config, receiver, updates).await,
        "wisprflow" => listen::<WisprFlowAdapter>(config, receiver, updates).await,
        "smallestai" => listen::<SmallestAIAdapter>(config, receiver, updates).await,
        "fireworks" => listen::<FireworksAdapter>(config, receiver, updates).await,
        "mistral" => listen::<MistralAdapter>(config, receiver, updates).await,
        "xai" => listen::<XaiAdapter>(config, receiver, updates).await,
        "argmax" => listen::<ArgmaxAdapter>(config, receiver, updates).await,
        "nari" => listen::<NariAdapter>(config, receiver, updates).await,
        "google_generative_ai" => {
            listen::<GoogleGenerativeAiAdapter>(config, receiver, updates).await
        }
        _ => Err(()),
    }
}

async fn listen<A: RealtimeSttAdapter>(
    mut config: PreviewConfig,
    mut receiver: mpsc::Receiver<ListenClientInput>,
    updates: &Updates,
) -> Result<String, ()> {
    config.params.channels = 1;
    config.params.sample_rate = super::recorder::SAMPLE_RATE;
    let mut transcript = Transcript {
        append_turns: config.provider == "cartesia",
        ..Default::default()
    };
    let client = ListenClient::builder()
        .adapter::<A>()
        .api_base(config.base_url)
        .api_key(config.api_key)
        .params(config.params)
        .build_single()
        .await
        .map_err(|_| ())?;
    let (ended_tx, mut ended_rx) = tokio::sync::oneshot::channel();
    let mut ended_tx = Some(ended_tx);
    let audio = futures::stream::poll_fn(move |cx| {
        let result = receiver.poll_recv(cx);
        if matches!(result, std::task::Poll::Ready(None))
            && let Some(sender) = ended_tx.take()
        {
            let _ = sender.send(());
        }
        result
    });
    let (responses, handle) = client.from_realtime_audio(audio).await.map_err(|_| ())?;
    tokio::pin!(responses);
    let mut finalizing = false;
    loop {
        tokio::select! {
            _ = &mut ended_rx, if !finalizing => {
                finalizing = true;
                handle.finalize().await;
            }
            response = responses.next() => {
                let response = response.ok_or(())?.map_err(|_| ())?;
                if matches!(response, StreamResponse::ErrorResponse { .. }) {
                    return Err(());
                }
                let finished = matches!(response, StreamResponse::TranscriptResponse { from_finalize: true, .. });
                if let Some(update) = transcript.update(response) {
                    updates(update);
                }
                if finalizing && finished {
                    if transcript.partial.as_ref().is_some_and(|(_, text)| !text.is_empty()) {
                        return Err(());
                    }
                    return Ok(transcript.text());
                }
            }
        }
    }
}

/// Final segments by start time plus the newest partial, as the panel shows them.
#[derive(Default)]
struct Transcript {
    append_turns: bool,
    segments: Vec<(f64, String)>,
    partial: Option<(f64, String)>,
}

impl Transcript {
    fn text(&self) -> String {
        self.segments
            .iter()
            .map(|segment| segment.1.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn update(&mut self, response: StreamResponse) -> Option<Update> {
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
        Some(Update::Transcript {
            text: self.text(),
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
    use owhisper_interface::stream::{Alternatives, Channel, Metadata};

    fn response(start: f64, text: &str, is_final: bool) -> StreamResponse {
        StreamResponse::TranscriptResponse {
            start,
            duration: 1.0,
            is_final,
            speech_final: is_final,
            from_finalize: false,
            channel: Channel {
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

    fn shown(update: Option<Update>) -> (String, String) {
        match update {
            Some(Update::Transcript { text, partial }) => (text, partial),
            other => panic!("expected a transcript, got {other:?}"),
        }
    }

    #[test]
    fn partials_are_replaced_and_final_retries_are_not_duplicated() {
        let mut transcript = Transcript::default();
        transcript.update(response(0.0, "hel", false));
        transcript.update(response(0.0, "hello", false));
        transcript.update(response(0.0, "Hello.", true));
        transcript.update(response(0.0, "Hello!", true));
        assert_eq!(
            shown(transcript.update(response(1.0, "next", false))),
            ("Hello!".into(), "next".into())
        );
        assert_eq!(
            shown(transcript.update(response(0.0, "Hello!", true))),
            ("Hello!".into(), "next".into())
        );
    }

    #[test]
    fn cartesia_appends_turns_that_reuse_the_same_start() {
        let mut transcript = Transcript {
            append_turns: true,
            ..Default::default()
        };
        transcript.update(response(0.0, "first", false));
        transcript.update(response(0.0, "First.", true));
        assert_eq!(
            shown(transcript.update(response(0.0, "second", false))),
            ("First.".into(), "second".into())
        );
        assert_eq!(
            shown(transcript.update(response(0.0, "Second.", true))),
            ("First. Second.".into(), String::new())
        );
    }

    #[test]
    fn audio_is_sent_as_pcm_and_a_full_queue_ends_the_preview() {
        let (sender, mut receiver) = mpsc::channel(1);
        let cancellation = CancellationToken::new();
        let preview = Preview {
            sender: Some(sender),
            task: None,
            cancellation: cancellation.clone(),
        };
        assert!(preview.send(&[-1.0, 0.0, 1.0]));
        assert!(!preview.send(&[0.5]));
        let Ok(ListenClientInput::Audio(bytes)) = receiver.try_recv() else {
            panic!("expected audio")
        };
        assert_eq!(bytes.as_ref(), &[1, 128, 0, 0, 255, 127]);
        drop(preview);
        assert!(cancellation.is_cancelled());
    }

    #[test]
    fn an_unsupported_provider_reports_the_preview_unavailable() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (sender, receiver) = std::sync::mpsc::channel();
        let preview = Preview::start(
            runtime.handle(),
            PreviewConfig {
                provider: "unsupported".into(),
                base_url: "http://localhost".into(),
                api_key: String::new(),
                params: ListenParams::default(),
            },
            std::sync::Arc::new(move |update| {
                let _ = sender.send(update);
            }),
        );
        assert_eq!(runtime.block_on(preview.finish()), None);
        assert_eq!(receiver.try_recv(), Ok(Update::PreviewUnavailable));
    }
}
