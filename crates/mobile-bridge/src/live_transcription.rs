use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use futures_util::{FutureExt, Stream, StreamExt};
use owhisper_client::{
    AdapterKind, AssemblyAIAdapter, CartesiaAdapter, DashScopeAdapter, DeepgramAdapter,
    ElevenLabsAdapter, FinalizeHandle, GladiaAdapter, GoogleGenerativeAiAdapter, ListenClient,
    ListenClientInput, MistralAdapter, OpenAIAdapter, RealtimeSttAdapter, SmallestAIAdapter,
    SonioxAdapter, XaiAdapter,
};
use owhisper_interface::{ListenParams, MixedMessage, stream::StreamResponse};
use serde::Deserialize;
use tokio::sync::{mpsc, watch};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

use crate::transcription::{ProviderTranscriptionError, validate_provider_settings};

const AUDIO_CHUNK_BYTES: usize = 3_200;
const QUEUED_AUDIO_CHUNKS: usize = 100;
const FINALIZE_TIMEOUT: Duration = Duration::from_secs(5);

#[uniffi::export(with_foreign)]
pub trait TranscriptionEventListener: Send + Sync {
    fn on_message(&self, message_json: String);
    fn on_error(&self);
}

#[derive(Deserialize)]
struct Request {
    provider: String,
    base_url: String,
    api_key: String,
    params: ListenParams,
}

#[derive(uniffi::Object)]
pub struct ProviderLiveTranscription {
    audio: Mutex<Option<mpsc::Sender<ListenClientInput>>>,
    finish: CancellationToken,
    cancel: CancellationToken,
    completed: watch::Receiver<Option<bool>>,
}

#[uniffi::export(async_runtime = "tokio")]
pub async fn start_provider_live_transcription(
    request_json: String,
    listener: Arc<dyn TranscriptionEventListener>,
) -> Result<Arc<ProviderLiveTranscription>, ProviderTranscriptionError> {
    let mut request: Request = serde_json::from_str(&request_json)
        .map_err(|_| ProviderTranscriptionError::InvalidSettings)?;
    let adapter = validate_provider_settings(
        &request.provider,
        &request.base_url,
        &request.api_key,
        &request.params,
    )?;
    if request.params.channels != 1 || request.params.sample_rate != 16_000 {
        return Err(ProviderTranscriptionError::InvalidSettings);
    }
    // Match desktop's primary-language fallback for streaming providers that
    // cannot recognize all selected languages together.
    if !adapter
        .is_supported_languages_live(&request.params.languages, request.params.model.as_deref())
    {
        request.params.languages.truncate(1);
        if !adapter
            .is_supported_languages_live(&request.params.languages, request.params.model.as_deref())
        {
            return Err(ProviderTranscriptionError::InvalidSettings);
        }
    }
    tokio::time::timeout(
        Duration::from_secs(15),
        dispatch(adapter, request, listener),
    )
    .await
    .map_err(|_| ProviderTranscriptionError::TimedOut)?
}

async fn dispatch(
    adapter: AdapterKind,
    request: Request,
    listener: Arc<dyn TranscriptionEventListener>,
) -> Result<Arc<ProviderLiveTranscription>, ProviderTranscriptionError> {
    macro_rules! adapters {
        ($($variant:ident => $adapter:ty),+ $(,)?) => {
            match adapter {
                $(AdapterKind::$variant => start::<$adapter>(request, listener).await,)+
                _ => Err(ProviderTranscriptionError::InvalidSettings),
            }
        };
    }
    adapters! {
        AssemblyAI => AssemblyAIAdapter,
        Cartesia => CartesiaAdapter,
        DashScope => DashScopeAdapter,
        Deepgram => DeepgramAdapter,
        ElevenLabs => ElevenLabsAdapter,
        Gladia => GladiaAdapter,
        GoogleGenerativeAi => GoogleGenerativeAiAdapter,
        Mistral => MistralAdapter,
        OpenAI => OpenAIAdapter,
        SmallestAI => SmallestAIAdapter,
        Soniox => SonioxAdapter,
        Xai => XaiAdapter,
    }
}

