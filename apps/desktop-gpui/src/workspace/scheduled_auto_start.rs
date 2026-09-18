//! `ScheduledMeetingAutoStart` + `ScheduledSessionAutoStart`: with
//! `auto_start_scheduled_meetings` on, a linked calendar meeting that has
//! just started opens its note (`getOrCreateSessionForEventId`), optionally
//! opens the meeting link, and records automatically; a capture that turns
//! out empty is discarded by the lifecycle (`discardEmptyAutomaticCapture`).

use std::collections::HashSet;
use std::time::{Duration, Instant};

use gpui::Context;

use super::Workspace;
use crate::scheduled_auto_start::{
    Action, LiveStatus, ScheduledMeeting, TICK_MS, action, next_start_delay_ms, select_due_meetings,
};

/// `PendingScheduledSessionAutoStart`'s timeout: an armed session that cannot
/// start within this long gives the slot back.
const PENDING_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Default)]
pub(crate) struct AutoStartState {
    /// The linked, timed calendar events (`SCHEDULED_MEETINGS_SQL`).
    rows: Vec<ScheduledMeeting>,
    /// `firedEventIds`: meetings this run already started, skipped or ignored.
    fired: HashSet<String>,
    /// `starting`: a `startScheduledMeeting` is resolving its session.
    starting: bool,
    /// `tab.state.autoStart`: the session opened for a due meeting, waiting
    /// for its note to load so the capture can start.
    pending: Option<(String, Instant)>,
}

