use std::net::SocketAddr;
use std::time::Duration;

use futures_util::StreamExt;
use owhisper_client::Provider;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

pub type ProxyWsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
pub type CloseInfo = Option<(u16, String)>;

pub async fn connect_to_proxy(
    proxy_addr: SocketAddr,
    provider: Provider,
    model: &str,
) -> ProxyWsStream {
    let provider_name = format!("{provider:?}").to_lowercase();
    let url = format!(
        "ws://{proxy_addr}/listen?provider={provider_name}&model={model}&encoding=linear16&sample_rate=16000&channels=1"
    );

    connect_to_url(&url).await
}

pub async fn connect_to_url(url: &str) -> ProxyWsStream {
    let (ws_stream, _) = connect_async(url)
        .await
        .expect("failed to connect to proxy websocket");
    ws_stream
}

pub async fn collect_text_messages(
    ws_stream: ProxyWsStream,
    timeout: Duration,
) -> (Vec<String>, CloseInfo) {
    let (mut _sender, mut receiver) = ws_stream.split();
    let mut messages = Vec::new();
    let mut close_info = None;

    let collect_future = async {
        while let Some(message) = receiver.next().await {
            match message {
                Ok(Message::Text(text)) => messages.push(text.to_string()),
                Ok(Message::Close(frame)) => {
                    close_info = frame.map(|frame| (frame.code.into(), frame.reason.to_string()));
                    break;
                }
                Ok(_) => {}
                Err(error) => {
                    eprintln!("websocket error: {error:?}");
                    break;
                }
            }
        }
    };

    let _ = tokio::time::timeout(timeout, collect_future).await;
    (messages, close_info)
}
