use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, EventId, Listener, Manager, Runtime};
use tokio::sync::{Mutex, Semaphore, mpsc, watch};
use tokio::task::JoinSet;

type PendingResultSender = tokio::sync::oneshot::Sender<serde_json::Value>;

#[derive(Clone, Default)]
pub struct PendingResults {
    inner: Arc<PendingResultsInner>,
}

#[derive(Default)]
struct PendingResultsInner {
    next_internal_id: AtomicU64,
    senders: Mutex<HashMap<u64, PendingResultSender>>,
}

impl PendingResults {
    async fn register(&self, sender: PendingResultSender) -> u64 {
        let mut senders = self.inner.senders.lock().await;
        loop {
            let internal_id = self.inner.next_internal_id.fetch_add(1, Ordering::Relaxed);
            if let std::collections::hash_map::Entry::Vacant(entry) = senders.entry(internal_id) {
                entry.insert(sender);
                return internal_id;
            }
        }
    }

    pub(crate) async fn take(&self, internal_id: u64) -> Option<PendingResultSender> {
        self.inner.senders.lock().await.remove(&internal_id)
    }

    async fn remove(&self, internal_id: u64) {
        self.inner.senders.lock().await.remove(&internal_id);
    }
}

type OutboundSender = mpsc::Sender<String>;

