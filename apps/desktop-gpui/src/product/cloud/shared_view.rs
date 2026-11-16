use std::sync::Arc;

use desktop_runtime::{CancellationToken, Result};
use gpui::{Context, Render, SharedString, Window, div, prelude::*, uniform_list};
use reqwest::Method;
use serde_json::{Value, json};

use super::{
    CloudServices,
    auth::SecureAuthProvider,
    transport::{Transport, failure},
};
use crate::ui::theme::theme;

pub struct SharedNote {
    title: Arc<str>,
    lines: Arc<[Arc<str>]>,
    _source: Value,
}

impl CloudServices {
    pub(super) async fn read_shared(
        &self,
        id: String,
        handoff: bool,
        cancel: CancellationToken,
    ) -> Result<SharedNote> {
        if cancel.is_cancelled() {
            return Err(desktop_runtime::ServiceError::Cancelled);
        }
        uuid::Uuid::parse_str(&id).map_err(|_| failure("Invalid shared-note identifier"))?;
        let this = self.clone();
        this.runtime.clone().service(move |_| async move {
            tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(desktop_runtime::ServiceError::Cancelled),
            result = async {
            let value = if handoff {
                let url = this.core.auth.transport.config.api.join("/shared-notes/handoffs/claim")
                    .map_err(|_| failure("Invalid shared-note endpoint"))?;
                Transport::checked(this.core.auth.transport.send(Method::POST, url, None,
                    Some(&json!({"request_id":id, "lease_id":uuid::Uuid::new_v4().to_string()})), &[]).await?)?
            } else {
                let account = this.identity().account_id.ok_or_else(|| failure("Sign in to open this shared note"))?;
                let lease = this.core.auth.lease(&account, false).await?;
                let response = this.core.rpc(&lease, "read_my_session_share_snapshot_v2", json!({"p_share_id":id})).await?;
                let rows = response.as_array().ok_or_else(|| failure("Invalid shared-note snapshot"))?;
                if rows.len() != 1 { return Err(failure("Shared note is unavailable or access was revoked")); }
                rows[0].clone()
            };
            let title = value["title"].as_str().ok_or_else(|| failure("Shared-note title is missing"))?;
            let body = value.get("body_json").or_else(|| value.get("body")).ok_or_else(|| failure("Shared-note body is missing"))?;
            let body = match body.as_str() {
                Some(text) => serde_json::from_str(text).map_err(|_| failure("Invalid shared-note document"))?,
                None => body.clone(),
            };
            let markdown = anlg_tiptap::tiptap_json_to_md(&body).map_err(|_| failure("Shared-note document could not be displayed"))?;
            if markdown.len() > 2 * 1024 * 1024 || title.len() > 4096 {
                return Err(failure("Shared note exceeds the preview limit"));
            }
            Ok(SharedNote { title: title.into(), lines: markdown.lines().map(Arc::from).collect(), _source: value })
            } => result
            }
        })?.receive().await
    }
}

pub struct SharedView {
    note: Option<SharedNote>,
    status: String,
    cancel: CancellationToken,
}

impl SharedView {
    pub fn new(cloud: CloudServices, id: String, handoff: bool, cx: &mut Context<Self>) -> Self {
        let cancel = CancellationToken::new();
        let pending = cancel.clone();
        if !handoff {
            let mut identities = cloud.identities();
            let account = identities.borrow_and_update().account_id.clone();
            let revoked = cancel.clone();
            cx.spawn(async move |this, cx| {
                loop {
                    let changed = matches!(
                        futures::future::select(
                            Box::pin(identities.changed()),
                            Box::pin(revoked.cancelled())
                        )
                        .await,
                        futures::future::Either::Left((Ok(()), _))
                    );
                    if !changed {
                        break;
                    }
                    if identities.borrow_and_update().account_id != account {
                        revoked.cancel();
                        let _ = this.update(cx, |this, cx| {
                            this.note = None;
                            this.status =
                                "Account changed. Reopen the shared note to authorize access."
                                    .into();
                            cx.notify();
                        });
                        break;
                    }
                }
            })
            .detach();
        }
        cx.spawn(async move |this, cx| {
            let result = cloud.read_shared(id, handoff, pending.clone()).await;
            if pending.is_cancelled() {
                return;
            }
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(note) => {
                        this.note = Some(note);
                        this.status = "Read-only shared note".into();
                    }
                    Err(error) => this.status = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
        Self {
            note: None,
            status: "Loading shared note…".into(),
            cancel,
        }
    }
}

impl Drop for SharedView {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl Render for SharedView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let title = self
            .note
            .as_ref()
            .map(|note| note.title.clone())
            .unwrap_or_else(|| "Shared note".into());
        let count = self.note.as_ref().map_or(0, |note| note.lines.len());
        div()
            .size_full()
            .flex()
            .flex_col()
            .p_6()
            .gap_3()
            .bg(colors.background)
            .child(div().text_xl().child(SharedString::from(title)))
            .child(
                div()
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child(self.status.clone()),
            )
            .child(
                uniform_list(
                    "shared-note-lines",
                    count,
                    cx.processor(|this, range: std::ops::Range<usize>, _, _| {
                        let Some(note) = &this.note else {
                            return Vec::new();
                        };
                        range
                            .map(|index| {
                                div()
                                    .h(gpui::px(28.))
                                    .overflow_hidden()
                                    .child(SharedString::from(note.lines[index].clone()))
                            })
                            .collect()
                    }),
                )
                .flex_1(),
            )
    }
}
