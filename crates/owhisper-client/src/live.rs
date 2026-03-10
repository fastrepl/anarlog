use std::pin::Pin;
use std::time::Duration;

use futures_util::{Stream, StreamExt};

use hypr_ws_client::client::{
    ClientRequestBuilder, Message, Utf8Bytes, WebSocketClient, WebSocketHandle, WebSocketIO,
};
use owhisper_interface::stream::StreamResponse;
use owhisper_interface::{ControlMessage, MixedMessage};

use crate::{DeepgramAdapter, ListenClientBuilder, RealtimeSttAdapter};

pub type ListenClientInput = MixedMessage<bytes::Bytes, ControlMessage>;
pub type ListenClientDualInput = MixedMessage<(bytes::Bytes, bytes::Bytes), ControlMessage>;

#[derive(Clone)]
pub struct ListenClient<A: RealtimeSttAdapter = DeepgramAdapter> {
    pub(crate) adapter: A,
    pub(crate) request: ClientRequestBuilder,
    pub(crate) initial_message: Option<Message>,
    pub(crate) connect_policy: Option<hypr_ws_client::client::WebSocketConnectPolicy>,
}

#[derive(Clone)]
pub struct ListenClientDual<A: RealtimeSttAdapter> {
    pub(crate) adapter: A,
    pub(crate) request: ClientRequestBuilder,
    pub(crate) initial_message: Option<Message>,
    pub(crate) connect_policy: Option<hypr_ws_client::client::WebSocketConnectPolicy>,
}

pub struct SingleHandle {
    inner: WebSocketHandle,
    finalize_text: Utf8Bytes,
}

pub enum DualHandle {
    Native {
        inner: WebSocketHandle,
        finalize_text: Utf8Bytes,
    },
    Split {
        mic: WebSocketHandle,
        spk: WebSocketHandle,
        finalize_text: Utf8Bytes,
    },
}

pub trait FinalizeHandle: Send {
    fn finalize(&self) -> impl std::future::Future<Output = ()> + Send;
    fn expected_finalize_count(&self) -> usize;
}

impl FinalizeHandle for SingleHandle {
    async fn finalize(&self) {
        self.inner
            .finalize_with_text(self.finalize_text.clone())
            .await
    }

    fn expected_finalize_count(&self) -> usize {
        1
    }
}

impl FinalizeHandle for DualHandle {
    async fn finalize(&self) {
        match self {
            DualHandle::Native {
                inner,
                finalize_text,
            } => inner.finalize_with_text(finalize_text.clone()).await,
            DualHandle::Split {
                mic,
                spk,
                finalize_text,
            } => {
                tokio::join!(
                    mic.finalize_with_text(finalize_text.clone()),
                    spk.finalize_with_text(finalize_text.clone())
                );
            }
        }
    }

    fn expected_finalize_count(&self) -> usize {
        match self {
            DualHandle::Native { .. } => 1,
            DualHandle::Split { .. } => 2,
        }
    }
}

fn interleave_audio(mic: &[u8], speaker: &[u8]) -> Vec<u8> {
    let mic_samples: Vec<i16> = mic
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();
    let speaker_samples: Vec<i16> = speaker
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();

    let max_len = mic_samples.len().max(speaker_samples.len());
    let mut interleaved = Vec::with_capacity(max_len * 2 * 2);

    for i in 0..max_len {
        let mic_sample = mic_samples.get(i).copied().unwrap_or(0);
        let speaker_sample = speaker_samples.get(i).copied().unwrap_or(0);
        interleaved.extend_from_slice(&mic_sample.to_le_bytes());
        interleaved.extend_from_slice(&speaker_sample.to_le_bytes());
    }

    interleaved
}

pub type TransformedInput = MixedMessage<Message, ControlMessage>;

pub struct ListenClientIO;

impl WebSocketIO for ListenClientIO {
    type Data = TransformedInput;
    type Input = TransformedInput;
    type Output = String;

    fn to_input(data: Self::Data) -> Self::Input {
        data
    }

    fn to_message(input: Self::Input) -> Message {
        match input {
            MixedMessage::Audio(msg) => msg,
            MixedMessage::Control(control) => {
                Message::Text(serde_json::to_string(&control).unwrap().into())
            }
        }
    }

