use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use owhisper_client::{FinalizeHandle, ListenClient, ListenClientInput, RealtimeSttAdapter};
use owhisper_interface::stream::StreamResponse;
use tauri::ipc::Channel;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::DictationUpdate;
use super::wav::SAMPLE_RATE;
use crate::preview::{PreviewConfig, PreviewTranscript, is_supported_url, with_realtime_adapter};

// Char's DictationStream FINISH_TIMEOUT_MS (dictation-stream.ts, d4608723).
pub const FINISH_DEADLINE: Duration = Duration::from_secs(10);

#[derive(Default)]
struct State {
    transcript: PreviewTranscript,
    committed_end: f64,
}

/// What a stream proved by the time it ended.
pub struct StreamReport {
    pub text: String,
    /// The stream finalized every word it was sent.
    pub complete: bool,
    /// Final text and the audio boundary (in samples) it covers. Only for providers whose finals
    /// carry audio-relative timing; None means only a full re-transcription is safe.
    pub committed: Option<(String, u64)>,
}

/// One provider stream for one operation. Audio is sent in recording order without gaps: once a
/// send fails the stream stops taking audio, so whatever it committed is still a prefix.
pub struct LiveStream {
    sender: Option<mpsc::Sender<ListenClientInput>>,
    task: Option<tauri::async_runtime::JoinHandle<Result<(), ()>>>,
    cancellation: CancellationToken,
    state: Arc<Mutex<State>>,
    proven_timing: bool,
    audio_complete: bool,
}

impl LiveStream {
    pub fn start(
        config: PreviewConfig,
        capacity: usize,
        updates: Channel<DictationUpdate>,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(capacity);
        let cancellation = CancellationToken::new();
        let state = Arc::new(Mutex::new(State {
            transcript: PreviewTranscript {
                append_turns: config.provider == "cartesia",
                ..Default::default()
            },
            ..Default::default()
        }));
        let proven_timing = has_proven_timing(&config);
        let cancelled = cancellation.clone();
        let task_state = state.clone();
        let task = tauri::async_runtime::spawn(async move {
            tokio::select! {
                biased;
                _ = cancelled.cancelled() => Err(()),
                result = open(config, receiver, &task_state, &updates) => {
                    if result.is_err() {
                        let _ = updates.send(DictationUpdate::PreviewUnavailable);
                    }
                    result
                }
            }
        });
        Self {
            sender: Some(sender),
            task: Some(task),
            cancellation,
            state,
            proven_timing,
            audio_complete: true,
        }
    }

    /// False once the stream no longer takes audio.
    pub fn send(&mut self, pcm: &[i16]) -> bool {
        let Some(sender) = &self.sender else {
            return false;
        };
        let bytes: Vec<u8> = pcm.iter().flat_map(|sample| sample.to_le_bytes()).collect();
        if sender
            .try_send(ListenClientInput::Audio(bytes.into()))
            .is_ok()
        {
            return true;
        }
        self.audio_complete = false;
        self.sender = None;
        false
    }

    /// Ends the audio and waits for the provider to finalize it, at most `deadline`.
    pub async fn finish(mut self, deadline: Duration) -> StreamReport {
        self.sender.take();
        let complete = match self.task.take() {
            Some(task) => matches!(tokio::time::timeout(deadline, task).await, Ok(Ok(Ok(())))),
            None => false,
        };
        self.cancellation.cancel();
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let text = state.transcript.text();
        let committed = (self.proven_timing && state.committed_end > 0.0).then(|| {
            let boundary = (state.committed_end * f64::from(SAMPLE_RATE)).round() as u64;
            (text.clone(), boundary)
        });
        StreamReport {
            text,
            complete: complete && self.audio_complete,
            committed,
        }
    }
}