const MAX_CONCURRENT_REQUESTS: usize = 32;
const MAX_EVENT_SUBSCRIPTIONS: usize = 256;
const OUTBOUND_BUFFER_CAPACITY: usize = 32;
const REPLY_SEND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const SLOW_CLIENT_CLOSE_CODE: u16 = 1013;

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

    let (mut socket_sender, mut receiver) = socket.split();
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<String>(OUTBOUND_BUFFER_CAPACITY);
    let (overflow_tx, mut overflow_rx) = watch::channel(false);
    let mut writer_overflow_rx = overflow_rx.clone();
    let writer_closed_tx = overflow_tx.clone();
    let mut writer = tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                changed = writer_overflow_rx.changed() => {
                    if changed.is_ok() && *writer_overflow_rx.borrow() {
                        let _ = socket_sender
                            .send(Message::Close(Some(axum::extract::ws::CloseFrame {
                                code: SLOW_CLIENT_CLOSE_CODE,
                                reason: "relay outbound queue overflow".into(),
                            })))
                            .await;
                    }
                    break;
                }
                message = outbound_rx.recv() => {
                    let Some(json) = message else { break };
                    let send_result = tokio::select! {
                        biased;
                        changed = writer_overflow_rx.changed() => {
                            if changed.is_err() || !*writer_overflow_rx.borrow() {
                                return;
                            }
                            None
                        }
                        result = socket_sender.send(Message::Text(json.into())) => Some(result),
                    };
                    match send_result {
                        Some(Ok(())) => {}
                        Some(Err(_)) => {
                            writer_closed_tx.send_replace(true);
                            break;
                        }
                        None => {
                            let _ = socket_sender
                                .send(Message::Close(Some(axum::extract::ws::CloseFrame {
                                    code: SLOW_CLIENT_CLOSE_CODE,
                                    reason: "relay outbound queue overflow".into(),
                                })))
                                .await;
                            break;
                        }
                    }
                }
            }
        }
    });
    let event_subs: Arc<Mutex<HashMap<u64, EventId>>> = Arc::new(Mutex::new(HashMap::new()));
    let active_invokes = Arc::new(Mutex::new(HashSet::new()));
    let request_limit = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
    let mut request_tasks = JoinSet::new();

    loop {
        let msg = tokio::select! {
            biased;
            changed = overflow_rx.changed() => {
                if changed.is_err() || *overflow_rx.borrow() {
                    break;
                }
                continue;
            }
            msg = receiver.next() => {
                let Some(msg) = msg else { break };
                msg
            }
        };
        while request_tasks.try_join_next().is_some() {}

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

        let permit = tokio::select! {
            biased;
            changed = overflow_rx.changed() => {
                if changed.is_err() || *overflow_rx.borrow() {
                    break;
                }
                continue;
            }
            permit = request_limit.clone().acquire_owned() => match permit {
                Ok(permit) => permit,
                Err(_) => break,
            },
        };
        let id = req.id;
        let outbound_tx = outbound_tx.clone();
        let app = app.clone();
        let subs = event_subs.clone();
        let pending = pending.clone();
        let active_invokes = active_invokes.clone();
        let overflow_tx = overflow_tx.clone();
        let pending_invocation = if matches!(
            req.cmd.as_str(),
            "plugin:event|listen" | "plugin:event|unlisten"
        ) {
            None
        } else {
            let (tx, rx) = tokio::sync::oneshot::channel::<serde_json::Value>();
            let internal_id = pending.register(tx).await;
            active_invokes.lock().await.insert(internal_id);
            Some((internal_id, rx))
        };

        request_tasks.spawn(async move {
            let _permit = permit;
            let result = match req.cmd.as_str() {
                "plugin:event|listen" => {
                    event_listen(&app, id, &req.args, &outbound_tx, &overflow_tx, &subs).await
                }
                "plugin:event|unlisten" => event_unlisten(&app, id, &req.args, &subs).await,
                _ => {
                    let (internal_id, result_rx) = pending_invocation
                        .expect("non-event invocations always register a pending result");
                    let result = invoke(
                        &app,
                        id,
                        internal_id,
                        result_rx,
                        &req.cmd,
                        &req.args,
                        &pending,
                    )
                    .await;
                    active_invokes.lock().await.remove(&internal_id);
                    result
                }
            };

            let json = serde_json::to_string(&result).unwrap_or_default();
            send_reply(&outbound_tx, &overflow_tx, json).await;
        });
    }

    request_tasks.abort_all();
    while request_tasks.join_next().await.is_some() {}
    let active_invoke_ids = active_invokes.lock().await.drain().collect::<Vec<_>>();
    for internal_id in active_invoke_ids {
        pending.remove(internal_id).await;
    }

    let (listener_count, event_ids) = {
        let mut subs = event_subs.lock().await;
        let count = subs.len();
        let event_ids = subs
            .drain()
            .map(|(_, event_id)| event_id)
            .collect::<Vec<_>>();
        (count, event_ids)
    };
    for event_id in event_ids {
        app.unlisten(event_id);
    }
    drop(outbound_tx);
    if tokio::time::timeout(std::time::Duration::from_secs(1), &mut writer)
        .await
        .is_err()
    {
        writer.abort();
        let _ = writer.await;
    }
    tracing::info!(
        "[relay] browser disconnected, cleaned up {} event listeners",
        listener_count
    );
}

async fn send_reply(sender: &OutboundSender, overflow: &watch::Sender<bool>, json: String) -> bool {
    send_reply_with_timeout(sender, overflow, json, REPLY_SEND_TIMEOUT).await
}

async fn send_reply_with_timeout(
    sender: &OutboundSender,
    overflow: &watch::Sender<bool>,
    json: String,
    timeout: std::time::Duration,
) -> bool {
    match tokio::time::timeout(timeout, sender.send(json)).await {
        Ok(Ok(())) => true,
        Ok(Err(_)) => {
            overflow.send_replace(true);
            false
        }
        Err(_) => {
            tracing::warn!("[relay] closing slow browser after reply send timed out");
            overflow.send_replace(true);
            false
        }
    }
}

fn try_send_event(sender: &OutboundSender, overflow: &watch::Sender<bool>, json: String) -> bool {
    match sender.try_send(json) {
        Ok(()) => true,
        Err(mpsc::error::TrySendError::Full(_)) => {
            tracing::warn!("[relay] closing slow browser after outbound queue overflow");
            overflow.send_replace(true);
            false
        }
        Err(mpsc::error::TrySendError::Closed(_)) => false,
    }
}

