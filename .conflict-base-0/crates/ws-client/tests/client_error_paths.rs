mod common;

use common::{
    TEST_TIMEOUT, TestIO, close_frame_server, first_output_result, invalid_message_server,
    reset_without_close_server, spawn_ws_server, test_client, test_message,
};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::{
    ClientRequestBuilder, Error as TungsteniteError,
    error::ProtocolError,
    protocol::{Message, frame::coding::CloseCode},
};
use ws_client::client::WebSocketClient;

#[tokio::test]
async fn test_invalid_request_returns_error_instead_of_panicking() {
    let client = WebSocketClient::new(
        ClientRequestBuilder::new("ws://127.0.0.1:1".parse().unwrap())
            .with_header("x-invalid", "bad\nvalue"),
    );

    let task = tokio::spawn(async move {
        client
            .from_audio::<TestIO, _>(None, futures_util::stream::empty())
            .await
    });

    let result = task.await.expect("invalid request should not panic");
    let error = match result {
        Ok(_) => panic!("invalid request should return an error"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("invalid request"),
        "unexpected error: {error:?}"
    );
}

#[tokio::test]
async fn test_reset_without_close_is_reported_as_error() {
    let addr = reset_without_close_server().await;
    let client = test_client(addr);

    let (output, _handle) = client
        .from_audio::<TestIO, _>(
            None,
            futures_util::stream::iter(vec![test_message("boom", 1)]),
        )
        .await
        .unwrap();
    let first = first_output_result(output).await;

    match first {
        Err(ws_client::Error::Connection(TungsteniteError::Protocol(
            ProtocolError::ResetWithoutClosingHandshake,
        ))) => {}
        other => panic!("expected reset protocol error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_invalid_payload_is_reported_as_parse_error() {
    let addr = invalid_message_server("not-json").await;
    let client = test_client(addr);

    let (output, _handle) = client
        .from_audio::<TestIO, _>(None, futures_util::stream::pending())
        .await
        .unwrap();
    let first = first_output_result(output).await;

    match first {
        Err(ws_client::Error::ParseError { .. }) => {}
        other => panic!("expected parse error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_remote_close_frame_is_reported_as_error() {
    let addr = close_frame_server(CloseCode::Policy, "policy").await;
    let client = test_client(addr);

    let (output, _handle) = client
        .from_audio::<TestIO, _>(None, futures_util::stream::pending())
        .await
        .unwrap();
    let first = first_output_result(output).await;

    match first {
        Err(ws_client::Error::RemoteClosed { code, reason, .. }) => {
            assert_eq!(code, Some(1008));
            assert_eq!(reason, "policy");
        }
        other => panic!("expected remote close error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_normal_close_frame_ends_stream_without_error() {
    let addr = close_frame_server(CloseCode::Normal, "").await;
    let client = test_client(addr);

    let (output, _handle) = client
        .from_audio::<TestIO, _>(None, futures_util::stream::pending())
        .await
        .unwrap();
    futures_util::pin_mut!(output);

    let next = tokio::time::timeout(TEST_TIMEOUT, output.next())
        .await
        .expect("stream should resolve");
    assert!(next.is_none(), "normal close should end the stream cleanly");
}

#[tokio::test]
async fn test_buffered_text_frame_is_yielded_before_send_error() {
    let addr = spawn_ws_server(move |ws_stream| async move {
        let mut ws_stream = ws_stream;

        let _ = ws_stream.next().await;
        ws_stream
            .send(Message::Text(
                serde_json::to_string(&test_message("provider-error", 9))
                    .unwrap()
                    .into(),
            ))
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        drop(ws_stream);
    })
    .await;

    let client = test_client(addr).with_keep_alive_message(
        std::time::Duration::from_millis(1),
        Message::Ping(vec![].into()),
    );

    let (output, _handle) = client
        .from_audio::<TestIO, _>(None, futures_util::stream::pending())
        .await
        .unwrap();
    futures_util::pin_mut!(output);

    tokio::time::sleep(std::time::Duration::from_millis(25)).await;

    let first = tokio::time::timeout(TEST_TIMEOUT, output.next())
        .await
        .expect("stream should resolve")
        .expect("stream should yield an item");
    match first {
        Ok(message) => assert_eq!(message, test_message("provider-error", 9)),
        other => panic!("expected buffered text frame before transport error, got {other:?}"),
    }

    let second = tokio::time::timeout(TEST_TIMEOUT, output.next())
        .await
        .expect("stream should resolve")
        .expect("stream should yield an item");
    assert!(
        second.is_err(),
        "expected transport error after buffered frame"
    );
}
