//! `services/audio-retention.ts` in the workspace: the minute tick that
//! finishes tombstoned audio and deletes expired recordings, and the
//! capture lifecycle's `deleteProcessedAudioForRetention`.

use gpui::Context;

use super::Workspace;
use super::recording::SessionMode;
use crate::audio_retention::{self, Policy};

fn flatten<T>(result: Result<anyhow::Result<T>, tokio::task::JoinError>) -> anyhow::Result<T> {
    result.map_err(anyhow::Error::from).and_then(|inner| inner)
}

impl Workspace {
    /// `normalizeAudioRetention(useConfigValue("audio_retention"))`
    fn audio_retention_policy(&self) -> Policy {
        audio_retention::normalize(
            self.provider_settings
                .value("audio_retention", &["general", "audio_retention"])
                .as_ref(),
        )
    }

    /// `isSessionAudioIdle`: no capture on the session and none starting.
    fn session_audio_idle(&self, session_id: &str) -> bool {
        self.session_mode(session_id) == SessionMode::Inactive && !self.recording.starting
    }

    /// The task manager's `audio-retention-cleanup` task: `cleanupExpiredAudio`
    /// every minute, starting now.
    pub(crate) fn start_audio_retention(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                if this
                    .update(cx, |this, cx| this.cleanup_expired_audio(cx))
                    .is_err()
                {
                    return;
                }
                cx.background_executor()
                    .timer(audio_retention::INTERVAL)
                    .await;
            }
        })
        .detach();
    }

    /// `cleanupExpiredAudio(policy)`: finish logically deleted audio, then
    /// delete the idle, processed, expired recordings.
    fn cleanup_expired_audio(&mut self, cx: &mut Context<Self>) {
        let policy = self.audio_retention_policy();
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let tombstoned = match flatten(store.logically_deleted_audio_sessions().await) {
                Ok(rows) => rows,
                Err(error) => {
                    tracing::error!(%error, "[audio-retention] failed to list deleted audio");
                    Vec::new()
                }
            };
            let mut deleted = Vec::new();
            for session_id in tombstoned {
                let idle = this
                    .update(cx, |this, _| this.session_audio_idle(&session_id))
                    .unwrap_or(false);
                if !idle {
                    continue;
                }
                match flatten(store.cleanup_deleted_session_audio(session_id.clone()).await) {
                    Ok(true) => deleted.push(session_id),
                    Ok(false) => {}
                    Err(error) => {
                        tracing::error!(%error, session_id, "[audio-retention] failed to finish audio deletion");
                    }
                }
            }
            if policy != Policy::Forever {
                let rows = match flatten(store.audio_retention_rows().await) {
                    Ok(rows) => rows,
                    Err(error) => {
                        tracing::error!(%error, "[audio-retention] failed to list sessions");
                        Vec::new()
                    }
                };
                let now_ms = chrono::Utc::now().timestamp_millis();
                for row in rows {
                    let idle = this
                        .update(cx, |this, _| this.session_audio_idle(&row.id))
                        .unwrap_or(false);
                    if !idle || !audio_retention::should_delete_expired(&row, policy, now_ms) {
                        continue;
                    }
                    match flatten(store.delete_local_session_audio(row.id.clone()).await) {
                        Ok(true) => deleted.push(row.id),
                        Ok(false) => {}
                        Err(error) => {
                            tracing::error!(%error, session_id = row.id, "[audio-retention] failed to delete audio");
                        }
                    }
                }
            }
            if !deleted.is_empty() {
                this.update(cx, |this, cx| this.on_session_audio_deleted(&deleted, cx))
                    .ok();
            }
        })
        .detach();
    }

    /// `deleteProcessedAudioForRetention(policy, sessionId)`: with the
    /// `none` policy, an idle session whose transcript is complete loses its
    /// recording right after the capture finishes.
    pub(crate) fn delete_processed_audio_for_retention(
        &mut self,
        session_id: String,
        cx: &mut Context<Self>,
    ) {
        if self.audio_retention_policy() != Policy::None || !self.session_audio_idle(&session_id) {
            return;
        }
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            if !matches!(
                flatten(store.session_audio_processed(session_id.clone()).await),
                Ok(true)
            ) {
                return;
            }
            let idle = this
                .update(cx, |this, _| this.session_audio_idle(&session_id))
                .unwrap_or(false);
            if !idle {
                return;
            }
            match flatten(store.delete_local_session_audio(session_id.clone()).await) {
                Ok(true) => {
                    this.update(cx, |this, cx| {
                        this.on_session_audio_deleted(std::slice::from_ref(&session_id), cx)
                    })
                    .ok();
                }
                Ok(false) => {}
                Err(error) => {
                    tracing::error!(%error, session_id, "[audio-retention] failed to delete audio");
                }
            }
        })
        .detach();
    }

    /// The `sessionAudioRetention` `deleted` event: the open note's header
    /// and player follow `useAudioExists`.
    fn on_session_audio_deleted(&mut self, session_ids: &[String], cx: &mut Context<Self>) {
        if let Some(selected) = self.selected.clone()
            && session_ids.contains(&selected)
        {
            self.reload_note(selected, cx);
        }
    }
}