async fn start<A: RealtimeSttAdapter>(
    request: Request,
    listener: Arc<dyn TranscriptionEventListener>,
) -> Result<Arc<ProviderLiveTranscription>, ProviderTranscriptionError> {
    let (sender, receiver) = mpsc::channel(QUEUED_AUDIO_CHUNKS);
    let client = ListenClient::builder()
        .adapter::<A>()
        .api_base(request.base_url)
        .api_key(request.api_key)
        .params(request.params)
        .build_single()
        .await
        .map_err(|_| ProviderTranscriptionError::RequestFailed)?;
    let (stream, handle) = client
        .from_realtime_audio(ReceiverStream::new(receiver))
        .await
        .map_err(|_| ProviderTranscriptionError::RequestFailed)?;
    let finish = CancellationToken::new();
    let cancel = CancellationToken::new();
    let (completed, result) = watch::channel(None);
    let session = Arc::new(ProviderLiveTranscription {
        audio: Mutex::new(Some(sender)),
        finish: finish.clone(),
        cancel: cancel.clone(),
        completed: result,
    });
    tokio::spawn(async move {
        let success = consume(stream, &handle, &*listener, &finish, &cancel).await;
        if !success && !cancel.is_cancelled() {
            listener.on_error();
        }
        let _ = completed.send(Some(success));
    });
    Ok(session)
}

async fn consume<S, E>(
    stream: S,
    handle: &impl FinalizeHandle,
    listener: &dyn TranscriptionEventListener,
    finish: &CancellationToken,
    cancel: &CancellationToken,
) -> bool
where
    S: Stream<Item = Result<StreamResponse, E>>,
{
    let stream = stream.peekable();
    tokio::pin!(stream);
    // Initialize ws-client's stream drop guard even if JS cancels before our
    // first poll. Peek retains any immediately available response.
    let _ = stream.as_mut().peek().now_or_never();
    let mut finalizing = false;
    let deadline = tokio::time::sleep(FINALIZE_TIMEOUT);
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => return false,
            _ = finish.cancelled(), if !finalizing => {
                finalizing = true;
                deadline.as_mut().reset(tokio::time::Instant::now() + FINALIZE_TIMEOUT);
                let interrupted = tokio::select! {
                    biased;
                    _ = cancel.cancelled() => true,
                    _ = &mut deadline => true,
                    _ = handle.finalize() => false,
                };
                if interrupted { return false; }
            }
            _ = &mut deadline, if finalizing => return false,
            message = stream.next() => {
                let response = match message {
                    Some(Ok(response)) => response,
                    Some(Err(_)) => return false,
                    None => return finalizing || finish.is_cancelled(),
                };
                if matches!(response, StreamResponse::ErrorResponse { .. }) { return false; }
                let terminal = matches!(response,
                    StreamResponse::TerminalResponse { .. } |
                    StreamResponse::TranscriptResponse { from_finalize: true, .. }
                );
                let Ok(json) = serde_json::to_string(&response) else { return false; };
                if json.len() > 1_000_000 { return false; }
                listener.on_message(json);
                if terminal && finalizing { return true; }
            }
        }
    }
}

#[uniffi::export(async_runtime = "tokio")]
impl ProviderLiveTranscription {
    pub fn send_audio(&self, audio: Vec<u8>) -> Result<(), ProviderTranscriptionError> {
        let sender = self.audio.lock().unwrap();
        let sender = sender
            .as_ref()
            .ok_or(ProviderTranscriptionError::RequestFailed)?;
        if self.cancel.is_cancelled()
            || self.completed.borrow().is_some()
            || !audio.len().is_multiple_of(2)
        {
            return Err(ProviderTranscriptionError::RequestFailed);
        }
        for chunk in audio.chunks(AUDIO_CHUNK_BYTES) {
            if sender
                .try_send(MixedMessage::Audio(bytes::Bytes::copy_from_slice(chunk)))
                .is_err()
            {
                self.cancel.cancel();
                return Err(ProviderTranscriptionError::RequestFailed);
            }
        }
        Ok(())
    }

