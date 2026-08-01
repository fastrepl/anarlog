use std::marker::PhantomData;
use std::pin::Pin;
use std::time::Duration;

use futures_util::{Stream, StreamExt};

use anlg_ws_client::client::{
    ClientRequestBuilder, Message, Utf8Bytes, WebSocketClient, WebSocketHandle, WebSocketIO,
};
use owhisper_interface::ListenParams;
use owhisper_interface::stream::StreamResponse;
use owhisper_interface::{ControlMessage, MixedMessage};

use crate::{
    DeepgramAdapter, RealtimeSttAdapter, append_provider_param, is_anarlog_proxy,
    normalize_listen_params,
};

pub struct ListenClientBuilder<A: RealtimeSttAdapter = DeepgramAdapter> {
    pub(crate) api_base: Option<String>,
    pub(crate) api_key: Option<String>,
    pub(crate) params: Option<ListenParams>,
    pub(crate) extra_headers: Vec<(String, String)>,
    pub(crate) connect_policy: Option<anlg_ws_client::client::WebSocketConnectPolicy>,
    pub(crate) _marker: PhantomData<A>,
}

impl Default for ListenClientBuilder {
    fn default() -> Self {
        Self {
            api_base: None,
            api_key: None,
            params: None,
            extra_headers: Vec::new(),
            connect_policy: None,
            _marker: PhantomData,
        }
    }
}

impl<A: RealtimeSttAdapter> ListenClientBuilder<A> {
    pub fn api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = Some(api_base.into());
        self
    }

    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn params(mut self, params: ListenParams) -> Self {
        self.params = Some(params);
        self
    }

    pub fn extra_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_headers.push((name.into(), value.into()));
        self
    }

    pub fn connect_policy(
        mut self,
        policy: anlg_ws_client::client::WebSocketConnectPolicy,
    ) -> Self {
        self.connect_policy = Some(policy);
        self
    }

    pub fn adapter<B: RealtimeSttAdapter>(self) -> ListenClientBuilder<B> {
        ListenClientBuilder {
            api_base: self.api_base,
            api_key: self.api_key,
            params: self.params,
            extra_headers: self.extra_headers,
            connect_policy: self.connect_policy,
            _marker: PhantomData,
        }
    }

    fn get_api_base(&self) -> &str {
        self.api_base.as_ref().expect("api_base is required")
    }

    pub(crate) fn normalized_params(&self) -> ListenParams {
        normalize_listen_params(self.params.clone().unwrap_or_default())
    }

    async fn build_request(
        &self,
        adapter: &A,
        params: &ListenParams,
        channels: u8,
    ) -> Result<anlg_ws_client::client::ClientRequestBuilder, crate::Error> {
        let original_api_base = self.get_api_base();
        let api_base = append_provider_param(original_api_base, adapter.provider_name());
        let url = adapter
            .build_ws_url_with_api_key(&api_base, params, channels, self.api_key.as_deref())
            .await
            .unwrap_or_else(|| adapter.build_ws_url(&api_base, params, channels));
        let uri = url.to_string().parse().map_err(|error| {
            crate::Error::provider_configuration(
                adapter.provider_name(),
                format!("realtime endpoint cannot be used as a WebSocket URI: {error}"),
            )
        })?;

        let mut request = anlg_ws_client::client::ClientRequestBuilder::new(uri);

        if is_anarlog_proxy(original_api_base) {
            if let Some(api_key) = self.api_key.as_deref() {
                request = request.with_header("Authorization", format!("Bearer {}", api_key));
            }
            for (name, value) in &self.extra_headers {
                request = request.with_header(name, value);
            }
        } else if let Some((header_name, header_value)) =
            adapter.build_auth_header(self.api_key.as_deref())
        {
            request = request.with_header(header_name, header_value);
        }

        Ok(request)
    }

    pub async fn build_with_channels(self, channels: u8) -> Result<ListenClient<A>, crate::Error> {
        let adapter = A::default();
        let params = self.normalized_params();
        let request = self.build_request(&adapter, &params, channels).await?;
        let initial_message = adapter.initial_message(self.api_key.as_deref(), &params, channels);

        Ok(ListenClient {
            adapter,
            request,
            initial_message,
            connect_policy: self.connect_policy,
        })
    }

    pub async fn build_single(self) -> Result<ListenClient<A>, crate::Error> {
        self.build_with_channels(1).await
    }

    pub async fn build_dual(self) -> Result<ListenClientDual<A>, crate::Error> {
        let adapter = A::default();
        let channels = if adapter.supports_native_multichannel() {
            2
        } else {
            1
        };
        let params = self.normalized_params();
        let request = self.build_request(&adapter, &params, channels).await?;
        let initial_message = adapter.initial_message(self.api_key.as_deref(), &params, channels);

        Ok(ListenClientDual {
            adapter,
            request,
            initial_message,
            connect_policy: self.connect_policy,
        })
    }
}

