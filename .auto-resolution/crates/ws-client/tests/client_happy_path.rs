mod common;

use common::{
    TEST_TIMEOUT, TestIO, collect_messages, echo_server, message_stream, spawn_ws_server,
    start_test_client, test_client, test_message,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::protocol::Message;

#[tokio::test]
async fn test_basic_echo() {
    let addr = echo_server().await;
    let client = test_client(addr);

    let messages = vec![test_message("hello", 1), test_message("world", 2)];

    let (output, _handle) = start_test_client(client, futures_util::stream::iter(messages.clone()))
        .await
        .unwrap();

    let received = collect_messages::<TestIO>(output, 2).await;
    assert_eq!(received, messages);
}

#[tokio::test]
async fn test_finalize() {
    let addr = echo_server().await;
    let client = test_client(addr);

    let (output, handle) = start_test_client(client, message_stream("initial"))
        .await
        .unwrap();

    let final_msg = test_message("final", 999);
    handle
        .finalize_with_text(serde_json::to_string(&final_msg).unwrap().into())
        .await;

    let received = collect_messages::<TestIO>(output, 2).await;
    assert_eq!(received.len(), 2);
    assert_eq!(received[1], final_msg);
}

#[tokio::test]
async fn test_keep_alive() {
    let addr = spawn_ws_server(move |ws_stream| async move {
        let (mut tx, mut rx) = ws_stream.split();

        let mut ping_count = 0;
        while let Some(Ok(msg)) = rx.next().await {
            if matches!(msg, Message::Ping(_)) {
                ping_count += 1;
                if ping_count >= 2 {
                    let response = test_message("done", ping_count as u32);
                    tx.send(Message::Text(
                        serde_json::to_string(&response).unwrap().into(),
                    ))
                    .await
                    .unwrap();
                    break;
                }
            }
        }
    })
    .await;

    let client = test_client(addr).with_keep_alive_message(
        std::time::Duration::from_millis(100),
        Message::Ping(vec![].into()),
    );

    let (output, _handle) = start_test_client(client, futures_util::stream::pending())
        .await
        .unwrap();

    let received = collect_messages::<TestIO>(output, 1).await;
    assert_eq!(received[0].text, "done");
    assert!(received[0].count >= 2);
}

#[tokio::test]
async fn test_dropping_output_cancels_background_send_task() {
    let (closed_tx, closed_rx) = oneshot::channel::<usize>();

    let addr = spawn_ws_server(move |ws_stream| async move {
        let (_tx, mut rx) = ws_stream.split();
        let mut messages_seen = 0usize;

        while let Some(result) = rx.next().await {
            match result {
                Ok(Message::Ping(_) | Message::Text(_) | Message::Binary(_)) => {
                    messages_seen += 1;
                }
                Ok(Message::Close(_)) => break,
                Ok(_) => {}
                Err(_) => break,
            }
        }

        let _ = closed_tx.send(messages_seen);
    })
    .await;

    let client = test_client(addr).with_keep_alive_message(
        std::time::Duration::from_millis(50),
        Message::Ping(vec![].into()),
    );

    let (output, _handle) = start_test_client(client, futures_util::stream::pending())
        .await
        .unwrap();

    drop(output);

    let messages_seen = tokio::time::timeout(TEST_TIMEOUT, closed_rx)
        .await
        .expect("connection should close promptly when the output stream is dropped")
        .expect("server should report closure");
    assert_eq!(messages_seen, 0, "unexpected outbound traffic after drop");
}

#[tokio::test]
async fn test_input_eof_closes_connection_without_finalize() {
    let (closed_tx, closed_rx) = oneshot::channel::<()>();

    let addr = spawn_ws_server(move |ws_stream| async move {
        let (_tx, mut rx) = ws_stream.split();

        while let Some(result) = rx.next().await {
            match result {
                Ok(Message::Close(_)) | Err(_) => break,
                Ok(_) => {}
            }
        }

        let _ = closed_tx.send(());
    })
    .await;

    let client = test_client(addr).with_keep_alive_message(
        std::time::Duration::from_millis(50),
        Message::Ping(vec![].into()),
    );

    let (output, _handle) = start_test_client(client, futures_util::stream::empty())
        .await
        .unwrap();
    let _output = output;

    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(6), closed_rx)
            .await
            .is_ok(),
        "connection should close after input EOF without explicit finalize"
    );
}