    fn from_message(msg: Message) -> Result<Option<Self::Output>, hypr_ws_client::Error> {
        Ok(match msg {
            Message::Text(text) => Some(text.to_string()),
            _ => None,
        })
    }
}

pub type TransformedDualInput = MixedMessage<(bytes::Bytes, bytes::Bytes, Message), ControlMessage>;

pub struct ListenClientDualIO;

impl WebSocketIO for ListenClientDualIO {
    type Data = TransformedDualInput;
    type Input = TransformedInput;
    type Output = String;

    fn to_input(data: Self::Data) -> Self::Input {
        match data {
            TransformedDualInput::Audio((_, _, transform_fn_result)) => {
                TransformedInput::Audio(transform_fn_result)
            }
            TransformedDualInput::Control(control) => TransformedInput::Control(control),
        }
    }

    fn to_message(input: Self::Input) -> Message {
        match input {
            TransformedInput::Audio(msg) => msg,
            TransformedInput::Control(control) => {
                Message::Text(serde_json::to_string(&control).unwrap().into())
            }
        }
    }

    fn from_message(msg: Message) -> Result<Option<Self::Output>, hypr_ws_client::Error> {
        Ok(match msg {
            Message::Text(text) => Some(text.to_string()),
            _ => None,
        })
    }
}

impl ListenClient<DeepgramAdapter> {
    pub fn builder() -> ListenClientBuilder<DeepgramAdapter> {
        ListenClientBuilder::default()
    }
}

impl<A: RealtimeSttAdapter> ListenClient<A> {
    #[allow(clippy::wrong_self_convention)]
    pub async fn from_realtime_audio(
        self,
        audio_stream: impl Stream<Item = ListenClientInput> + Send + Unpin + 'static,
    ) -> Result<
        (
            impl Stream<Item = Result<StreamResponse, hypr_ws_client::Error>>,
            SingleHandle,
        ),
        hypr_ws_client::Error,
    > {
        let finalize_text = extract_finalize_text(&self.adapter);
        let ws =
            websocket_client_with_keep_alive(&self.request, &self.adapter, self.connect_policy);

        // Transform audio stream to use adapter's audio_to_message method
        let adapter_for_transform = self.adapter.clone();
        let transformed_stream = audio_stream.map(move |input| match input {
            MixedMessage::Audio(data) => {
                TransformedInput::Audio(adapter_for_transform.audio_to_message(data))
            }
            MixedMessage::Control(control) => TransformedInput::Control(control),
        });

        let (raw_stream, inner) = ws
            .from_audio::<ListenClientIO, _>(self.initial_message, Box::pin(transformed_stream))
            .await?;

        let adapter = self.adapter;
        let mapped_stream = raw_stream.flat_map(move |result| {
            let adapter = adapter.clone();
            let responses: Vec<Result<StreamResponse, hypr_ws_client::Error>> = match result {
                Ok(raw) => adapter.parse_response(&raw).into_iter().map(Ok).collect(),
                Err(e) => vec![Err(e)],
            };
            futures_util::stream::iter(responses)
        });

        let handle = SingleHandle {
            inner,
            finalize_text,
        };
        Ok((mapped_stream, handle))
    }
}

type DualOutputStream =
    Pin<Box<dyn Stream<Item = Result<StreamResponse, hypr_ws_client::Error>> + Send>>;

impl<A: RealtimeSttAdapter> ListenClientDual<A> {
    #[allow(clippy::wrong_self_convention)]
    pub async fn from_realtime_audio(
        self,
        stream: impl Stream<Item = ListenClientDualInput> + Send + Unpin + 'static,
    ) -> Result<(DualOutputStream, DualHandle), hypr_ws_client::Error> {
        if self.adapter.supports_native_multichannel() {
            self.from_realtime_audio_native(stream).await
        } else {
            self.from_realtime_audio_split(stream).await
        }
    }

