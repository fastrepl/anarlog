use std::marker::PhantomData;
use std::pin::Pin;
use std::time::Duration;

use futures_util::{Stream, StreamExt};

use hypr_ws_client::client::{ClientRequestBuilder, Message, WebSocketClient, WebSocketIO};
use owhisper_interface::ListenParams;
use owhisper_interface::stream::StreamResponse;
use owhisper_interface::{ControlMessage, MixedMessage};

use crate::adapter::{RealtimeAudioEncoder, RealtimeAudioInput};
use crate::{
    DeepgramAdapter, RealtimeSttAdapter, append_provider_param, is_hyprnote_proxy,
    normalize_listen_params,
};

pub struct ListenClientBuilder<A: RealtimeSttAdapter = DeepgramAdapter> {
    pub(crate) api_base: Option<String>,
    pub(crate) api_key: Option<String>,
    pub(crate) params: Option<ListenParams>,
    pub(crate) extra_headers: Vec<(String, String)>,
    pub(crate) connect_policy: Option<hypr_ws_client::client::WebSocketConnectPolicy>,
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
        policy: hypr_ws_client::client::WebSocketConnectPolicy,
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
    ) -> hypr_ws_client::client::ClientRequestBuilder {
        let original_api_base = self.get_api_base();
        let api_base = append_provider_param(original_api_base, adapter.provider_name());
        let url = adapter
            .build_ws_url_with_api_key(&api_base, params, channels, self.api_key.as_deref())
            .await
            .unwrap_or_else(|| adapter.build_ws_url(&api_base, params, channels));
        let uri = url.to_string().parse().unwrap();

        let mut request = hypr_ws_client::client::ClientRequestBuilder::new(uri);

        if is_hyprnote_proxy(original_api_base) {
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

        request
    }

    pub async fn build_with_channels(self, channels: u8) -> ListenClient<A> {
        let adapter = A::default();
        let normalized_params = self.normalized_params();
        let input_sample_rate = normalized_params.sample_rate;
        let mut source_params = adapter.normalize_realtime_params(normalized_params, channels);
        source_params.sample_rate = input_sample_rate;
        let provider_sample_rate = adapter.realtime_target_sample_rate(input_sample_rate, channels);
        let mut provider_params = source_params.clone();
        provider_params.sample_rate = provider_sample_rate;
        let request = self
            .build_request(&adapter, &provider_params, channels)
            .await;
        let initial_message =
            adapter.initial_message(self.api_key.as_deref(), &provider_params, channels);

        ListenClient {
            adapter,
            request,
            initial_message,
            connect_policy: self.connect_policy,
            input_sample_rate,
            provider_sample_rate,
            provider_params,
            channels,
        }
    }

    pub async fn build_single(self) -> ListenClient<A> {
        self.build_with_channels(1).await
    }

    pub async fn build_dual(self) -> ListenClientDual<A> {
        let adapter = A::default();
        let channels = if adapter.supports_native_multichannel() {
            2
        } else {
            1
        };
        let normalized_params = self.normalized_params();
        let input_sample_rate = normalized_params.sample_rate;
        let mut source_params = adapter.normalize_realtime_params(normalized_params, channels);
        source_params.sample_rate = input_sample_rate;
        let provider_sample_rate = adapter.realtime_target_sample_rate(input_sample_rate, channels);
        let mut provider_params = source_params.clone();
        provider_params.sample_rate = provider_sample_rate;
        let request = self
            .build_request(&adapter, &provider_params, channels)
            .await;
        let initial_message =
            adapter.initial_message(self.api_key.as_deref(), &provider_params, channels);

        ListenClientDual {
            adapter,
            request,
            initial_message,
            connect_policy: self.connect_policy,
            input_sample_rate,
            provider_sample_rate,
            provider_params,
            channels,
        }
    }
}

pub type ListenClientInput = MixedMessage<bytes::Bytes, ControlMessage>;
pub type ListenClientDualInput = MixedMessage<(bytes::Bytes, bytes::Bytes), ControlMessage>;

#[derive(Clone)]
pub struct ListenClient<A: RealtimeSttAdapter = DeepgramAdapter> {
    pub(crate) adapter: A,
    pub(crate) request: ClientRequestBuilder,
    pub(crate) initial_message: Option<Message>,
    pub(crate) connect_policy: Option<hypr_ws_client::client::WebSocketConnectPolicy>,
    pub(crate) input_sample_rate: u32,
    pub(crate) provider_sample_rate: u32,
    pub(crate) provider_params: ListenParams,
    pub(crate) channels: u8,
}