pub type ListenClientInput = MixedMessage<bytes::Bytes, ControlMessage>;
pub type ListenClientDualInput = MixedMessage<(bytes::Bytes, bytes::Bytes), ControlMessage>;

#[derive(Clone)]
pub struct ListenClient<A: RealtimeSttAdapter = DeepgramAdapter> {
    pub(crate) adapter: A,
    pub(crate) request: ClientRequestBuilder,
    pub(crate) initial_message: Option<Message>,
    pub(crate) connect_policy: Option<anlg_ws_client::client::WebSocketConnectPolicy>,
}

#[derive(Clone)]
pub struct ListenClientDual<A: RealtimeSttAdapter> {
    pub(crate) adapter: A,
    pub(crate) request: ClientRequestBuilder,
    pub(crate) initial_message: Option<Message>,
    pub(crate) connect_policy: Option<anlg_ws_client::client::WebSocketConnectPolicy>,
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

    fn from_message(msg: Message) -> Result<Option<Self::Output>, anlg_ws_client::Error> {
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

    fn from_message(msg: Message) -> Result<Option<Self::Output>, anlg_ws_client::Error> {
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
            impl Stream<Item = Result<StreamResponse, anlg_ws_client::Error>>,
            SingleHandle,
        ),
        anlg_ws_client::Error,
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
            let responses: Vec<Result<StreamResponse, anlg_ws_client::Error>> = match result {
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
    Pin<Box<dyn Stream<Item = Result<StreamResponse, anlg_ws_client::Error>> + Send>>;

impl<A: RealtimeSttAdapter> ListenClientDual<A> {
    #[allow(clippy::wrong_self_convention)]
    pub async fn from_realtime_audio(
        self,
        stream: impl Stream<Item = ListenClientDualInput> + Send + Unpin + 'static,
    ) -> Result<(DualOutputStream, DualHandle), anlg_ws_client::Error> {
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
    ) -> Result<(DualOutputStream, DualHandle), anlg_ws_client::Error> {
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
            let responses: Vec<Result<StreamResponse, anlg_ws_client::Error>> = match result {
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
    ) -> Result<(DualOutputStream, DualHandle), anlg_ws_client::Error> {
        let finalize_text = extract_finalize_text(&self.adapter);
        let mic_adapter = self.adapter.fork_session();
        let spk_adapter = self.adapter.fork_session();
        let (mic_tx, mic_rx) = tokio::sync::mpsc::channel::<TransformedInput>(32);
        let (spk_tx, spk_rx) = tokio::sync::mpsc::channel::<TransformedInput>(32);

        let mic_ws = websocket_client_with_keep_alive(
            &self.request,
            &mic_adapter,
            self.connect_policy.clone(),
        );
        let spk_ws =
            websocket_client_with_keep_alive(&self.request, &spk_adapter, self.connect_policy);

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
            mic_adapter.clone(),
            spk_adapter.clone(),
        ));

        let mic_stream = mic_raw.flat_map({
            let adapter = mic_adapter;
            move |result| {
                let adapter = adapter.clone();
                let responses: Vec<Result<StreamResponse, anlg_ws_client::Error>> = match result {
                    Ok(raw) => adapter.parse_response(&raw).into_iter().map(Ok).collect(),
                    Err(e) => vec![Err(e)],
                };
                futures_util::stream::iter(responses)
            }
        });

        let spk_stream = spk_raw.flat_map({
            let adapter = spk_adapter;
            move |result| {
                let adapter = adapter.clone();
                let responses: Vec<Result<StreamResponse, anlg_ws_client::Error>> = match result {
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
    mic_adapter: A,
    spk_adapter: A,
) {
    while let Some(msg) = stream.next().await {
        match msg {
            MixedMessage::Audio((mic, spk)) => {
                let mic_msg = mic_adapter.audio_to_message(mic);
                let spk_msg = spk_adapter.audio_to_message(spk);
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
) -> impl Stream<Item = Result<StreamResponse, anlg_ws_client::Error>> + Send
where
    S1: Stream<Item = Result<StreamResponse, anlg_ws_client::Error>> + Send + 'static,
    S2: Stream<Item = Result<StreamResponse, anlg_ws_client::Error>> + Send + 'static,
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
    connect_policy: Option<anlg_ws_client::client::WebSocketConnectPolicy>,
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
mod tests;