impl Drop for LiveStream {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

/// Providers whose final results carry start/duration measured on the audio sent, so the end of
/// the last final is a boundary nothing before it changes. Other providers (turn-based, zero-start
/// or proxied) only ever fall back to the whole recording.
fn has_proven_timing(config: &PreviewConfig) -> bool {
    match config.provider.as_str() {
        "soniox" => true,
        "deepgram" => !config
            .params
            .model
            .as_deref()
            .is_some_and(owhisper_client::DeepgramFluxAdapter::is_model),
        _ => false,
    }
}

async fn open(
    config: PreviewConfig,
    receiver: mpsc::Receiver<ListenClientInput>,
    state: &Mutex<State>,
    updates: &Channel<DictationUpdate>,
) -> Result<(), ()> {
    if !is_supported_url(&config.base_url) {
        return Err(());
    }
    with_realtime_adapter!(config, listen(config, receiver, state, updates))
}

async fn listen<A: RealtimeSttAdapter>(
    mut config: PreviewConfig,
    mut receiver: mpsc::Receiver<ListenClientInput>,
    state: &Mutex<State>,
    updates: &Channel<DictationUpdate>,
) -> Result<(), ()> {
    config.params.channels = 1;
    config.params.sample_rate = SAMPLE_RATE;
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
    let audio = futures_util::stream::poll_fn(move |cx| {
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
                let update = {
                    let mut state = state.lock().unwrap_or_else(|error| error.into_inner());
                    if let StreamResponse::TranscriptResponse { start, duration, is_final: true, .. } = &response
                        && start.is_finite()
                        && duration.is_finite()
                        && *duration >= 0.0
                    {
                        state.committed_end = state.committed_end.max(start + duration);
                    }
                    let update = state.transcript.update(response);
                    if finalizing && finished {
                        let pending = state.transcript.partial.as_ref().is_some_and(|(_, text)| !text.is_empty());
                        return if pending { Err(()) } else { Ok(()) };
                    }
                    update
                };
                if let Some(update) = update {
                    updates.send(update).map_err(|_| ())?;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use futures_util::SinkExt;
    use owhisper_interface::ListenParams;
    use owhisper_interface::stream::{Alternatives, Channel as TranscriptChannel, Metadata};
    use tauri::ipc::InvokeResponseBody;
    use tokio_tungstenite::tungstenite::Message;

    use super::*;

    fn config(provider: &str, address: std::net::SocketAddr) -> PreviewConfig {
        PreviewConfig {
            provider: provider.into(),
            base_url: format!("http://{address}"),
            api_key: "test".into(),
            params: ListenParams::default(),
        }
    }

    fn response(text: &str, is_final: bool) -> StreamResponse {
        StreamResponse::TranscriptResponse {
            start: 0.0,
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

    #[tokio::test]
    async fn wispr_finalizes_after_all_audio_and_reports_the_final_text() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(socket).await.unwrap();
            let auth = ws.next().await.unwrap().unwrap().into_text().unwrap();
            let auth: serde_json::Value = serde_json::from_str(&auth).unwrap();
            assert_eq!(auth["type"], "auth");
            assert_eq!(auth["access_token"], "test");
            ws.send(Message::Text(r#"{"status":"auth"}"#.into()))
                .await
                .unwrap();
            for position in 0..4 {
                let packet = loop {
                    if let Message::Text(text) = ws.next().await.unwrap().unwrap() {
                        break text;
                    }
                };
                let packet: serde_json::Value = serde_json::from_str(&packet).unwrap();
                assert_eq!(packet["type"], "append");
                assert_eq!(packet["position"], position);
                assert_eq!(packet["audio_packets"]["packet_duration"], 0.1);
            }
            let commit = ws.next().await.unwrap().unwrap().into_text().unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&commit).unwrap(),
                serde_json::json!({"type":"commit","total_packets":4})
            );
            ws.send(Message::Text(
                r#"{"status":"text","final":false,"body":{"text":"unfinished"}}"#.into(),
            ))
            .await
            .unwrap();
            ws.send(Message::Text(
                r#"{"status":"text","final":true,"body":{"text":"Finished sentence."}}"#.into(),
            ))
            .await
            .unwrap();
        });
        let mut stream =
            LiveStream::start(config("wisprflow", address), 8, Channel::new(|_| Ok(())));
        for _ in 0..3 {
            assert!(stream.send(&[16_384; 1_600]));
        }
        assert!(stream.send(&[16_384; 300]));
        let report = tokio::time::timeout(Duration::from_secs(3), stream.finish(FINISH_DEADLINE))
            .await
            .unwrap();
        assert!(report.complete);
        assert_eq!(report.text, "Finished sentence.");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn shows_live_words_and_closes_the_connection_when_dropped() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(socket).await.unwrap();
            let audio = ws.next().await.unwrap().unwrap();
            assert!(matches!(audio, Message::Binary(ref bytes) if bytes.len() == 6));
            for event in [response("hello", false), response("Hello!", true)] {
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
        let mut stream = LiveStream::start(config("anarlog", address), 8, updates);
        assert!(stream.send(&[0, 16_384, i16::MAX]));
        tokio::time::timeout(Duration::from_secs(3), async {
            while values.lock().unwrap().len() < 2 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(values.lock().unwrap()[0]["partial"], "hello");
        assert_eq!(values.lock().unwrap()[1]["text"], "Hello!");
        drop(stream);
        tokio::time::timeout(Duration::from_secs(3), server)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(values.lock().unwrap().len(), 2);
    }
}