    #[allow(clippy::wrong_self_convention)]
    async fn from_realtime_audio_native(
        self,
        stream: impl Stream<Item = ListenClientDualInput> + Send + Unpin + 'static,
    ) -> Result<(DualOutputStream, DualHandle), hypr_ws_client::Error> {
        let finalize_text = extract_finalize_text(&self.adapter);
        let ws =
            websocket_client_with_keep_alive(&self.request, &self.adapter, self.connect_policy);

        // Transform audio stream to use adapter's audio_to_message method
        let adapter_for_transform = self.adapter.clone();
        let transformed_stream = stream.map(move |input| match input {
            MixedMessage::Audio((mic, speaker)) => {
                let interleaved = interleave_audio(&mic, &speaker);
                let msg = adapter_for_transform.audio_to_message(interleaved.into());
                TransformedDualInput::Audio((mic, speaker, msg))
            }
            MixedMessage::Control(control) => TransformedDualInput::Control(control),
        });

        let (raw_stream, inner) = ws
            .from_audio::<ListenClientDualIO, _>(self.initial_message, Box::pin(transformed_stream))
            .await?;

        let adapter = self.adapter;
        let mapped_stream = raw_stream.flat_map(move |result| {
            let adapter = adapter.clone();
            let responses: Vec<Result<StreamResponse, hypr_ws_client::Error>> = match result {
                Ok(raw) => adapter.parse_response(&raw).into_iter().map(Ok).collect(),
                Err(e) => vec![Err(e)],
            };
            futures_util::stream::iter(responses)
        });

        let handle = DualHandle::Native {
            inner,
            finalize_text,
        };
        Ok((Box::pin(mapped_stream), handle))
    }

    #[allow(clippy::wrong_self_convention)]
    async fn from_realtime_audio_split(
        self,
        stream: impl Stream<Item = ListenClientDualInput> + Send + Unpin + 'static,
    ) -> Result<(DualOutputStream, DualHandle), hypr_ws_client::Error> {
        let finalize_text = extract_finalize_text(&self.adapter);
        let (mic_tx, mic_rx) = tokio::sync::mpsc::channel::<TransformedInput>(32);
        let (spk_tx, spk_rx) = tokio::sync::mpsc::channel::<TransformedInput>(32);

        let mic_ws = websocket_client_with_keep_alive(
            &self.request,
            &self.adapter,
            self.connect_policy.clone(),
        );
        let spk_ws =
            websocket_client_with_keep_alive(&self.request, &self.adapter, self.connect_policy);

        let mic_outbound = tokio_stream::wrappers::ReceiverStream::new(mic_rx);
        let spk_outbound = tokio_stream::wrappers::ReceiverStream::new(spk_rx);

        let mic_connect =
            mic_ws.from_audio::<ListenClientIO, _>(self.initial_message.clone(), mic_outbound);
        let spk_connect =
            spk_ws.from_audio::<ListenClientIO, _>(self.initial_message, spk_outbound);

        let ((mic_raw, mic_handle), (spk_raw, spk_handle)) =
            tokio::try_join!(mic_connect, spk_connect)?;

        tokio::spawn(forward_dual_to_single(
            stream,
            mic_tx,
            spk_tx,
            self.adapter.clone(),
        ));

        let adapter = self.adapter.clone();
        let mic_stream = mic_raw.flat_map({
            let adapter = adapter.clone();
            move |result| {
                let adapter = adapter.clone();
                let responses: Vec<Result<StreamResponse, hypr_ws_client::Error>> = match result {
                    Ok(raw) => adapter.parse_response(&raw).into_iter().map(Ok).collect(),
                    Err(e) => vec![Err(e)],
                };
                futures_util::stream::iter(responses)
            }
        });

        let spk_stream = spk_raw.flat_map({
            let adapter = adapter.clone();
            move |result| {
                let adapter = adapter.clone();
                let responses: Vec<Result<StreamResponse, hypr_ws_client::Error>> = match result {
                    Ok(raw) => adapter.parse_response(&raw).into_iter().map(Ok).collect(),
                    Err(e) => vec![Err(e)],
                };
                futures_util::stream::iter(responses)
            }
        });

        let merged_stream = merge_streams_with_channel_remap(mic_stream, spk_stream);

        Ok((
            Box::pin(merged_stream),
            DualHandle::Split {
                mic: mic_handle,
                spk: spk_handle,
                finalize_text,
            },
        ))
    }
}