async fn event_listen<R: Runtime>(
    app: &AppHandle<R>,
    id: u64,
    args: &serde_json::Value,
    sender: &OutboundSender,
    overflow: &watch::Sender<bool>,
    subs: &Arc<Mutex<HashMap<u64, EventId>>>,
) -> InvokeResponse {
    let event_name = args
        .get("event")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let handler_id = args.get("handler").and_then(|v| v.as_u64()).unwrap_or(0);

    let ws_sender = sender.clone();
    let overflow = overflow.clone();
    let event_name_clone = event_name.clone();
    let mut subscriptions = subs.lock().await;
    if !can_register_event_subscription(
        subscriptions.len(),
        subscriptions.contains_key(&handler_id),
    ) {
        return InvokeResponse {
            id,
            ok: false,
            payload: serde_json::Value::String(format!(
                "relay event subscription limit reached ({MAX_EVENT_SUBSCRIPTIONS})"
            )),
        };
    }
    let event_id = app.listen(&event_name, move |event| {
        let payload = serde_json::from_str::<serde_json::Value>(event.payload())
            .unwrap_or(serde_json::Value::Null);

        let push = EventPush {
            r#type: "event",
            handler: handler_id,
            payload: serde_json::json!({
                "event": event_name_clone,
                "id": handler_id,
                "payload": payload,
            }),
        };

        let json = serde_json::to_string(&push).unwrap_or_default();
        try_send_event(&ws_sender, &overflow, json);
    });
    let old_event_id = subscriptions.insert(handler_id, event_id);
    drop(subscriptions);
    if let Some(old_event_id) = old_event_id {
        app.unlisten(old_event_id);
    }

    InvokeResponse {
        id,
        ok: true,
        payload: serde_json::json!(handler_id),
    }
}

fn can_register_event_subscription(subscription_count: usize, replaces_existing: bool) -> bool {
    replaces_existing || subscription_count < MAX_EVENT_SUBSCRIPTIONS
}

async fn event_unlisten<R: Runtime>(
    app: &AppHandle<R>,
    id: u64,
    args: &serde_json::Value,
    subs: &Arc<Mutex<HashMap<u64, EventId>>>,
) -> InvokeResponse {
    let handler_id = event_handler_id(args);

    if let Some(event_id) = subs.lock().await.remove(&handler_id) {
        app.unlisten(event_id);
    }

    InvokeResponse {
        id,
        ok: true,
        payload: serde_json::Value::Null,
    }
}

fn event_handler_id(args: &serde_json::Value) -> u64 {
    args.get("eventId")
        .or_else(|| args.get("handler"))
        .and_then(|value| value.as_u64())
        .unwrap_or(0)
}

