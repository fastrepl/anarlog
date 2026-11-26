//! Shipping's updater `maybe_emit_updated` and `useUpdaterEvents`: after an update, the next
//! launch opens "What's new". The first launch only records the version.

use std::sync::Arc;

use anlg_desktop_identity::store_json::StoreJson;
use desktop_runtime::{Result, RuntimeHandle, ServiceError};
use gpui::Context;
use serde_json::{Value, json};

use super::ApplicationView;
use crate::workspace::{changelog, navigation::Route};

const LAST_SEEN_KEY: &str = "gpui_last_seen_version";

/// Whether to show the release notes: a different version was seen before.
pub fn updated(last_seen: Option<&str>, current: &str) -> bool {
    last_seen.is_some_and(|last| !last.is_empty() && last != current)
}

/// Records `current` as seen and returns the previously seen version. A channel run shares
/// shipping's record in `store.json`, so the first launch after the update shows what's new.
async fn record(
    runtime: &RuntimeHandle,
    shipping: Option<Arc<StoreJson>>,
    current: &str,
) -> Result<Option<String>> {
    if let Some(store) = shipping {
        let current = current.to_owned();
        return runtime
            .service(move |_| async move {
                use crate::shipping_store::{LAST_SEEN_VERSION, UPDATER, read, write};
                let previous = read(store.clone(), UPDATER, LAST_SEEN_VERSION).await?;
                write(store, UPDATER, LAST_SEEN_VERSION, json!(current)).await?;
                Ok(previous.as_ref().and_then(Value::as_str).map(str::to_owned))
            })?
            .receive()
            .await;
    }
    let previous =
        crate::product::local::data::setting_value(runtime, LAST_SEEN_KEY.into()).await?;
    let stored = json!(current).to_string();
    runtime
        .submit(move |services| async move {
            services
                .executor
                .execute(
                    "INSERT INTO app_settings (id, value_json, updated_at) VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at".into(),
                    vec![json!(LAST_SEEN_KEY), json!(stored)],
                )
                .await
                .map(|_| ())
                .map_err(|error| ServiceError::Failed(error.to_string().into()))
        })?
        .receive()
        .await?;
    Ok(previous.as_ref().and_then(Value::as_str).map(str::to_owned))
}

/// Call once the runtime is ready.
pub(super) fn open_after_update(runtime: RuntimeHandle, cx: &mut Context<ApplicationView>) {
    let current = changelog::current_version();
    if current.is_empty() {
        return;
    }
    let shipping = crate::shipping_store::ShippingStore::get(cx);
    cx.spawn(async move |this, cx| {
        let previous = match record(&runtime, shipping, current).await {
            Ok(previous) => previous,
            Err(error) => {
                tracing::warn!(%error, "last seen version was not recorded");
                return;
            }
        };
        if updated(previous.as_deref(), current) {
            let _ = this.update(cx, |this, cx| {
                let pending = this.pending_product.take();
                this.workspace.update(cx, |workspace, cx| {
                    workspace.open_route(Route::Changelog, true, cx)
                });
                if let Some(mut pending) = pending {
                    pending.workspace_route = this.workspace.read(cx).current_route();
                    this.pending_product = Some(pending);
                }
            });
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first launch after the update from the Tauri app shows what's new, as a Tauri
    /// update would, and a rollback sees this version as seen.
    #[tokio::test]
    async fn channel_runs_share_shipping_last_seen_version() {
        let root = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(desktop_runtime::Profile {
            database: root.path().join("library.sqlite"),
        })
        .unwrap();
        ready.receive().await.unwrap();
        let store = Arc::new(StoreJson::at(root.path().join("store.json")));
        store
            .set("updater2", "LastSeenVersion", json!("1.4.35"))
            .unwrap();

        let previous = record(&runtime, Some(store.clone()), "1.5.0")
            .await
            .unwrap();

        assert_eq!(previous.as_deref(), Some("1.4.35"));
        assert!(updated(previous.as_deref(), "1.5.0"));
        assert_eq!(
            store.get("updater2", "LastSeenVersion").unwrap(),
            Some(json!("1.5.0"))
        );
        runtime.shutdown().await.unwrap();
    }

    #[test]
    fn release_notes_open_only_after_the_version_changes() {
        assert!(!updated(None, "1.4.35"));
        assert!(!updated(Some(""), "1.4.35"));
        assert!(!updated(Some("1.4.35"), "1.4.35"));
        assert!(updated(Some("1.4.34"), "1.4.35"));
    }
}
