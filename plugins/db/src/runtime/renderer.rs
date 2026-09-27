use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use tauri::ipc::Channel;

use crate::QueryEvent;

#[derive(Default)]
pub(super) struct RendererSubscriptions {
    sessions: Mutex<HashMap<String, Arc<RendererSession>>>,
}

impl RendererSubscriptions {
    pub(super) fn session(&self, label: &str) -> Arc<RendererSession> {
        self.sessions
            .lock()
            .unwrap()
            .entry(label.to_string())
            .or_default()
            .clone()
    }

    pub(super) fn close(&self, label: &str) -> Vec<String> {
        self.sessions
            .lock()
            .unwrap()
            .remove(label)
            .map(|session| session.close())
            .unwrap_or_default()
    }

    pub(super) fn remove(&self, subscription_id: &str) {
        for session in self.sessions.lock().unwrap().values() {
            session.remove(subscription_id);
        }
    }
}

pub(super) struct RendererSession {
    subscriptions: Mutex<Option<HashSet<String>>>,
}

impl Default for RendererSession {
    fn default() -> Self {
        Self {
            subscriptions: Mutex::new(Some(HashSet::new())),
        }
    }
}

impl RendererSession {
    pub(super) fn register(&self, subscription_id: &str) -> bool {
        let mut subscriptions = self.subscriptions.lock().unwrap();
        let Some(subscriptions) = subscriptions.as_mut() else {
            return false;
        };
        subscriptions.insert(subscription_id.to_string());
        true
    }

    fn remove(&self, subscription_id: &str) {
        if let Some(subscriptions) = self.subscriptions.lock().unwrap().as_mut() {
            subscriptions.remove(subscription_id);
        }
    }

    fn close(&self) -> Vec<String> {
        self.subscriptions
            .lock()
            .unwrap()
            .take()
            .unwrap_or_default()
            .into_iter()
            .collect()
    }

    pub(super) fn send(
        &self,
        channel: &Channel<QueryEvent>,
        event: QueryEvent,
    ) -> std::result::Result<(), String> {
        let subscriptions = self.subscriptions.lock().unwrap();
        if subscriptions.is_none() {
            return Err("live query renderer has closed".to_string());
        }
        channel.send(event).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use anlg_db_reactive::QueryEventSink;

    use super::*;
    use crate::runtime::PluginDbRuntime;

    #[test]
    fn closing_a_document_rejects_pending_registrations_and_later_delivery() {
        let renderers = RendererSubscriptions::default();
        let session = renderers.session("main");
        assert!(session.register("old-subscription"));

        assert_eq!(renderers.close("main"), vec!["old-subscription"]);
        assert!(!session.register("late-subscription"));
        let channel = Channel::new(|_| panic!("closed document received an event"));
        assert!(session.send(&channel, QueryEvent::Result(vec![])).is_err());

        let replacement = renderers.session("main");
        assert!(replacement.register("new-subscription"));
        assert!(!session.register("another-late-subscription"));
        assert_eq!(renderers.close("main"), vec!["new-subscription"]);
    }

    #[test]
    fn closing_one_renderer_preserves_other_windows_and_ignores_unsubscribed_queries() {
        let renderers = RendererSubscriptions::default();
        let main = renderers.session("main");
        let note = renderers.session("note");
        assert!(main.register("main-query"));
        assert!(main.register("removed-query"));
        assert!(note.register("note-query"));
        renderers.remove("removed-query");

        assert_eq!(renderers.close("main"), vec!["main-query"]);
        assert!(renderers.close("main").is_empty());
        assert!(note.register("another-note-query"));
        assert_eq!(renderers.close("note").len(), 2);
    }

    async fn runtime() -> PluginDbRuntime {
        let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
        PluginDbRuntime::new(Arc::new(db))
    }

    #[tokio::test]
    async fn renderer_loss_releases_reactive_and_non_reactive_registrations() {
        let runtime = runtime().await;
        let delivered = Arc::new(AtomicUsize::new(0));
        let mut registrations = Vec::new();
        for sql in ["SELECT id FROM templates", "SELECT * FROM missing_table"] {
            let delivered = delivered.clone();
            let sink = runtime.query_channel(
                "main",
                Channel::new(move |_| {
                    delivered.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }),
            );
            let registration = runtime
                .subscribe(sql.to_string(), vec![], sink.clone())
                .await
                .unwrap();
            registrations.push((registration.id, sink));
        }
        assert_eq!(delivered.load(Ordering::SeqCst), 2);

        let closed = runtime.close_webview_subscriptions("main");
        assert_eq!(closed.len(), 2);
        for (id, sink) in registrations {
            assert!(closed.contains(&id));
            assert!(sink.send_result(vec![]).is_err());
            runtime.unsubscribe(&id).await.unwrap();
            assert!(
                runtime
                    .live_query_runtime
                    .dependency_analysis(&id)
                    .await
                    .is_none()
            );
        }
        assert_eq!(delivered.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn renderer_loss_during_setup_does_not_leak_a_subscription() {
        let runtime = runtime().await;
        let sink = runtime.query_channel(
            "main",
            Channel::new(|_| panic!("closed document received an initial result")),
        );
        assert!(runtime.close_webview_subscriptions("main").is_empty());
        assert!(
            runtime
                .subscribe("SELECT id FROM templates".to_string(), vec![], sink)
                .await
                .is_err()
        );
        assert!(runtime.close_webview_subscriptions("main").is_empty());

        let replacement = runtime.query_channel("main", Channel::new(|_| Ok(())));
        let registration = runtime
            .subscribe("SELECT id FROM templates".to_string(), vec![], replacement)
            .await
            .unwrap();
        runtime.unsubscribe(&registration.id).await.unwrap();
        assert!(runtime.close_webview_subscriptions("main").is_empty());
    }
}