async fn forward_dual_to_single<A: RealtimeSttAdapter>(
    mut stream: impl Stream<Item = ListenClientDualInput> + Send + Unpin + 'static,
    mic_tx: tokio::sync::mpsc::Sender<TransformedInput>,
    spk_tx: tokio::sync::mpsc::Sender<TransformedInput>,
    adapter: A,
) {
    while let Some(msg) = stream.next().await {
        match msg {
            MixedMessage::Audio((mic, spk)) => {
                let mic_msg = adapter.audio_to_message(mic);
                let spk_msg = adapter.audio_to_message(spk);
                if mic_tx.send(MixedMessage::Audio(mic_msg)).await.is_err() {
                    break;
                }
                if spk_tx.send(MixedMessage::Audio(spk_msg)).await.is_err() {
                    break;
                }
            }
            MixedMessage::Control(ctrl) => {
                if mic_tx
                    .send(MixedMessage::Control(ctrl.clone()))
                    .await
                    .is_err()
                {
                    break;
                }
                if spk_tx.send(MixedMessage::Control(ctrl)).await.is_err() {
                    break;
                }
            }
        }
    }
}

fn merge_streams_with_channel_remap<S1, S2>(
    mic_stream: S1,
    spk_stream: S2,
) -> impl Stream<Item = Result<StreamResponse, hypr_ws_client::Error>> + Send
where
    S1: Stream<Item = Result<StreamResponse, hypr_ws_client::Error>> + Send + 'static,
    S2: Stream<Item = Result<StreamResponse, hypr_ws_client::Error>> + Send + 'static,
{
    let mic_mapped = mic_stream.map(|result| {
        result.map(|mut response| {
            response.set_channel_index(0, 2);
            response
        })
    });

    let spk_mapped = spk_stream.map(|result| {
        result.map(|mut response| {
            response.set_channel_index(1, 2);
            response
        })
    });

    futures_util::stream::select(mic_mapped, spk_mapped)
}

fn websocket_client_with_keep_alive<A: RealtimeSttAdapter>(
    request: &ClientRequestBuilder,
    adapter: &A,
    connect_policy: Option<hypr_ws_client::client::WebSocketConnectPolicy>,
) -> WebSocketClient {
    let mut client = WebSocketClient::new(request.clone());

    if let Some(connect_policy) = connect_policy {
        client = client.with_connect_policy(connect_policy);
    }

    if let Some(keep_alive) = adapter.keep_alive_message() {
        client = client.with_keep_alive_message(Duration::from_secs(5), keep_alive);
    }

    client
}