    pub async fn finish(&self) -> bool {
        let audio = self.audio.lock().unwrap().take();
        self.finish.cancel();
        drop(audio);
        let mut completed = self.completed.clone();
        loop {
            if let Some(success) = *completed.borrow_and_update() {
                return success;
            }
            if completed.changed().await.is_err() {
                return false;
            }
        }
    }

    pub fn cancel(&self) {
        self.audio.lock().unwrap().take();
        self.cancel.cancel();
    }
}

impl Drop for ProviderLiveTranscription {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::SinkExt;
    use serde_json::json;
    use tokio_tungstenite::{
        accept_hdr_async,
        tungstenite::{
            Message,
            handshake::server::{Request as WsRequest, Response as WsResponse},
        },
    };

    #[derive(Default)]
    struct Events {
        messages: Mutex<Vec<serde_json::Value>>,
        errors: Mutex<usize>,
    }
    impl TranscriptionEventListener for Events {
        fn on_message(&self, json: String) {
            self.messages
                .lock()
                .unwrap()
                .push(serde_json::from_str(&json).unwrap());
        }
        fn on_error(&self) {
            *self.errors.lock().unwrap() += 1;
        }
    }

    fn request(base_url: String) -> Request {
        Request {
            provider: "assemblyai".into(),
            base_url,
            api_key: "synthetic-key".into(),
            params: ListenParams {
                model: Some("universal-3-5-pro-realtime".into()),
                ..Default::default()
            },
        }
    }

    #[tokio::test]
    async fn finish_is_announced_before_audio_eof_wakes_the_provider() {
        use std::{
            sync::atomic::{AtomicBool, Ordering},
            task::{Context, Wake, Waker},
        };

        struct EofWake {
            finish: CancellationToken,
            finish_at_eof: AtomicBool,
        }
        impl Wake for EofWake {
            fn wake(self: Arc<Self>) {
                self.finish_at_eof
                    .store(self.finish.is_cancelled(), Ordering::SeqCst);
            }
        }
        let (sender, mut receiver) = mpsc::channel(1);
        let (_completed, result) = watch::channel(Some(true));
        let finish = CancellationToken::new();
        let observer = Arc::new(EofWake {
            finish: finish.clone(),
            finish_at_eof: AtomicBool::new(false),
        });
        let waker = Waker::from(observer.clone());
        assert!(
            receiver
                .poll_recv(&mut Context::from_waker(&waker))
                .is_pending()
        );
        let live = ProviderLiveTranscription {
            audio: Mutex::new(Some(sender)),
            finish,
            cancel: CancellationToken::new(),
            completed: result,
        };

        assert!(live.finish().await);
        assert!(observer.finish_at_eof.load(Ordering::SeqCst));
        assert!(receiver.recv().await.is_none());
    }

    #[tokio::test]
    async fn finish_between_select_polls_accepts_eof_but_unexpected_eof_fails() {
        struct Handle;
        impl FinalizeHandle for Handle {
            async fn finalize(&self) {
                panic!("EOF should arrive before the next finalization poll");
            }
            fn expected_finalize_count(&self) -> usize {
                1
            }
        }
        for finishing in [false, true] {
            let finish = CancellationToken::new();
            let mut polls = 0;
            let stream = futures_util::stream::poll_fn(|_| {
                polls += 1;
                if polls == 1 {
                    return std::task::Poll::Pending;
                }
                if finishing {
                    finish.cancel();
                }
                std::task::Poll::Ready(None::<Result<StreamResponse, ()>>)
            });
            assert_eq!(
                consume(
                    stream,
                    &Handle,
                    &Events::default(),
                    &finish,
                    &CancellationToken::new()
                )
                .await,
                finishing,
            );
        }
    }