async fn invoke<R: Runtime>(
    app: &AppHandle<R>,
    browser_id: u64,
    internal_id: u64,
    rx: tokio::sync::oneshot::Receiver<serde_json::Value>,
    cmd: &str,
    args: &serde_json::Value,
    pending: &PendingResults,
) -> InvokeResponse {
    let args_json = serde_json::to_string(args).unwrap_or("{}".into());
    let cmd_json = serde_json::to_string(cmd).unwrap_or("\"\"".into());

    let js = format!(
        r#"(async function() {{
  try {{
    var r = await window.__TAURI_INTERNALS__.invoke({cmd_json}, {args_json});
    await window.__TAURI_INTERNALS__.invoke("plugin:relay|relay_result", {{
      id: {internal_id}, ok: true, data: r === undefined ? null : r
    }});
  }} catch(e) {{
    await window.__TAURI_INTERNALS__.invoke("plugin:relay|relay_result", {{
      id: {internal_id}, ok: false, data: (e && e.message) ? e.message : String(e)
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
                pending.remove(internal_id).await;
                return InvokeResponse {
                    id: browser_id,
                    ok: false,
                    payload: serde_json::Value::String(format!("eval failed: {e}")),
                };
            }
        }
        None => {
            pending.remove(internal_id).await;
            return InvokeResponse {
                id: browser_id,
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
                id: browser_id,
                ok,
                payload: data,
            }
        }
        Ok(Err(_)) => InvokeResponse {
            id: browser_id,
            ok: false,
            payload: serde_json::Value::String("relay channel dropped".into()),
        },
        Err(_) => {
            pending.remove(internal_id).await;
            InvokeResponse {
                id: browser_id,
                ok: false,
                payload: serde_json::Value::String("invoke timed out after 30s".into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_EVENT_SUBSCRIPTIONS, PendingResults, can_register_event_subscription, event_handler_id,
        send_reply, send_reply_with_timeout, try_send_event,
    };

    #[test]
    fn reads_tauri_event_id_for_unlisten_requests() {
        assert_eq!(event_handler_id(&serde_json::json!({ "eventId": 42 })), 42);
        assert_eq!(event_handler_id(&serde_json::json!({ "handler": 43 })), 43);
    }

    #[tokio::test]
    async fn signals_connection_close_instead_of_dropping_an_event_silently() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let (overflow_tx, overflow_rx) = tokio::sync::watch::channel(false);

        assert!(try_send_event(&tx, &overflow_tx, "first".to_string()));
        assert!(!try_send_event(&tx, &overflow_tx, "second".to_string()));
        assert!(*overflow_rx.borrow());
        assert_eq!(rx.recv().await.as_deref(), Some("first"));
    }

    #[tokio::test]
    async fn replies_wait_for_capacity_instead_of_being_dropped() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let (overflow_tx, overflow_rx) = tokio::sync::watch::channel(false);
        assert!(try_send_event(&tx, &overflow_tx, "event".to_string()));

        let reply_tx = tx.clone();
        let reply_overflow_tx = overflow_tx.clone();
        let reply = tokio::spawn(async move {
            send_reply(&reply_tx, &reply_overflow_tx, "reply".to_string()).await
        });
        tokio::task::yield_now().await;
        assert!(!reply.is_finished());

        assert_eq!(rx.recv().await.as_deref(), Some("event"));
        assert!(reply.await.unwrap());
        assert!(!*overflow_rx.borrow());
        assert_eq!(rx.recv().await.as_deref(), Some("reply"));
    }

    #[tokio::test]
    async fn reply_timeout_signals_connection_close() {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        tx.send("first".to_string()).await.unwrap();
        let (overflow_tx, overflow_rx) = tokio::sync::watch::channel(false);

        assert!(
            !send_reply_with_timeout(
                &tx,
                &overflow_tx,
                "reply".to_string(),
                std::time::Duration::from_millis(25),
            )
            .await
        );
        assert!(*overflow_rx.borrow());
    }

    #[tokio::test]
    async fn pending_results_use_server_unique_internal_ids() {
        let pending = PendingResults::default();
        let (first_tx, first_rx) = tokio::sync::oneshot::channel();
        let (second_tx, second_rx) = tokio::sync::oneshot::channel();

        let first_id = pending.register(first_tx).await;
        let second_id = pending.clone().register(second_tx).await;

        assert_ne!(first_id, second_id);
        pending
            .take(first_id)
            .await
            .unwrap()
            .send(serde_json::json!(1))
            .unwrap();
        pending
            .take(second_id)
            .await
            .unwrap()
            .send(serde_json::json!(2))
            .unwrap();
        assert_eq!(first_rx.await.unwrap(), serde_json::json!(1));
        assert_eq!(second_rx.await.unwrap(), serde_json::json!(2));
    }

    #[test]
    fn event_subscription_limit_allows_replacements_but_rejects_growth() {
        assert!(can_register_event_subscription(
            MAX_EVENT_SUBSCRIPTIONS,
            true
        ));
        assert!(!can_register_event_subscription(
            MAX_EVENT_SUBSCRIPTIONS,
            false
        ));
    }
}