fn extract_finalize_text<A: RealtimeSttAdapter>(adapter: &A) -> Utf8Bytes {
    match adapter.finalize_message() {
        Message::Text(text) => text,
        _ => r#"{"type":"Finalize"}"#.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bytes::Bytes;
    use hypr_ws_client::client::Message;

    use super::{ListenClientDualInput, TransformedInput, forward_dual_to_single};
    use crate::test_utils::{run_dual_test, run_single_test};
    use crate::{
        AssemblyAIAdapter, DeepgramAdapter, ListenClient, RealtimeSttAdapter, SonioxAdapter,
    };

    #[derive(Clone, Default)]
    struct TestAdapter;

    impl RealtimeSttAdapter for TestAdapter {
        fn provider_name(&self) -> &'static str {
            "test"
        }

        fn is_supported_languages(
            &self,
            _languages: &[hypr_language::Language],
            _model: Option<&str>,
        ) -> bool {
            true
        }

        fn supports_native_multichannel(&self) -> bool {
            false
        }

        fn build_ws_url(
            &self,
            _api_base: &str,
            _params: &owhisper_interface::ListenParams,
            _channels: u8,
        ) -> url::Url {
            "ws://localhost".parse().expect("invalid test url")
        }

        fn build_auth_header(&self, _api_key: Option<&str>) -> Option<(&'static str, String)> {
            None
        }

        fn keep_alive_message(&self) -> Option<Message> {
            None
        }

        fn finalize_message(&self) -> Message {
            Message::Text("finalize".into())
        }

        fn parse_response(&self, _raw: &str) -> Vec<owhisper_interface::stream::StreamResponse> {
            Vec::new()
        }
    }

    fn proxy_base() -> String {
        std::env::var("PROXY_URL").unwrap_or_else(|_| "localhost:3001".to_string())
    }

    #[tokio::test]
    async fn forward_dual_to_single_forwards_all_audio_without_dropping() {
        let stream = futures_util::stream::iter(vec![
            ListenClientDualInput::Audio((
                Bytes::from_static(b"mic-1"),
                Bytes::from_static(b"spk-1"),
            )),
            ListenClientDualInput::Audio((
                Bytes::from_static(b"mic-2"),
                Bytes::from_static(b"spk-2"),
            )),
        ]);
        let (mic_tx, mut mic_rx) = tokio::sync::mpsc::channel(1);
        let (spk_tx, mut spk_rx) = tokio::sync::mpsc::channel(1);

        let task = tokio::spawn(forward_dual_to_single(stream, mic_tx, spk_tx, TestAdapter));

        let Some(TransformedInput::Audio(Message::Binary(first_mic))) = mic_rx.recv().await else {
            panic!("missing first mic frame");
        };
        let Some(TransformedInput::Audio(Message::Binary(first_spk))) = spk_rx.recv().await else {
            panic!("missing first speaker frame");
        };
        let Some(TransformedInput::Audio(Message::Binary(second_mic))) =
            tokio::time::timeout(Duration::from_secs(1), mic_rx.recv())
                .await
                .expect("timed out waiting for second mic frame")
        else {
            panic!("missing second mic frame");
        };
        let Some(TransformedInput::Audio(Message::Binary(second_spk))) =
            tokio::time::timeout(Duration::from_secs(1), spk_rx.recv())
                .await
                .expect("timed out waiting for second speaker frame")
        else {
            panic!("missing second speaker frame");
        };

        assert_eq!(first_mic.as_ref(), b"mic-1");
        assert_eq!(first_spk.as_ref(), b"spk-1");
        assert_eq!(second_mic.as_ref(), b"mic-2");
        assert_eq!(second_spk.as_ref(), b"spk-2");

        let _: () = task.await.expect("forward task panicked");
    }

    #[tokio::test]
    #[ignore]
    async fn test_proxy_deepgram_single() {
        let client = ListenClient::builder()
            .adapter::<DeepgramAdapter>()
            .api_base(&format!("http://{}", proxy_base()))
            .params(owhisper_interface::ListenParams {
                model: Some("nova-3".to_string()),
                languages: vec![hypr_language::ISO639::En.into()],
                ..Default::default()
            })
            .build_single()
            .await;

        run_single_test(client, "proxy-deepgram").await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_proxy_deepgram_dual() {
        let client = ListenClient::builder()
            .adapter::<DeepgramAdapter>()
            .api_base(&format!("http://{}", proxy_base()))
            .params(owhisper_interface::ListenParams {
                model: Some("nova-3".to_string()),
                languages: vec![hypr_language::ISO639::En.into()],
                ..Default::default()
            })
            .build_dual()
            .await;

        run_dual_test(client, "proxy-deepgram").await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_proxy_soniox_single() {
        let client = ListenClient::builder()
            .adapter::<SonioxAdapter>()
            .api_base(&format!("http://{}", proxy_base()))
            .params(owhisper_interface::ListenParams {
                model: Some("stt-v3".to_string()),
                languages: vec![hypr_language::ISO639::En.into()],
                ..Default::default()
            })
            .build_single()
            .await;

        run_single_test(client, "proxy-soniox").await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_proxy_soniox_dual() {
        let client = ListenClient::builder()
            .adapter::<SonioxAdapter>()
            .api_base(&format!("http://{}", proxy_base()))
            .params(owhisper_interface::ListenParams {
                model: Some("stt-v3".to_string()),
                languages: vec![hypr_language::ISO639::En.into()],
                ..Default::default()
            })
            .build_dual()
            .await;

        run_dual_test(client, "proxy-soniox").await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_proxy_assemblyai_single() {
        let client = ListenClient::builder()
            .adapter::<AssemblyAIAdapter>()
            .api_base(&format!("http://{}", proxy_base()))
            .params(owhisper_interface::ListenParams {
                model: Some("universal-streaming-english".to_string()),
                languages: vec![hypr_language::ISO639::En.into()],
                ..Default::default()
            })
            .build_single()
            .await;

        run_single_test(client, "proxy-assemblyai").await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_proxy_assemblyai_dual() {
        let client = ListenClient::builder()
            .adapter::<AssemblyAIAdapter>()
            .api_base(&format!("http://{}", proxy_base()))
            .params(owhisper_interface::ListenParams {
                model: Some("universal-streaming-english".to_string()),
                languages: vec![hypr_language::ISO639::En.into()],
                ..Default::default()
            })
            .build_dual()
            .await;

        run_dual_test(client, "proxy-assemblyai").await;
    }
}