#[derive(Clone)]
pub struct ListenClientDual<A: RealtimeSttAdapter> {
    pub(crate) adapter: A,
    pub(crate) request: ClientRequestBuilder,
    pub(crate) initial_message: Option<Message>,
    pub(crate) connect_policy: Option<hypr_ws_client::client::WebSocketConnectPolicy>,
    pub(crate) input_sample_rate: u32,
    pub(crate) provider_sample_rate: u32,
    pub(crate) provider_params: ListenParams,
    pub(crate) channels: u8,
}

pub struct SingleHandle {
    command_tx: tokio::sync::mpsc::UnboundedSender<ForwarderCommand>,
}

pub struct DualFinalizeHandle {
    command_tx: tokio::sync::mpsc::UnboundedSender<ForwarderCommand>,
}

pub enum DualHandle {
    Native(DualFinalizeHandle),
    Split(DualFinalizeHandle),
}

enum ForwarderCommand {
    Finalize(tokio::sync::oneshot::Sender<()>),
}

pub trait FinalizeHandle: Send {
    fn finalize(&self) -> impl std::future::Future<Output = ()> + Send;
    fn expected_finalize_count(&self) -> usize;
}

impl FinalizeHandle for SingleHandle {
    async fn finalize(&self) {
        finalize_forwarder(&self.command_tx).await;
    }

    fn expected_finalize_count(&self) -> usize {
        1
    }
}

impl FinalizeHandle for DualHandle {
    async fn finalize(&self) {
        match self {
            DualHandle::Native(handle) | DualHandle::Split(handle) => {
                finalize_forwarder(&handle.command_tx).await;
            }
        }
    }

    fn expected_finalize_count(&self) -> usize {
        match self {
            DualHandle::Native(..) => 1,
            DualHandle::Split(..) => 2,
        }
    }
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
        let ListenClient {
            adapter,
            request,
            initial_message,
            connect_policy,
            input_sample_rate,
            provider_sample_rate,
            provider_params,
            channels,
        } = self;
        let encoder = adapter
            .create_audio_encoder(
                input_sample_rate,
                provider_sample_rate,
                &provider_params,
                channels,
            )
            .map_err(forwarder_send_error)?;
        let finalize_message = adapter.finalize_message();
        let ws = websocket_client_with_keep_alive(&request, &adapter, connect_policy);
        let (outbound_tx, outbound_rx) = tokio::sync::mpsc::channel::<TransformedInput>(32);
        let (error_tx, error_rx) = tokio::sync::mpsc::unbounded_channel();
        let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();

        let outbound = tokio_stream::wrappers::ReceiverStream::new(outbound_rx);
        let (raw_stream, _) = ws
            .from_audio::<ListenClientIO, _>(initial_message, outbound)
            .await?;

        tokio::spawn(forward_realtime_audio(
            audio_stream,
            outbound_tx,
            error_tx,
            encoder,
            finalize_message,
            command_rx,
            RealtimeAudioInput::Mono,
        ));

        let mapped_stream = futures_util::stream::select(
            map_raw_stream(raw_stream, adapter),
            tokio_stream::wrappers::UnboundedReceiverStream::new(error_rx).map(Err),
        );

        Ok((mapped_stream, SingleHandle { command_tx }))
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
        let ListenClientDual {
            adapter,
            request,
            initial_message,
            connect_policy,
            input_sample_rate,
            provider_sample_rate,
            provider_params,
            channels,
        } = self;
        let encoder = adapter
            .create_audio_encoder(
                input_sample_rate,
                provider_sample_rate,
                &provider_params,
                channels,
            )
            .map_err(forwarder_send_error)?;
        let finalize_message = adapter.finalize_message();
        let ws = websocket_client_with_keep_alive(&request, &adapter, connect_policy);
        let (outbound_tx, outbound_rx) = tokio::sync::mpsc::channel::<TransformedInput>(32);
        let (error_tx, error_rx) = tokio::sync::mpsc::unbounded_channel();
        let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();

        let outbound = tokio_stream::wrappers::ReceiverStream::new(outbound_rx);
        let (raw_stream, _) = ws
            .from_audio::<ListenClientIO, _>(initial_message, outbound)
            .await?;

        tokio::spawn(forward_realtime_audio(
            stream,
            outbound_tx,
            error_tx,
            encoder,
            finalize_message,
            command_rx,
            |(mic, speaker)| RealtimeAudioInput::Stereo { mic, speaker },
        ));

        let mapped_stream = futures_util::stream::select(
            map_raw_stream(raw_stream, adapter),
            tokio_stream::wrappers::UnboundedReceiverStream::new(error_rx).map(Err),
        );

