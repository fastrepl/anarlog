use futures_util::StreamExt;
use owhisper_client::{ListenClient, ListenClientInput, RealtimeSttAdapter};
use owhisper_interface::{ListenParams, stream::StreamResponse};
use tauri::ipc::Channel;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Clone, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PreviewConfig {
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub params: ListenParams,
}

#[derive(Clone, serde::Serialize, specta::Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RecordingUpdate {
    Amplitude { amplitude: f64 },
    Transcript { text: String, partial: String },
    PreviewUnavailable,
}

pub struct Preview {
    sender: mpsc::Sender<ListenClientInput>,
    cancellation: CancellationToken,
}

impl Preview {
    pub fn start(config: PreviewConfig, updates: Channel<RecordingUpdate>) -> Self {
        let (sender, receiver) = mpsc::channel(32);
        let cancellation = CancellationToken::new();
        let cancelled = cancellation.clone();
        tauri::async_runtime::spawn(async move {
            tokio::select! {
                biased;
                _ = cancelled.cancelled() => {},
                result = run(config, receiver, &updates) => {
                    if result.is_err() {
                        let _ = updates.send(RecordingUpdate::PreviewUnavailable);
                    }
                }
            }
        });
        Self {
            sender,
            cancellation,
        }
    }

    pub fn send(&self, samples: &[f32]) -> bool {
        let bytes: Vec<u8> = samples
            .iter()
            .flat_map(|sample| {
                (((*sample).clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16).to_le_bytes()
            })
            .collect();
        // WAV capture must never wait for a slow preview connection.
        !matches!(
            self.sender.try_send(ListenClientInput::Audio(bytes.into())),
            Err(mpsc::error::TrySendError::Closed(_))
        )
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
    updates: &Channel<RecordingUpdate>,
) -> Result<(), ()> {
    use owhisper_client::*;
    let url = url::Url::parse(&config.base_url).map_err(|_| ())?;
    if !matches!(url.scheme(), "http" | "https" | "ws" | "wss") || url.host_str().is_none() {
        return Err(());
    }
    match config.provider.as_str() {
        "anarlog" => listen::<AnarlogAdapter>(config, receiver, updates).await,
        "deepgram" => listen::<DeepgramAdapter>(config, receiver, updates).await,
        "soniox" => listen::<SonioxAdapter>(config, receiver, updates).await,
        "assemblyai" => listen::<AssemblyAIAdapter>(config, receiver, updates).await,
        "openai" => listen::<OpenAIAdapter>(config, receiver, updates).await,
        "cartesia" => listen::<CartesiaAdapter>(config, receiver, updates).await,
        "elevenlabs" => listen::<ElevenLabsAdapter>(config, receiver, updates).await,
        "gladia" => listen::<GladiaAdapter>(config, receiver, updates).await,
        "meta" => listen::<MetaAdapter>(config, receiver, updates).await,
        "dashscope" => listen::<DashScopeAdapter>(config, receiver, updates).await,
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
    receiver: mpsc::Receiver<ListenClientInput>,
    updates: &Channel<RecordingUpdate>,
) -> Result<(), ()> {
    config.params.channels = 1;
    config.params.sample_rate = 16_000;
    let client = ListenClient::builder()
        .adapter::<A>()
        .api_base(config.base_url)
        .api_key(config.api_key)
        .params(config.params)
        .build_single()
        .await
        .map_err(|_| ())?;
    let (responses, _handle) = client
        .from_realtime_audio(tokio_stream::wrappers::ReceiverStream::new(receiver))
        .await
        .map_err(|_| ())?;
    tokio::pin!(responses);
    let mut transcript = PreviewTranscript::default();
    while let Some(response) = responses.next().await {
        let response = response.map_err(|_| ())?;
        if matches!(response, StreamResponse::ErrorResponse { .. }) {
            return Err(());
        }
        if let Some(update) = transcript.update(response) {
            updates.send(update).map_err(|_| ())?;
        }
    }
    Err(())
}

#[derive(Default)]
struct PreviewTranscript {
    segments: Vec<(f64, String)>,
    partial: Option<(f64, String)>,
}

impl PreviewTranscript {
    fn update(&mut self, response: StreamResponse) -> Option<RecordingUpdate> {
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
        if is_final && !text.is_empty() {
            if let Some(segment) = self.segments.iter_mut().find(|segment| segment.0 == start) {
                segment.1 = text.clone();
            } else {
                self.segments.push((start, text.clone()));
                self.segments.sort_by(|a, b| a.0.total_cmp(&b.0));
            }
        }
        if is_final {
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
        Some(RecordingUpdate::Transcript {
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
    use futures_util::SinkExt;
    use owhisper_interface::stream::{Alternatives, Channel as TranscriptChannel, Metadata};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tauri::ipc::InvokeResponseBody;
    use tokio_tungstenite::tungstenite::Message;

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
    fn queue_pressure_does_not_disable_a_healthy_preview() {
        let (sender, mut receiver) = mpsc::channel(1);
        let preview = Preview {
            sender,
            cancellation: CancellationToken::new(),
        };
        assert!(preview.send(&[0.0]));
        assert!(preview.send(&[0.1]));
        assert!(receiver.try_recv().is_ok());
        assert!(preview.send(&[0.2]));
        drop(receiver);
        assert!(!preview.send(&[0.3]));
    }

    #[test]
    fn partials_are_replaced_and_final_retries_are_not_duplicated() {
        let mut transcript = PreviewTranscript::default();
        transcript.update(response(0.0, "hel", false));
        transcript.update(response(0.0, "hello", false));
        transcript.update(response(0.0, "Hello.", true));
        transcript.update(response(0.0, "Hello!", true));
        let Some(RecordingUpdate::Transcript { text, partial }) =
            transcript.update(response(1.0, "next", false))
        else {
            panic!()
        };
        assert_eq!(text, "Hello!");
        assert_eq!(partial, "next");
        let Some(RecordingUpdate::Transcript { text, partial }) =
            transcript.update(response(0.0, "Hello!", true))
        else {
            panic!()
        };
        assert_eq!(text, "Hello!");
        assert_eq!(partial, "next");
    }

    #[tokio::test]
    async fn converts_audio_to_pcm_and_drops_overflow_without_disabling_preview() {
        let (sender, mut receiver) = mpsc::channel(1);
        let cancellation = CancellationToken::new();
        let preview = Preview {
            sender,
            cancellation: cancellation.clone(),
        };
        assert!(preview.send(&[-1.0, 0.0, 1.0]));
        assert!(preview.send(&[0.5]));
        let ListenClientInput::Audio(bytes) = receiver.recv().await.unwrap() else {
            panic!()
        };
        assert_eq!(bytes.as_ref(), &[1, 128, 0, 0, 255, 127]);
        drop(preview);
        assert!(cancellation.is_cancelled());
    }

    #[tokio::test]
    async fn streams_live_words_and_closes_the_connection_on_cancel() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(socket).await.unwrap();
            let audio = ws.next().await.unwrap().unwrap();
            assert!(matches!(audio, Message::Binary(ref bytes) if bytes.len() == 6));
            for event in [response(0.0, "hello", false), response(0.0, "Hello!", true)] {
                ws.send(Message::Text(serde_json::to_string(&event).unwrap().into()))
                    .await
                    .unwrap();
            }
            loop {
                match ws.next().await {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(error)) => panic!("Unexpected WebSocket error: {error}"),
                    _ => {}
                }
            }
        });
        let values = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let captured = values.clone();
        let updates = Channel::new(move |body| {
            if let InvokeResponseBody::Json(body) = body {
                captured
                    .lock()
                    .unwrap()
                    .push(serde_json::from_str(&body).unwrap());
            }
            Ok(())
        });
        let preview = Preview::start(
            PreviewConfig {
                provider: "anarlog".into(),
                base_url: format!("http://{address}"),
                api_key: "test".into(),
                params: ListenParams::default(),
            },
            updates,
        );
        assert!(preview.send(&[0.0, 0.5, 1.0]));
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if values.lock().unwrap().len() >= 2 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(values.lock().unwrap()[0]["partial"], "hello");
        assert_eq!(values.lock().unwrap()[1]["text"], "Hello!");
        drop(preview);
        tokio::time::timeout(Duration::from_secs(3), server)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(values.lock().unwrap().len(), 2);
    }
}
