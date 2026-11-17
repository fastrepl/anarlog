//! Shipping's updater `maybe_emit_updated` and `useUpdaterEvents`: after an update, the next
//! launch opens "What's new". The first launch only records the version.

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

/// Records `current` as seen and returns the previously seen version.
async fn record(runtime: &RuntimeHandle, current: &str) -> Result<Option<String>> {
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
    cx.spawn(async move |this, cx| {
        let previous = match record(&runtime, current).await {
            Ok(previous) => previous,
            Err(error) => {
                tracing::warn!(%error, "last seen version was not recorded");
                return;
            }
        };
        if updated(previous.as_deref(), current) {
            let _ = this.update(cx, |this, cx| {
                this.workspace.update(cx, |workspace, cx| {
                    workspace.open_route(Route::Changelog, true, cx)
                })
            });
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_notes_open_only_after_the_version_changes() {
        assert!(!updated(None, "1.4.35"));
        assert!(!updated(Some(""), "1.4.35"));
        assert!(!updated(Some("1.4.35"), "1.4.35"));
        assert!(updated(Some("1.4.34"), "1.4.35"));
    }
}