impl Workspace {
    /// The scheduler loop: the rows are re-read and the tick runs every
    /// `TICK_MS`, and exactly when the next meeting is due.
    pub(crate) fn start_scheduled_auto_start(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(task) = this.update(cx, |this, _| this.store.scheduled_meetings()) else {
                    return;
                };
                if let Ok(Ok(rows)) = task.await
                    && this
                        .update(cx, |this, _| this.auto_start.rows = rows)
                        .is_err()
                {
                    return;
                }
                let Ok(delay) = this.update(cx, |this, cx| {
                    this.tick_scheduled_auto_start(cx);
                    let now = chrono::Utc::now().timestamp_millis();
                    next_start_delay_ms(&this.auto_start.rows, now, &this.auto_start.fired)
                        .map_or(TICK_MS, |ms| (ms.max(1) as u64).min(TICK_MS))
                }) else {
                    return;
                };
                cx.background_executor()
                    .timer(Duration::from_millis(delay))
                    .await;
            }
        })
        .detach();
    }

    fn scheduled_live_status(&self) -> LiveStatus {
        if self.recording.live.is_some() || self.recording.starting {
            LiveStatus::Active
        } else if !self.recording.finalizing.is_empty()
            || !self.recording.pending_post_capture.is_empty()
        {
            LiveStatus::Finalizing
        } else {
            LiveStatus::Inactive
        }
    }

    /// `tick`
    fn tick_scheduled_auto_start(&mut self, cx: &mut Context<Self>) {
        if self
            .auto_start
            .pending
            .as_ref()
            .is_some_and(|(_, armed)| armed.elapsed() >= PENDING_TIMEOUT)
        {
            // `clearPendingAutoStart` after the 30s wait.
            self.auto_start.pending = None;
        }
        self.try_pending_auto_start(cx);
        if self.auto_start.starting {
            return;
        }
        let enabled = self.provider_settings.bool_setting(
            "auto_start_scheduled_meetings",
            &["general", "auto_start_scheduled_meetings"],
            false,
        );
        if !enabled {
            return;
        }
        let now = chrono::Utc::now().timestamp_millis();
        let due: Vec<ScheduledMeeting> =
            select_due_meetings(&self.auto_start.rows, now, &self.auto_start.fired)
                .into_iter()
                .cloned()
                .collect();
        // `hasScheduledAutoStartInFlight` / `hasPendingAutoStart`: transient,
        // the next tick looks again.
        if self.auto_start.pending.is_some() {
            return;
        }
        match action(self.scheduled_live_status()) {
            Action::Retry => return,
            Action::Skip => {
                // Do not let an overlapping meeting start after the active
                // recording ends.
                for row in &due {
                    self.auto_start.fired.insert(row.id.clone());
                }
                return;
            }
            Action::Start => {}
        }
        let Some(next) = due.first().cloned() else {
            return;
        };
        self.start_scheduled_meeting(next, due, cx);
    }

    /// `startScheduledMeeting`: an ignored event or series is claimed and
    /// skipped; otherwise its session is opened (created from the event when
    /// needed), the link opens when `auto_join_scheduled_meetings` is on, and
    /// the note tab is armed to start recording.
    fn start_scheduled_meeting(
        &mut self,
        next: ScheduledMeeting,
        due: Vec<ScheduledMeeting>,
        cx: &mut Context<Self>,
    ) {
        let ignored_events = super::calendar_tab::ignored_ids(
            &self.provider_settings,
            "ignored_events",
            "tracking_id",
        );
        let ignored_series = super::calendar_tab::ignored_ids(
            &self.provider_settings,
            "ignored_recurring_series",
            "id",
        );
        if ignored_events.contains(&next.tracking_id_event)
            || (!next.recurrence_series_id.is_empty()
                && ignored_series.contains(&next.recurrence_series_id))
        {
            self.auto_start.fired.insert(next.id);
            return;
        }
        let auto_join = self.provider_settings.bool_setting(
            "auto_join_scheduled_meetings",
            &["general", "auto_join_scheduled_meetings"],
            false,
        );
        self.auto_start.starting = true;
        let task = self.store.open_event_session(next.id.clone());
        cx.spawn(async move |this, cx| {
            let session = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update(cx, |this, cx| {
                this.auto_start.starting = false;
                let session_id = match session {
                    Ok(session_id) => session_id,
                    Err(error) => {
                        tracing::error!(%error, "[listener] failed to auto-start scheduled meeting");
                        return;
                    }
                };
                // A recording that started meanwhile makes this meeting an
                // overlapping one: `ignored`, claimed for good (#7466).
                if this.scheduled_live_status() == LiveStatus::Active {
                    this.auto_start.fired.insert(next.id);
                    return;
                }
                // `canStartLiveSession`: a finalizing capture or a running
                // batch on the session blocks the start; the next tick retries.
                if this.scheduled_live_status() != LiveStatus::Inactive
                    || this.session_mode(&session_id) != super::recording::SessionMode::Inactive
                {
                    return;
                }
                // Joining and listening are independent: the link opens as
                // soon as the meeting is due.
                if auto_join {
                    crate::opener::open_url(&next.meeting_link);
                }
                tracing::info!(
                    event_id = next.id,
                    session_id,
                    "[listener] auto-starting scheduled meeting"
                );
                // `openNew({ type: "sessions", id, state: { autoStart: true } })`
                this.open_tab(session_id.clone(), true, cx);
                this.auto_start.pending = Some((session_id, Instant::now()));
                // Anything overlapping the meeting just started is one the
                // user walked out of.
                for row in &due {
                    this.auto_start.fired.insert(row.id.clone());
                }
                this.try_pending_auto_start(cx);
            })
            .ok();
        })
        .detach();
    }

    /// `ReadyScheduledSessionAutoStart`: once the armed session's note is
    /// loaded and a capture can start, start it automatically.
    pub(crate) fn try_pending_auto_start(&mut self, cx: &mut Context<Self>) {
        let Some((session_id, _)) = self.auto_start.pending.clone() else {
            return;
        };
        let loaded = matches!(&self.note, super::Note::Ready { preview, .. } if preview.session.id == session_id);
        if !loaded || self.selected.as_deref() != Some(session_id.as_str()) {
            return;
        }
        if self.scheduled_live_status() != LiveStatus::Inactive
            || self.session_mode(&session_id) != super::recording::SessionMode::Inactive
        {
            return;
        }
        self.auto_start.pending = None;
        self.start_listening_with(session_id, true, cx);
    }
}