        Ok((
            Box::pin(mapped_stream),
            DualHandle::Native(DualFinalizeHandle { command_tx }),
        ))
    }

    #[allow(clippy::wrong_self_convention)]
    async fn from_realtime_audio_split(
        self,
        stream: impl Stream<Item = ListenClientDualInput> + Send + Unpin + 'static,
    ) -> Result<(DualOutputStream, DualHandle), hypr_ws_client::Error> {
        let ListenClientDual {
            adapter,
            request,
            initial_message,
            connect_policy,
            input_sample_rate,
            provider_sample_rate,
            provider_params,
            channels,
        } = self;
        let (mic_tx, mic_rx) = tokio::sync::mpsc::channel::<TransformedInput>(32);
        let (spk_tx, spk_rx) = tokio::sync::mpsc::channel::<TransformedInput>(32);
        let (error_tx, error_rx) = tokio::sync::mpsc::unbounded_channel();
        let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();
        let mic_encoder = adapter
            .create_audio_encoder(
                input_sample_rate,
                provider_sample_rate,
                &provider_params,
                channels,
            )
            .map_err(forwarder_send_error)?;
        let spk_encoder = adapter
            .create_audio_encoder(
                input_sample_rate,
                provider_sample_rate,
                &provider_params,
                channels,
            )
            .map_err(forwarder_send_error)?;
        let finalize_message = adapter.finalize_message();

        let mic_ws = websocket_client_with_keep_alive(&request, &adapter, connect_policy.clone());
        let spk_ws = websocket_client_with_keep_alive(&request, &adapter, connect_policy);

        let mic_outbound = tokio_stream::wrappers::ReceiverStream::new(mic_rx);
        let spk_outbound = tokio_stream::wrappers::ReceiverStream::new(spk_rx);

        let mic_connect =
            mic_ws.from_audio::<ListenClientIO, _>(initial_message.clone(), mic_outbound);
        let spk_connect = spk_ws.from_audio::<ListenClientIO, _>(initial_message, spk_outbound);

        let ((mic_raw, _), (spk_raw, _)) = tokio::try_join!(mic_connect, spk_connect)?;

        tokio::spawn(forward_dual_to_single(
            stream,
            mic_tx,
            spk_tx,
            error_tx,
            mic_encoder,
            spk_encoder,
            finalize_message.clone(),
            finalize_message,
            command_rx,
        ));

        let adapter_for_mic = adapter.clone();
        let adapter_for_spk = adapter.clone();
        let merged_stream = futures_util::stream::select(
            merge_streams_with_channel_remap(
                map_raw_stream(mic_raw, adapter_for_mic),
                map_raw_stream(spk_raw, adapter_for_spk),
            ),
            tokio_stream::wrappers::UnboundedReceiverStream::new(error_rx).map(Err),
        );

        Ok((
            Box::pin(merged_stream),
            DualHandle::Split(DualFinalizeHandle { command_tx }),
        ))
    }
}

async fn forward_realtime_audio<S, T, E, F>(
    mut stream: S,
    outbound_tx: tokio::sync::mpsc::Sender<TransformedInput>,
    error_tx: tokio::sync::mpsc::UnboundedSender<hypr_ws_client::Error>,
    mut encoder: E,
    finalize_message: Message,
    mut command_rx: tokio::sync::mpsc::UnboundedReceiver<ForwarderCommand>,
    mut map_audio: F,
) where
    S: Stream<Item = MixedMessage<T, ControlMessage>> + Send + Unpin + 'static,
    E: RealtimeAudioEncoder,
    F: FnMut(T) -> RealtimeAudioInput + Send + 'static,
{
    loop {
        tokio::select! {
            maybe_cmd = command_rx.recv() => {
                match maybe_cmd {
                    Some(ForwarderCommand::Finalize(done_tx)) => {
                        let _ = finalize_encoder(
                            &mut encoder,
                            &outbound_tx,
                            &error_tx,
                            finalize_message.clone(),
                        )
                        .await;
                        let _ = done_tx.send(());
                        break;
                    }
                    None => break,
                }
            }
            maybe_msg = stream.next() => {
                match maybe_msg {
                    Some(MixedMessage::Audio(audio)) => {
                        let input = map_audio(audio);
                        if !push_encoder_messages(&mut encoder, input, &outbound_tx, &error_tx).await {
                            break;
                        }
                    }
                    Some(MixedMessage::Control(control)) => {
                        if !handle_forwarder_control(
                            control,
                            &mut encoder,
                            &outbound_tx,
                            &error_tx,
                            &finalize_message,
                        )
                        .await
                        {
                            break;
                        }
                    }
                    None => {
                        let _ = flush_encoder(&mut encoder, &outbound_tx, &error_tx).await;
                        break;
                    }
                }
            }
        }
    }
}

