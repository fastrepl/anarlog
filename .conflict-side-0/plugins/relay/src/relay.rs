use std::{collections::HashMap, sync::Arc};

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, EventId, Listener, Manager, Runtime};
use tokio::sync::Mutex;

pub type PendingResults = Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<serde_json::Value>>>>;
pub type WsSender = Arc<Mutex<SplitSink<WebSocket, Message>>>;

#[derive(Deserialize)]
pub struct InvokeRequest {
    pub id: u64,
    pub cmd: String,
    pub args: serde_json::Value,
}

#[derive(Serialize)]
pub struct InvokeResponse {
    pub id: u64,
    pub ok: bool,
    pub payload: serde_json::Value,
}

#[derive(Serialize)]
pub struct EventPush {
    pub r#type: &'static str,
    pub handler: u64,
    pub payload: serde_json::Value,
}

pub async fn handle_ws<R: Runtime>(socket: WebSocket, app: AppHandle<R>, pending: PendingResults) {
    tracing::info!("[relay] browser connected");

    let (sender, mut receiver) = socket.split();
    let sender: WsSender = Arc::new(Mutex::new(sender));
    let event_subs: Arc<Mutex<HashMap<u64, EventId>>> = Arc::new(Mutex::new(HashMap::new()));

    while let Some(msg) = receiver.next().await {
        let msg = match msg {
            Ok(Message::Text(t)) => t,
            Ok(Message::Close(_)) => break,
            Ok(_) => continue,
            Err(e) => {
                tracing::warn!("[relay] ws recv error: {e}");
                break;
            }
        };

        let req: InvokeRequest = match serde_json::from_str(&msg) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("[relay] bad request: {e}");
                continue;
            }
        };

        let id = req.id;
        let sender_clone = sender.clone();
        let app = app.clone();
        let subs = event_subs.clone();
        let pending = pending.clone();

        tokio::spawn(async move {
            let result = match req.cmd.as_str() {
                "plugin:event|listen" => {
                    event_listen(&app, id, &req.args, &sender_clone, &subs).await
                }
                "plugin:event|unlisten" => event_unlisten(&app, id, &req.args, &subs).await,
                _ => invoke(&app, id, &req.cmd, &req.args, &pending).await,
            };

            let json = serde_json::to_string(&result).unwrap_or_default();
            let mut tx = sender_clone.lock().await;
            let _ = tx.send(Message::Text(json.into())).await;
        });
    }

    let subs = event_subs.lock().await;
    for (_, event_id) in subs.iter() {
        app.unlisten(*event_id);
    }
    tracing::info!(
        "[relay] browser disconnected, cleaned up {} event listeners",
        subs.len()
    );
}

async fn event_listen<R: Runtime>(
    app: &AppHandle<R>,
    id: u64,
    args: &serde_json::Value,
    sender: &WsSender,
    subs: &Arc<Mutex<HashMap<u64, EventId>>>,
) -> InvokeResponse {
    let event_name = args
        .get("event")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let handler_id = args.get("handler").and_then(|v| v.as_u64()).unwrap_or(0);

    let ws_sender = sender.clone();
    let event_name_clone = event_name.clone();
    let event_id = app.listen(&event_name, move |event| {
        let payload = serde_json::from_str::<serde_json::Value>(event.payload())
            .unwrap_or(serde_json::Value::Null);

        let push = EventPush {
            r#type: "event",
            handler: handler_id,
            payload: serde_json::json!({
                "event": event_name_clone,
                "id": 0,
                "payload": payload,
            }),
        };

        let json = serde_json::to_string(&push).unwrap_or_default();
        let sender = ws_sender.clone();
        tauri::async_runtime::spawn(async move {
            let mut tx = sender.lock().await;
            let _ = tx.send(Message::Text(json.into())).await;
        });
    });

    if let Some(old_event_id) = subs.lock().await.insert(handler_id, event_id) {
        app.unlisten(old_event_id);
    }

    InvokeResponse {
        id,
        ok: true,
        payload: serde_json::json!(handler_id),
    }
}

async fn event_unlisten<R: Runtime>(
    app: &AppHandle<R>,
    id: u64,
    args: &serde_json::Value,
    subs: &Arc<Mutex<HashMap<u64, EventId>>>,
) -> InvokeResponse {
    let handler_id = args.get("handler").and_then(|v| v.as_u64()).unwrap_or(0);

    if let Some(event_id) = subs.lock().await.remove(&handler_id) {
        app.unlisten(event_id);
    }

    InvokeResponse {
        id,
        ok: true,
        payload: serde_json::Value::Null,
    }
}

async fn invoke<R: Runtime>(
    app: &AppHandle<R>,
    id: u64,
    cmd: &str,
    args: &serde_json::Value,
    pending: &PendingResults,
) -> InvokeResponse {
    let (tx, rx) = tokio::sync::oneshot::channel::<serde_json::Value>();
    pending.lock().await.insert(id, tx);

    let args_json = serde_json::to_string(args).unwrap_or("{}".into());
    let cmd_json = serde_json::to_string(cmd).unwrap_or("\"\"".into());

    let js = format!(
        r#"(async function() {{
  try {{
    var r = await window.__TAURI_INTERNALS__.invoke({cmd_json}, {args_json});
    await window.__TAURI_INTERNALS__.invoke("plugin:relay|relay_result", {{
      id: {id}, ok: true, data: r === undefined ? null : r
    }});
  }} catch(e) {{
    await window.__TAURI_INTERNALS__.invoke("plugin:relay|relay_result", {{
      id: {id}, ok: false, data: (e && e.message) ? e.message : String(e)
    }});
  }}
}})()"#,
    );

    let webview = app
        .webview_windows()
        .into_iter()
        .find(|(label, _)| label == "main")
        .map(|(_, w)| w);

    match webview {
        Some(w) => {
            if let Err(e) = w.eval(&js) {
                pending.lock().await.remove(&id);
                return InvokeResponse {
                    id,
                    ok: false,
                    payload: serde_json::Value::String(format!("eval failed: {e}")),
                };
            }
        }
        None => {
            pending.lock().await.remove(&id);
            return InvokeResponse {
                id,
                ok: false,
                payload: serde_json::Value::String("main webview not found".into()),
            };
        }
    }

    match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
        Ok(Ok(value)) => {
            let ok = value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
            let data = value
                .get("data")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            InvokeResponse {
                id,
                ok,
                payload: data,
            }
        }
        Ok(Err(_)) => InvokeResponse {
            id,
            ok: false,
            payload: serde_json::Value::String("relay channel dropped".into()),
        },
        Err(_) => {
            pending.lock().await.remove(&id);
            InvokeResponse {
                id,
                ok: false,
                payload: serde_json::Value::String("invoke timed out after 30s".into()),
            }
        }
    }
}