    #[tokio::test]
    async fn metadata_during_recording_keeps_the_stream_open_until_finish() {
        struct Handle;
        impl FinalizeHandle for Handle {
            async fn finalize(&self) {}
            fn expected_finalize_count(&self) -> usize {
                1
            }
        }
        let (sender, receiver) = mpsc::channel::<Result<StreamResponse, ()>>(4);
        let metadata = || StreamResponse::TerminalResponse {
            request_id: "test-request".into(),
            created: String::new(),
            duration: 0.0,
            channels: 1,
        };
        let events = Events::default();
        let finish = CancellationToken::new();
        let cancel = CancellationToken::new();
        let consuming = consume(
            ReceiverStream::new(receiver),
            &Handle,
            &events,
            &finish,
            &cancel,
        );
        tokio::pin!(consuming);

        sender.send(Ok(metadata())).await.unwrap();
        assert!(consuming.as_mut().now_or_never().is_none());
        assert_eq!(events.messages.lock().unwrap().len(), 1);

        sender
            .send(Ok(StreamResponse::SpeechStartedResponse {
                channel: vec![0],
                timestamp: 1.0,
            }))
            .await
            .unwrap();
        assert!(consuming.as_mut().now_or_never().is_none());
        assert_eq!(events.messages.lock().unwrap()[1]["type"], "SpeechStarted");

        finish.cancel();
        sender.send(Ok(metadata())).await.unwrap();
        assert!(consuming.await);
    }

    #[tokio::test]
    async fn stalled_finalize_obeys_the_stop_deadline_and_cancellation() {
        struct StalledHandle;
        impl FinalizeHandle for StalledHandle {
            async fn finalize(&self) {
                std::future::pending::<()>().await;
            }
            fn expected_finalize_count(&self) -> usize {
                1
            }
        }
        let finish = CancellationToken::new();
        finish.cancel();
        for cancel_during_finalize in [false, true] {
            let events = Events::default();
            let cancel = CancellationToken::new();
            let consuming = consume(
                futures_util::stream::pending::<Result<StreamResponse, ()>>(),
                &StalledHandle,
                &events,
                &finish,
                &cancel,
            );
            tokio::pin!(consuming);
            assert!(consuming.as_mut().now_or_never().is_none());
            let timeout = if cancel_during_finalize {
                cancel.cancel();
                Duration::from_millis(100)
            } else {
                FINALIZE_TIMEOUT + Duration::from_secs(1)
            };
            assert!(!tokio::time::timeout(timeout, consuming).await.unwrap());
        }
    }