async fn handle_forwarder_control<E: RealtimeAudioEncoder>(
    control: ControlMessage,
    encoder: &mut E,
    outbound_tx: &tokio::sync::mpsc::Sender<TransformedInput>,
    error_tx: &tokio::sync::mpsc::UnboundedSender<hypr_ws_client::Error>,
    finalize_message: &Message,
) -> bool {
    match control {
        ControlMessage::KeepAlive => outbound_tx
            .send(MixedMessage::Control(ControlMessage::KeepAlive))
            .await
            .is_ok(),
        ControlMessage::Finalize => {
            let _ =
                finalize_encoder(encoder, outbound_tx, error_tx, finalize_message.clone()).await;
            false
        }
        ControlMessage::CloseStream => {
            let _ = flush_encoder(encoder, outbound_tx, error_tx).await;
            false
        }
    }
}

async fn finalize_split_forwarder<E1, E2>(
    mic_encoder: &mut E1,
    spk_encoder: &mut E2,
    mic_tx: &tokio::sync::mpsc::Sender<TransformedInput>,
    spk_tx: &tokio::sync::mpsc::Sender<TransformedInput>,
    error_tx: &tokio::sync::mpsc::UnboundedSender<hypr_ws_client::Error>,
    mic_finalize_message: Message,
    spk_finalize_message: Message,
) -> bool
where
    E1: RealtimeAudioEncoder,
    E2: RealtimeAudioEncoder,
{
    let mic_ok = finalize_encoder(mic_encoder, mic_tx, error_tx, mic_finalize_message).await;
    let spk_ok = finalize_encoder(spk_encoder, spk_tx, error_tx, spk_finalize_message).await;
    mic_ok && spk_ok
}

async fn flush_split_forwarder<E1, E2>(
    mic_encoder: &mut E1,
    spk_encoder: &mut E2,
    mic_tx: &tokio::sync::mpsc::Sender<TransformedInput>,
    spk_tx: &tokio::sync::mpsc::Sender<TransformedInput>,
    error_tx: &tokio::sync::mpsc::UnboundedSender<hypr_ws_client::Error>,
) -> bool
where
    E1: RealtimeAudioEncoder,
    E2: RealtimeAudioEncoder,
{
    let mic_ok = flush_encoder(mic_encoder, mic_tx, error_tx).await;
    let spk_ok = flush_encoder(spk_encoder, spk_tx, error_tx).await;
    mic_ok && spk_ok
}

async fn handle_split_forwarder_control<E1, E2>(
    control: ControlMessage,
    mic_encoder: &mut E1,
    spk_encoder: &mut E2,
    mic_tx: &tokio::sync::mpsc::Sender<TransformedInput>,
    spk_tx: &tokio::sync::mpsc::Sender<TransformedInput>,
    error_tx: &tokio::sync::mpsc::UnboundedSender<hypr_ws_client::Error>,
    mic_finalize_message: &Message,
    spk_finalize_message: &Message,
) -> bool
where
    E1: RealtimeAudioEncoder,
    E2: RealtimeAudioEncoder,
{
    match control {
        ControlMessage::KeepAlive => {
            if mic_tx
                .send(MixedMessage::Control(ControlMessage::KeepAlive))
                .await
                .is_err()
            {
                return false;
            }
            spk_tx
                .send(MixedMessage::Control(ControlMessage::KeepAlive))
                .await
                .is_ok()
        }
        ControlMessage::Finalize => {
            let _ = finalize_split_forwarder(
                mic_encoder,
                spk_encoder,
                mic_tx,
                spk_tx,
                error_tx,
                mic_finalize_message.clone(),
                spk_finalize_message.clone(),
            )
            .await;
            false
        }
        ControlMessage::CloseStream => {
            let _ = flush_split_forwarder(mic_encoder, spk_encoder, mic_tx, spk_tx, error_tx).await;
            false
        }
    }
}

async fn flush_split_forwarder_on_eof<E1, E2>(
    mic_encoder: &mut E1,
    spk_encoder: &mut E2,
    mic_tx: &tokio::sync::mpsc::Sender<TransformedInput>,
    spk_tx: &tokio::sync::mpsc::Sender<TransformedInput>,
    error_tx: &tokio::sync::mpsc::UnboundedSender<hypr_ws_client::Error>,
) where
    E1: RealtimeAudioEncoder,
    E2: RealtimeAudioEncoder,
{
    let _ = flush_split_forwarder(mic_encoder, spk_encoder, mic_tx, spk_tx, error_tx).await;
}

