//! Schedules pre-meeting brief preparation before upcoming calendar meetings start.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use chrono::DateTime;
use serde_json::Value;

const RETRY_COOLDOWN_MS: i64 = 60_000;

/// Non-all-day calendar events with a start timestamp, attendees, and shipping's ignored flags.
pub const UPCOMING_BRIEF_SQL: &str = "SELECT e.id, e.started_at, e.participants_json, e.attendance_json,
        EXISTS(SELECT 1 FROM app_settings a,
            json_each(CASE WHEN json_valid(a.value_json) AND json_type(a.value_json) = 'array' THEN a.value_json ELSE '[]' END) ignored
            WHERE ignored.type = 'object' AND (
                (a.id = 'ignored_events' AND e.tracking_id_event <> '' AND json_extract(ignored.value,'$.tracking_id') = e.tracking_id_event)
                OR (a.id = 'ignored_recurring_series' AND e.recurrence_series_id <> ''
                    AND json_extract(ignored.value,'$.id') = e.recurrence_series_id))) AS ignored
    FROM events e
    WHERE e.deleted_at IS NULL AND e.is_all_day = 0 AND e.started_at <> ''";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BriefMeeting {
    pub id: Arc<str>,
    pub started_at: Arc<str>,
    pub has_attendees: bool,
    pub declined: bool,
    pub ignored: bool,
}

impl BriefMeeting {
    pub fn from_row(row: &Value) -> Self {
        let participants_raw = row["participants_json"].as_str().unwrap_or("[]");
        let has_attendees = serde_json::from_str::<Vec<Value>>(participants_raw)
            .unwrap_or_default()
            .iter()
            .any(|participant| {
                participant.is_object()
                    && participant["is_current_user"] != true
                    && (!participant["email"]
                        .as_str()
                        .unwrap_or("")
                        .trim()
                        .is_empty()
                        || !participant["name"].as_str().unwrap_or("").trim().is_empty())
            });
        let declined = row["attendance_json"]
            .as_str()
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
            .is_some_and(|attendance| attendance["self_status"] == "declined");
        Self {
            id: row["id"].as_str().unwrap_or_default().into(),
            started_at: row["started_at"].as_str().unwrap_or_default().into(),
            has_attendees,
            declined,
            ignored: row["ignored"].as_i64() == Some(1) || row["ignored"] == true,
        }
    }

    pub fn start_ms(&self) -> Option<i64> {
        DateTime::parse_from_rfc3339(&self.started_at)
            .ok()
            .map(|time| time.timestamp_millis())
    }

    fn eligible(&self) -> bool {
        !self.id.is_empty() && !self.ignored && !self.declined && self.has_attendees
    }

    pub fn minutes_until_start(&self, now_ms: i64) -> Option<i64> {
        let start = self.start_ms()?;
        Some(((start - now_ms).max(0) + 59_999) / 60_000)
    }
}

#[derive(Default)]
pub struct BriefScheduler {
    meetings: Vec<BriefMeeting>,
    completed: HashSet<Arc<str>>,
    retry_after: HashMap<Arc<str>, i64>,
}

impl BriefScheduler {
    pub fn replace(&mut self, meetings: Vec<BriefMeeting>) {
        let live: HashSet<Arc<str>> = meetings.iter().map(|meeting| meeting.id.clone()).collect();
        self.completed.retain(|id| live.contains(id));
        self.retry_after.retain(|id, _| live.contains(id));
        self.meetings = meetings;
    }

    pub fn select_due(&self, now_ms: i64, lead_minutes: u32) -> Vec<BriefMeeting> {
        let lead_ms = i64::from(lead_minutes.max(1)) * 60_000;
        let mut due: Vec<&BriefMeeting> = self
            .meetings
            .iter()
            .filter(|meeting| meeting.eligible())
            .filter(|meeting| !self.completed.contains(&meeting.id))
            .filter(|meeting| {
                self.retry_after
                    .get(&meeting.id)
                    .is_none_or(|until| now_ms >= *until)
            })
            .filter(|meeting| {
                meeting
                    .start_ms()
                    .is_some_and(|start| now_ms <= start && start - now_ms <= lead_ms)
            })
            .collect();
        due.sort_by(|a, b| {
            a.start_ms()
                .unwrap_or(i64::MAX)
                .cmp(&b.start_ms().unwrap_or(i64::MAX))
                .then_with(|| a.id.cmp(&b.id))
        });
        due.into_iter().cloned().collect()
    }