    #[tokio::test]
    #[allow(clippy::result_large_err)] // tungstenite fixes the handshake callback error type.
    async fn assemblyai_streams_pcm_and_delivers_last_turn_before_finish() {
        let server = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", server.local_addr().unwrap());
        let worker = tokio::spawn(async move {
            let (socket, _) = server.accept().await.unwrap();
            let mut socket = accept_hdr_async(socket, |req: &WsRequest, response: WsResponse| {
                assert_eq!(req.headers()["authorization"], "synthetic-key");
                assert!(
                    req.uri()
                        .to_string()
                        .contains("speech_model=universal-3-5-pro")
                );
                assert!(req.uri().to_string().contains("sample_rate=16000"));
                Ok(response)
            })
            .await
            .unwrap();
            let mut audio = Vec::new();
            while let Some(Ok(message)) = socket.next().await {
                match message {
                    Message::Binary(bytes) => {
                        audio.extend_from_slice(&bytes);
                        socket
                            .send(Message::Text(
                                json!({"type":"Turn", "transcript":"Hello", "end_of_turn":false})
                                    .to_string()
                                    .into(),
                            ))
                            .await
                            .unwrap();
                    }
                    Message::Text(text) if text.contains("Terminate") => {
                        assert_eq!(audio, vec![7; 9_600]);
                        socket.send(Message::Text(json!({"type":"Turn", "transcript":"Hello world", "end_of_turn":true, "speaker_label":"B", "words":[
                            {"text":"Hello", "start":0, "end":300, "word_is_final":true},
                            {"text":"world", "start":300, "end":600, "word_is_final":true}
                        ]}).to_string().into())).await.unwrap();
                        socket.send(Message::Text(json!({"type":"Termination", "audio_duration_seconds":1, "session_duration_seconds":1}).to_string().into())).await.unwrap();
                        return;
                    }
                    _ => {}
                }
            }
            panic!("client did not finalize");
        });
        let events = Arc::new(Events::default());
        let live = start::<AssemblyAIAdapter>(request(base), events.clone())
            .await
            .unwrap();
        live.send_audio(vec![7; 9_600]).unwrap();
        assert!(
            tokio::time::timeout(Duration::from_secs(7), live.finish())
                .await
                .unwrap()
        );
        assert!(live.finish().await);
        worker.await.unwrap();
        let events = events.messages.lock().unwrap();
        assert!(events.iter().any(|event| event["is_final"] == false));
        let final_turn = events
            .iter()
            .find(|event| event["is_final"] == true)
            .unwrap();
        assert_eq!(
            final_turn["channel"]["alternatives"][0]["transcript"],
            "Hello world"
        );
        assert_eq!(
            final_turn["channel"]["alternatives"][0]["words"][0]["speaker"],
            1
        );
    }

    #[tokio::test]
    async fn cancelling_immediately_closes_the_native_socket() {
        let server = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", server.local_addr().unwrap());
        let worker = tokio::spawn(async move {
            let (socket, _) = server.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(socket).await.unwrap();
            assert!(matches!(
                socket.next().await,
                None | Some(Ok(Message::Close(_)))
            ));
        });
        let events = Arc::new(Events::default());
        let live = start::<AssemblyAIAdapter>(request(base), events.clone())
            .await
            .unwrap();
        live.cancel();
        assert!(!live.finish().await);
        tokio::time::timeout(Duration::from_secs(2), worker)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(*events.errors.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn invalid_credentials_formats_and_batch_only_providers_never_connect() {
        for (provider, base, key, sample_rate, channels) in [
            ("assemblyai", "http://localhost", "key", 16_000, 1),
            ("assemblyai", "https://provider.example", "", 16_000, 1),
            ("assemblyai", "https://provider.example", "key", 48_000, 1),
            ("assemblyai", "https://provider.example", "key", 16_000, 2),
            ("groq", "https://provider.example", "key", 16_000, 1),
        ] {
            let events = Arc::new(Events::default());
            let result = start_provider_live_transcription(
                json!({"provider":provider, "base_url":base, "api_key":key,
                "params":{"model":"model", "sample_rate":sample_rate, "channels":channels}})
                .to_string(),
                events,
            )
            .await;
            assert!(matches!(
                result,
                Err(ProviderTranscriptionError::InvalidSettings)
            ));
        }
    }

    #[test]
    fn audio_backpressure_is_bounded_and_cancels_instead_of_dropping_words() {
        let (sender, _receiver) = mpsc::channel(QUEUED_AUDIO_CHUNKS);
        let (_completed, result) = watch::channel(None);
        let live = ProviderLiveTranscription {
            audio: Mutex::new(Some(sender)),
            finish: CancellationToken::new(),
            cancel: CancellationToken::new(),
            completed: result,
        };
        live.send_audio(vec![0; 320_000]).unwrap();
        assert!(live.send_audio(vec![0; 2]).is_err());
        assert!(live.cancel.is_cancelled());
    }
}