async fn forward_dual_to_single<E1, E2>(
    mut stream: impl Stream<Item = ListenClientDualInput> + Send + Unpin + 'static,
    mic_tx: tokio::sync::mpsc::Sender<TransformedInput>,
    spk_tx: tokio::sync::mpsc::Sender<TransformedInput>,
    error_tx: tokio::sync::mpsc::UnboundedSender<hypr_ws_client::Error>,
    mut mic_encoder: E1,
    mut spk_encoder: E2,
    mic_finalize_message: Message,
    spk_finalize_message: Message,
    mut command_rx: tokio::sync::mpsc::UnboundedReceiver<ForwarderCommand>,
) where
    E1: RealtimeAudioEncoder,
    E2: RealtimeAudioEncoder,
{
    loop {
        tokio::select! {
            maybe_cmd = command_rx.recv() => {
                match maybe_cmd {
                    Some(ForwarderCommand::Finalize(done_tx)) => {
                        let _ = finalize_split_forwarder(
                            &mut mic_encoder,
                            &mut spk_encoder,
                            &mic_tx,
                            &spk_tx,
                            &error_tx,
                            mic_finalize_message.clone(),
                            spk_finalize_message.clone(),
                        )
                        .await;
                        let _ = done_tx.send(());
                        break;
                    }
                    None => break,
                }
            }
            maybe_msg = stream.next() => {
                match maybe_msg {
                    Some(MixedMessage::Audio((mic, spk))) => {
                        if !push_encoder_messages(
                            &mut mic_encoder,
                            RealtimeAudioInput::Mono(mic),
                            &mic_tx,
                            &error_tx,
                        )
                        .await
                        {
                            break;
                        }
                        if !push_encoder_messages(
                            &mut spk_encoder,
                            RealtimeAudioInput::Mono(spk),
                            &spk_tx,
                            &error_tx,
                        )
                        .await
                        {
                            break;
                        }
                    }
                    Some(MixedMessage::Control(control)) => {
                        if !handle_split_forwarder_control(
                            control,
                            &mut mic_encoder,
                            &mut spk_encoder,
                            &mic_tx,
                            &spk_tx,
                            &error_tx,
                            &mic_finalize_message,
                            &spk_finalize_message,
                        )
                        .await
                        {
                            break;
                        }
                    }
                    None => {
                        flush_split_forwarder_on_eof(
                            &mut mic_encoder,
                            &mut spk_encoder,
                            &mic_tx,
                            &spk_tx,
                            &error_tx,
                        )
                        .await;
                        break;
                    }
                }
            }
        }
    }
}

async fn finalize_forwarder(command_tx: &tokio::sync::mpsc::UnboundedSender<ForwarderCommand>) {
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();
    if command_tx.send(ForwarderCommand::Finalize(done_tx)).is_ok() {
        let _ = done_rx.await;
    }
}

fn map_raw_stream<A, S>(
    raw_stream: S,
    adapter: A,
) -> impl Stream<Item = Result<StreamResponse, hypr_ws_client::Error>> + Send
where
    A: RealtimeSttAdapter,
    S: Stream<Item = Result<String, hypr_ws_client::Error>> + Send + 'static,
{
    raw_stream.flat_map(move |result| {
        let adapter = adapter.clone();
        let responses: Vec<Result<StreamResponse, hypr_ws_client::Error>> = match result {
            Ok(raw) => adapter.parse_response(&raw).into_iter().map(Ok).collect(),
            Err(error) => vec![Err(error)],
        };
        futures_util::stream::iter(responses)
    })
}

async fn push_encoder_messages<E: RealtimeAudioEncoder>(
    encoder: &mut E,
    input: RealtimeAudioInput,
    outbound_tx: &tokio::sync::mpsc::Sender<TransformedInput>,
    error_tx: &tokio::sync::mpsc::UnboundedSender<hypr_ws_client::Error>,
) -> bool {
    match encoder.push(input) {
        Ok(messages) => send_audio_messages(outbound_tx, messages).await,
        Err(error) => {
            let _ = error_tx.send(forwarder_send_error(error));
            false
        }
    }
}

async fn flush_encoder<E: RealtimeAudioEncoder>(
    encoder: &mut E,
    outbound_tx: &tokio::sync::mpsc::Sender<TransformedInput>,
    error_tx: &tokio::sync::mpsc::UnboundedSender<hypr_ws_client::Error>,
) -> bool {
    match encoder.flush() {
        Ok(messages) => send_audio_messages(outbound_tx, messages).await,
        Err(error) => {
            let _ = error_tx.send(forwarder_send_error(error));
            false
        }
    }
}

async fn finalize_encoder<E: RealtimeAudioEncoder>(
    encoder: &mut E,
    outbound_tx: &tokio::sync::mpsc::Sender<TransformedInput>,
    error_tx: &tokio::sync::mpsc::UnboundedSender<hypr_ws_client::Error>,
    finalize_message: Message,
) -> bool {
    flush_encoder(encoder, outbound_tx, error_tx).await
        && outbound_tx
            .send(MixedMessage::Audio(finalize_message))
            .await
            .is_ok()
}