    pub fn mark_done(&mut self, event_id: &str, permanently_done: bool, now_ms: i64) {
        if permanently_done {
            self.retry_after.remove(event_id);
            self.completed.insert(event_id.into());
        } else {
            self.retry_after
                .insert(event_id.into(), now_ms + RETRY_COOLDOWN_MS);
        }
    }

    pub fn next_wake(&self, now_ms: i64, lead_minutes: u32) -> Option<Duration> {
        let lead_ms = i64::from(lead_minutes.max(1)) * 60_000;
        let mut earliest: Option<i64> = None;
        for meeting in self.meetings.iter().filter(|meeting| meeting.eligible()) {
            if self.completed.contains(&meeting.id) {
                continue;
            }
            let Some(start) = meeting.start_ms() else {
                continue;
            };
            if start < now_ms {
                continue;
            }
            let mut wake_at = (start - lead_ms).max(now_ms);
            if let Some(retry_at) = self.retry_after.get(&meeting.id) {
                wake_at = wake_at.max(*retry_at);
            }
            if wake_at <= start {
                earliest = Some(earliest.map_or(wake_at, |best| best.min(wake_at)));
            }
        }
        earliest.map(|at| Duration::from_millis((at - now_ms).max(0) as u64))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn row(
        id: &str,
        started_at: &str,
        participants: Value,
        declined: bool,
        ignored: bool,
    ) -> BriefMeeting {
        BriefMeeting::from_row(&json!({
            "id": id,
            "started_at": started_at,
            "participants_json": participants.to_string(),
            "attendance_json": if declined {
                json!({"self_status": "declined"}).to_string()
            } else {
                json!({"self_status": "accepted"}).to_string()
            },
            "ignored": ignored,
        }))
    }

    #[test]
    fn selects_due_upcoming_meetings_in_lead_window_and_computes_wake() {
        let now = DateTime::parse_from_rfc3339("2026-11-25T10:00:00Z")
            .unwrap()
            .timestamp_millis();
        let attendees = json!([{"name": "Ada", "email": "ada@acme.com", "is_current_user": false}]);
        let solo = json!([{"name": "Me", "email": "me@acme.com", "is_current_user": true}]);

        let mut scheduler = BriefScheduler::default();
        scheduler.replace(vec![
            row(
                "past",
                "2026-11-25T09:59:00Z",
                attendees.clone(),
                false,
                false,
            ),
            row("solo", "2026-11-25T10:05:00Z", solo, false, false),
            row(
                "declined",
                "2026-11-25T10:05:00Z",
                attendees.clone(),
                true,
                false,
            ),
            row(
                "ignored",
                "2026-11-25T10:05:00Z",
                attendees.clone(),
                false,
                true,
            ),
            row(
                "due-soon",
                "2026-11-25T10:10:00Z",
                attendees.clone(),
                false,
                false,
            ),
            row("later", "2026-11-25T10:25:00Z", attendees, false, false),
        ]);

        let due = scheduler.select_due(now, 15);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id.as_ref(), "due-soon");
        assert_eq!(due[0].minutes_until_start(now), Some(10));

        assert_eq!(scheduler.next_wake(now, 15), Some(Duration::ZERO));
        scheduler.mark_done("due-soon", true, now);
        assert!(scheduler.select_due(now, 15).is_empty());
        assert_eq!(
            scheduler.next_wake(now, 15),
            Some(Duration::from_secs(10 * 60))
        );
    }

    #[test]
    fn transient_failure_waits_for_cooldown_before_retry() {
        let now = DateTime::parse_from_rfc3339("2026-11-25T10:00:00Z")
            .unwrap()
            .timestamp_millis();
        let attendees = json!([{"name": "Ada", "email": "ada@acme.com", "is_current_user": false}]);

        let mut scheduler = BriefScheduler::default();
        scheduler.replace(vec![row(
            "due",
            "2026-11-25T10:10:00Z",
            attendees,
            false,
            false,
        )]);

        scheduler.mark_done("due", false, now);
        assert!(scheduler.select_due(now + 10_000, 15).is_empty());
        assert_eq!(
            scheduler.next_wake(now + 10_000, 15),
            Some(Duration::from_millis(50_000))
        );
        assert_eq!(scheduler.select_due(now + RETRY_COOLDOWN_MS, 15).len(), 1);
    }
}