async fn send_audio_messages(
    outbound_tx: &tokio::sync::mpsc::Sender<TransformedInput>,
    messages: Vec<Message>,
) -> bool {
    for message in messages {
        if outbound_tx
            .send(MixedMessage::Audio(message))
            .await
            .is_err()
        {
            return false;
        }
    }
    true
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

fn forwarder_send_error(error: crate::Error) -> hypr_ws_client::Error {
    hypr_ws_client::Error::SendError(error.to_string())
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

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use hypr_ws_client::client::Message;

    use super::{
        DualFinalizeHandle, DualHandle, ForwarderCommand, ListenClientDualInput, ListenClientInput,
        SingleHandle, TransformedInput, forward_dual_to_single, forward_realtime_audio,
    };
    use crate::test_utils::{run_dual_test, run_single_test};
    use crate::{
        AssemblyAIAdapter, DeepgramAdapter, FinalizeHandle, ListenClient, OpenAIAdapter,
        RealtimeAudioEncoder, RealtimeAudioInput, RealtimeSttAdapter, SonioxAdapter,
    };

    struct FlushOnFinalizeEncoder {
        label: &'static str,
        notify_tx: tokio::sync::mpsc::UnboundedSender<&'static str>,
        pending: Option<Message>,
    }

    impl FlushOnFinalizeEncoder {
        fn new(
            label: &'static str,
            notify_tx: tokio::sync::mpsc::UnboundedSender<&'static str>,
        ) -> Self {
            Self {
                label,
                notify_tx,
                pending: None,
            }
        }
    }

    impl RealtimeAudioEncoder for FlushOnFinalizeEncoder {
        fn push(&mut self, input: RealtimeAudioInput) -> Result<Vec<Message>, crate::Error> {
            let RealtimeAudioInput::Mono(audio) = input else {
                panic!("expected mono audio");
            };
            let _ = self.notify_tx.send(self.label);
            self.pending = Some(Message::Binary(audio));
            Ok(Vec::new())
        }

        fn flush(&mut self) -> Result<Vec<Message>, crate::Error> {
            Ok(self.pending.take().into_iter().collect())
        }
    }

    struct StereoMarkerEncoder;

    impl RealtimeAudioEncoder for StereoMarkerEncoder {
        fn push(&mut self, input: RealtimeAudioInput) -> Result<Vec<Message>, crate::Error> {
            let RealtimeAudioInput::Stereo { mic, speaker } = input else {
                panic!("expected stereo audio");
            };
            let mut payload = Vec::new();
            payload.extend_from_slice(&mic);
            payload.push(0xff);
            payload.extend_from_slice(&speaker);
            Ok(vec![Message::Binary(payload.into())])
        }

        fn flush(&mut self) -> Result<Vec<Message>, crate::Error> {
            Ok(Vec::new())
        }
    }

    fn proxy_base() -> String {
        std::env::var("PROXY_URL").unwrap_or_else(|_| "localhost:3001".to_string())
    }

    #[tokio::test]
    async fn single_handle_finalize_flushes_encoder_before_input_eof() {
        let (stream_tx, stream_rx) = tokio::sync::mpsc::channel(4);
        let (outbound_tx, mut outbound_rx) = tokio::sync::mpsc::channel(4);
        let (error_tx, mut error_rx) = tokio::sync::mpsc::unbounded_channel();
        let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();
        let (notify_tx, mut notify_rx) = tokio::sync::mpsc::unbounded_channel();
        let handle = SingleHandle { command_tx };

        let task = tokio::spawn(forward_realtime_audio(
            tokio_stream::wrappers::ReceiverStream::new(stream_rx),
            outbound_tx,
            error_tx,
            FlushOnFinalizeEncoder::new("single", notify_tx),
            Message::Text("finalize".into()),
            command_rx,
            RealtimeAudioInput::Mono,
        ));

        stream_tx
            .send(ListenClientInput::Audio(Bytes::from_static(b"tail")))
            .await
            .expect("stream send should succeed");
        assert_eq!(notify_rx.recv().await, Some("single"));
        handle.finalize().await;

        let Some(TransformedInput::Audio(Message::Binary(audio))) = outbound_rx.recv().await else {
            panic!("missing flushed audio");
        };
        assert_eq!(audio.as_ref(), b"tail");

        let Some(TransformedInput::Audio(Message::Text(finalize))) = outbound_rx.recv().await
        else {
            panic!("missing finalize message");
        };
        assert_eq!(finalize.as_str(), "finalize");

        assert!(error_rx.try_recv().is_err());
        let _: () = task.await.expect("forward task panicked");
    }

    #[tokio::test]
    async fn split_handle_finalize_flushes_both_channel_encoders_before_input_eof() {
        let (stream_tx, stream_rx) = tokio::sync::mpsc::channel(4);
        let (mic_tx, mut mic_rx) = tokio::sync::mpsc::channel(4);
        let (spk_tx, mut spk_rx) = tokio::sync::mpsc::channel(4);
        let (error_tx, mut error_rx) = tokio::sync::mpsc::unbounded_channel();
        let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();
        let (notify_tx, mut notify_rx) = tokio::sync::mpsc::unbounded_channel();
        let handle = DualHandle::Split(DualFinalizeHandle { command_tx });

        let task = tokio::spawn(forward_dual_to_single(
            tokio_stream::wrappers::ReceiverStream::new(stream_rx),
            mic_tx,
            spk_tx,
            error_tx,
            FlushOnFinalizeEncoder::new("mic", notify_tx.clone()),
            FlushOnFinalizeEncoder::new("spk", notify_tx),
            Message::Text("finalize".into()),
            Message::Text("finalize".into()),
            command_rx,
        ));

        stream_tx
            .send(ListenClientDualInput::Audio((
                Bytes::from_static(b"mic-tail"),
                Bytes::from_static(b"spk-tail"),
            )))
            .await
            .expect("stream send should succeed");
        let first = notify_rx.recv().await.expect("missing first push");
        let second = notify_rx.recv().await.expect("missing second push");
        assert!(matches!((first, second), ("mic", "spk") | ("spk", "mic")));

        handle.finalize().await;

        let Some(TransformedInput::Audio(Message::Binary(mic_audio))) = mic_rx.recv().await else {
            panic!("missing flushed mic audio");
        };
        let Some(TransformedInput::Audio(Message::Binary(spk_audio))) = spk_rx.recv().await else {
            panic!("missing flushed speaker audio");
        };
        assert_eq!(mic_audio.as_ref(), b"mic-tail");
        assert_eq!(spk_audio.as_ref(), b"spk-tail");

        let Some(TransformedInput::Audio(Message::Text(mic_finalize))) = mic_rx.recv().await else {
            panic!("missing mic finalize message");
        };
        let Some(TransformedInput::Audio(Message::Text(spk_finalize))) = spk_rx.recv().await else {
            panic!("missing speaker finalize message");
        };
        assert_eq!(mic_finalize.as_str(), "finalize");
        assert_eq!(spk_finalize.as_str(), "finalize");

        assert!(error_rx.try_recv().is_err());
        let _: () = task.await.expect("forward task panicked");
    }

    #[tokio::test]
    async fn native_dual_forwarder_uses_encoder_owned_stereo_input() {
        let stream = futures_util::stream::iter(vec![ListenClientDualInput::Audio((
            Bytes::from_static(b"mic"),
            Bytes::from_static(b"spk"),
        ))]);
        let (outbound_tx, mut outbound_rx) = tokio::sync::mpsc::channel(4);
        let (error_tx, mut error_rx) = tokio::sync::mpsc::unbounded_channel();
        let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel::<ForwarderCommand>();
        drop(command_tx);

        let task = tokio::spawn(forward_realtime_audio(
            stream,
            outbound_tx,
            error_tx,
            StereoMarkerEncoder,
            Message::Text("finalize".into()),
            command_rx,
            |(mic, speaker)| RealtimeAudioInput::Stereo { mic, speaker },
        ));

        let Some(TransformedInput::Audio(Message::Binary(audio))) = outbound_rx.recv().await else {
            panic!("missing stereo output");
        };
        assert_eq!(audio.as_ref(), b"mic\xffspk");

        assert!(error_rx.try_recv().is_err());
        let _: () = task.await.expect("forward task panicked");
    }

    #[tokio::test]
    async fn build_single_normalizes_languages_before_initial_message() {
        let client = ListenClient::builder()
            .adapter::<SonioxAdapter>()
            .api_base("https://api.soniox.com")
            .params(owhisper_interface::ListenParams {
                languages: vec![
                    "en-US".parse().unwrap(),
                    "en-GB".parse().unwrap(),
                    hypr_language::ISO639::En.into(),
                    "ko-KR".parse().unwrap(),
                ],
                ..Default::default()
            })
            .build_single()
            .await;

        let msg = client.initial_message.expect("missing initial message");
        let Message::Text(text) = msg else {
            panic!("expected text message");
        };
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        let hints = json["language_hints"].as_array().unwrap();

        assert_eq!(hints.len(), 2);
        assert_eq!(hints[0].as_str().unwrap(), "en");
        assert_eq!(hints[1].as_str().unwrap(), "ko");
    }

    #[tokio::test]
    async fn build_single_separates_input_and_provider_sample_rates() {
        let client = ListenClient::builder()
            .adapter::<OpenAIAdapter>()
            .api_base("https://api.openai.com")
            .params(owhisper_interface::ListenParams {
                languages: vec![hypr_language::ISO639::En.into()],
                sample_rate: 16_000,
                ..Default::default()
            })
            .build_single()
            .await;

        assert_eq!(client.input_sample_rate, 16_000);
        assert_eq!(client.provider_sample_rate, 24_000);
        assert_eq!(client.provider_params.sample_rate, 24_000);

        let msg = client.initial_message.expect("missing initial message");
        let Message::Text(text) = msg else {
            panic!("expected text message");
        };
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            json["session"]["audio"]["input"]["format"]["rate"]
                .as_u64()
                .unwrap(),
            24_000
        );
    }

    #[tokio::test]
    async fn build_single_default_adapter_uses_input_rate_as_provider_rate() {
        assert_eq!(
            SonioxAdapter::default().realtime_target_sample_rate(16_000, 1),
            16_000
        );

        let client = ListenClient::builder()
            .adapter::<SonioxAdapter>()
            .api_base("https://api.soniox.com")
            .params(owhisper_interface::ListenParams {
                sample_rate: 16_000,
                ..Default::default()
            })
            .build_single()
            .await;

        assert_eq!(client.input_sample_rate, 16_000);
        assert_eq!(client.provider_sample_rate, 16_000);
        assert_eq!(client.provider_params.sample_rate, 16_000);
    }

    #[tokio::test]
    async fn finite_single_stream_flushes_and_terminates_without_finalize() {
        let stream =
            futures_util::stream::iter(vec![ListenClientInput::Audio(Bytes::from_static(b"tail"))]);
        let (outbound_tx, mut outbound_rx) = tokio::sync::mpsc::channel(4);
        let (error_tx, mut error_rx) = tokio::sync::mpsc::unbounded_channel();
        let (_command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();
        let (notify_tx, mut notify_rx) = tokio::sync::mpsc::unbounded_channel();

        let task = tokio::spawn(forward_realtime_audio(
            stream,
            outbound_tx,
            error_tx,
            FlushOnFinalizeEncoder::new("single", notify_tx),
            Message::Text("finalize".into()),
            command_rx,
            RealtimeAudioInput::Mono,
        ));

        assert_eq!(notify_rx.recv().await, Some("single"));

        let Some(TransformedInput::Audio(Message::Binary(audio))) = outbound_rx.recv().await else {
            panic!("missing flushed audio");
        };
        assert_eq!(audio.as_ref(), b"tail");

        assert!(outbound_rx.recv().await.is_none());
        assert!(error_rx.try_recv().is_err());
        let _: () = task.await.expect("forward task panicked");
    }

    #[tokio::test]
    async fn finite_split_stream_flushes_and_terminates_without_finalize() {
        let stream = futures_util::stream::iter(vec![ListenClientDualInput::Audio((
            Bytes::from_static(b"mic-tail"),
            Bytes::from_static(b"spk-tail"),
        ))]);
        let (mic_tx, mut mic_rx) = tokio::sync::mpsc::channel(4);
        let (spk_tx, mut spk_rx) = tokio::sync::mpsc::channel(4);
        let (error_tx, mut error_rx) = tokio::sync::mpsc::unbounded_channel();
        let (_command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();
        let (notify_tx, mut notify_rx) = tokio::sync::mpsc::unbounded_channel();

        let task = tokio::spawn(forward_dual_to_single(
            stream,
            mic_tx,
            spk_tx,
            error_tx,
            FlushOnFinalizeEncoder::new("mic", notify_tx.clone()),
            FlushOnFinalizeEncoder::new("spk", notify_tx),
            Message::Text("finalize".into()),
            Message::Text("finalize".into()),
            command_rx,
        ));

        let first = notify_rx.recv().await.expect("missing first push");
        let second = notify_rx.recv().await.expect("missing second push");
        assert!(matches!((first, second), ("mic", "spk") | ("spk", "mic")));

        let Some(TransformedInput::Audio(Message::Binary(mic_audio))) = mic_rx.recv().await else {
            panic!("missing flushed mic audio");
        };
        let Some(TransformedInput::Audio(Message::Binary(spk_audio))) = spk_rx.recv().await else {
            panic!("missing flushed speaker audio");
        };
        assert_eq!(mic_audio.as_ref(), b"mic-tail");
        assert_eq!(spk_audio.as_ref(), b"spk-tail");

        assert!(mic_rx.recv().await.is_none());
        assert!(spk_rx.recv().await.is_none());
        assert!(error_rx.try_recv().is_err());
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
