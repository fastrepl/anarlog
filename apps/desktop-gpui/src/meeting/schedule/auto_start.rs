//! Shipping's "Start when meeting begins" (`stt/scheduled-auto-start.tsx`): a calendar meeting
//! with a link starts recording when it begins, or within five minutes after (the app may have
//! been asleep). Time is passed in, so every decision here runs on a fake clock in tests.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use chrono::DateTime;
use serde_json::Value;

use super::{apps::MicApp, attendance};

pub const GRACE_MS: i64 = 5 * 60_000;
/// A meeting app that held the microphone this long before a meeting is on an earlier call.
const PREVIOUS_CALL_MIN_HOLD_MS: i64 = 10 * 60_000;
/// People often join a few minutes before the start.
const EARLY_JOIN_MS: i64 = 5 * 60_000;
pub const RETRY: Duration = Duration::from_secs(15);
pub const MIC_HOLD_POLL: Duration = Duration::from_secs(30);
const MIC_HOLD_LOOKAHEAD_MS: i64 = PREVIOUS_CALL_MIN_HOLD_MS + 5 * 60_000;

/// Calendar blocks without a meeting link ("Lunch", "Focus time") never start: auto-start
/// watches every calendar. `ignored` mirrors shipping's ignored events and series.
pub const MEETINGS_SQL: &str = "SELECT e.id, e.started_at, e.ended_at, e.meeting_link,
        e.tracking_id_event, e.recurrence_series_id, e.attendance_json,
        EXISTS(SELECT 1 FROM app_settings a,
            json_each(CASE WHEN json_valid(a.value_json) AND json_type(a.value_json) = 'array' THEN a.value_json ELSE '[]' END) ignored
            WHERE ignored.type = 'object' AND (
                (a.id = 'ignored_events' AND e.tracking_id_event <> '' AND json_extract(ignored.value,'$.tracking_id') = e.tracking_id_event)
                OR (a.id = 'ignored_recurring_series' AND e.recurrence_series_id <> ''
                    AND json_extract(ignored.value,'$.id') = e.recurrence_series_id))) AS ignored
    FROM events e
    WHERE e.deleted_at IS NULL AND e.is_all_day = 0 AND e.started_at <> '' AND e.meeting_link <> ''";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Meeting {
    pub id: Arc<str>,
    pub started_at: Arc<str>,
    pub ended_at: Arc<str>,
    pub meeting_link: Arc<str>,
    pub tracking_id: Arc<str>,
    pub series_id: Arc<str>,
    pub attendance_json: Option<Arc<str>>,
    pub ignored: bool,
}

fn text(row: &Value, key: &str) -> Arc<str> {
    row[key].as_str().unwrap_or_default().into()
}

impl Meeting {
    pub fn from_row(row: &Value) -> Self {
        Self {
            id: text(row, "id"),
            started_at: text(row, "started_at"),
            ended_at: text(row, "ended_at"),
            meeting_link: text(row, "meeting_link"),
            tracking_id: text(row, "tracking_id_event"),
            series_id: text(row, "recurrence_series_id"),
            attendance_json: row["attendance_json"].as_str().map(Into::into),
            ignored: row["ignored"].as_i64() == Some(1) || row["ignored"] == true,
        }
    }

    pub fn start_ms(&self) -> Option<i64> {
        instant_ms(&self.started_at)
    }

    fn end_ms(&self) -> Option<i64> {
        instant_ms(&self.ended_at)
    }

    /// Due now: eligible attendance and started at most the grace period ago.
    pub fn due(&self, now_ms: i64) -> bool {
        attendance::decide(self.attendance_json.as_deref(), now_ms).eligible()
            && self
                .start_ms()
                .is_some_and(|start| (0..=GRACE_MS).contains(&(now_ms - start)))
    }
}

fn instant_ms(value: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|time| time.timestamp_millis())
}

/// Shipping's `selectDueMeetings`: back-to-back meetings overlap inside the grace window, and
/// the one that just started is the one the user is walking into, so the newest start leads.
pub fn select_due(meetings: &[Meeting], now_ms: i64, fired: &HashSet<Arc<str>>) -> Vec<Meeting> {
    let mut due: Vec<&Meeting> = meetings
        .iter()
        .filter(|meeting| !fired.contains(&meeting.id) && meeting.due(now_ms))
        .collect();
    due.sort_by_key(|meeting| std::cmp::Reverse(meeting.start_ms()));
    due.into_iter().cloned().collect()
}

/// What the live capture is doing, as far as starting another meeting is concerned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Live {
    Idle,
    /// A start is in flight or the previous meeting is finalizing; try again shortly.
    Busy,
    Recording,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Start(Meeting),
    Wait(Duration),
    Idle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Started,
    /// Another recording took over or the event is ignored: never start this one.
    Ignored,
    /// Transient (capture busy, auto-start switched off mid-start): retry on the next tick.
    Blocked,
    /// The event changed under the start (declined, rescheduled): dormant until it changes.
    Ineligible,
}

/// The scheduler's memory across ticks, matching the refs in shipping's
/// `ProScheduledMeetingAutoStart`.
#[derive(Default)]
pub struct Scheduler {
    meetings: Vec<Meeting>,
    fired: HashSet<Arc<str>>,
    revisions: HashMap<Arc<str>, u64>,
    ineligible: HashMap<Arc<str>, u64>,
    /// First time each meeting app was seen on the microphone; any stop removes it.
    holds: HashMap<String, i64>,
    last_recording_ended: Option<i64>,
    starting: Option<(Arc<str>, u64)>,
}

impl Scheduler {
    pub fn starting(&self) -> bool {
        self.starting.is_some()
    }

    /// New calendar rows; a changed event gets a new revision and becomes eligible again.
    pub fn replace(&mut self, meetings: Vec<Meeting>) {
        let before: HashMap<&Arc<str>, &Meeting> = self
            .meetings
            .iter()
            .map(|meeting| (&meeting.id, meeting))
            .collect();
        let after: HashMap<&Arc<str>, &Meeting> = meetings
            .iter()
            .map(|meeting| (&meeting.id, meeting))
            .collect();
        let changed: Vec<Arc<str>> = before
            .keys()
            .chain(after.keys())
            .filter(|id| before.get(*id) != after.get(*id))
            .map(|id| (*id).clone())
            .collect();
        for id in changed {
            *self.revisions.entry(id.clone()).or_default() += 1;
            self.ineligible.remove(&id);
        }
        self.meetings = meetings;
    }

    pub fn recording_ended(&mut self, now_ms: i64) {
        self.last_recording_ended = Some(now_ms);
    }

    /// Whether the microphone needs watching: near or during a meeting, or while a hold lasts.
    pub fn watches_mic(&self, enabled: bool, now_ms: i64) -> bool {
        enabled
            && (!self.holds.is_empty()
                || self.meetings.iter().any(|meeting| {
                    let Some(start) = meeting.start_ms() else {
                        return false;
                    };
                    let during = meeting
                        .end_ms()
                        .is_some_and(|end| now_ms >= start && now_ms < end);
                    during
                        || (now_ms >= start - MIC_HOLD_LOOKAHEAD_MS && now_ms <= start + GRACE_MS)
                }))
    }

    pub fn clear_holds(&mut self) {
        self.holds.clear();
    }

    /// Shipping's `trackMicHolds`.
    pub fn observe_mic(&mut self, apps: &[MicApp], now_ms: i64) {
        let on_mic: HashSet<&str> = apps
            .iter()
            .filter(|app| super::apps::is_meeting_capable(app))
            .map(|app| app.id.as_str())
            .collect();
        self.holds.retain(|id, _| on_mic.contains(id.as_str()));
        for id in on_mic {
            self.holds.entry(id.to_owned()).or_insert(now_ms);
        }
    }

    pub fn mic_stopped(&mut self, apps: &[MicApp]) {
        for app in apps {
            self.holds.remove(&app.id);
        }
    }

    /// Shipping's `isPreviousCallOnMic`: an app took the microphone during an earlier meeting
    /// and has held it since, so starting now would file the rest of that call here.
    fn previous_call_on_mic(&self, meeting: &Meeting) -> bool {
        let Some(start) = meeting.start_ms() else {
            return false;
        };
        self.holds.values().any(|&held_since| {
            held_since <= start - PREVIOUS_CALL_MIN_HOLD_MS
                && self.meetings.iter().any(|other| {
                    let (Some(other_start), Some(other_end)) = (other.start_ms(), other.end_ms())
                    else {
                        return false;
                    };
                    other.id != meeting.id
                        && other_start < start
                        && (other_start <= start - PREVIOUS_CALL_MIN_HOLD_MS
                            || self
                                .last_recording_ended
                                .is_some_and(|ended| ended > other_start))
                        && other_start - EARLY_JOIN_MS <= held_since
                        && held_since < other_end
                })
        })
    }

    fn next_start(&self, enabled: bool, now_ms: i64) -> Step {
        if !enabled {
            return Step::Idle;
        }
        self.meetings
            .iter()
            .filter(|meeting| !self.fired.contains(&meeting.id))
            .filter_map(Meeting::start_ms)
            .filter(|start| *start > now_ms)
            .min()
            .map_or(Step::Idle, |start| {
                Step::Wait(Duration::from_millis((start - now_ms).max(1) as u64))
            })
    }

    /// Shipping's `tick`: what to do now.
    pub fn tick(&mut self, enabled: bool, live: Live, now_ms: i64) -> Step {
        if self.starting.is_some() {
            return Step::Idle;
        }
        if !enabled {
            return Step::Idle;
        }
        let due: Vec<Meeting> = select_due(&self.meetings, now_ms, &self.fired)
            .into_iter()
            .filter(|meeting| self.ineligible.get(&meeting.id) != self.revisions.get(&meeting.id))
            .collect();
        match live {
            // An overlapping meeting must not start after the active recording ends.
            Live::Recording => {
                self.fired
                    .extend(due.iter().map(|meeting| meeting.id.clone()));
                return self.next_start(enabled, now_ms);
            }
            Live::Busy if !due.is_empty() => return Step::Wait(RETRY),
            Live::Busy => return self.next_start(enabled, now_ms),
            Live::Idle => {}
        }
        let Some(next) = due.first() else {
            return self.next_start(enabled, now_ms);
        };
        if self.previous_call_on_mic(next) {
            return Step::Wait(RETRY);
        }
        let revision = self.revisions.get(&next.id).copied().unwrap_or_default();
        self.starting = Some((next.id.clone(), revision));
        Step::Start(next.clone())
    }

    /// Records how a start ended and says when to tick again.
    pub fn finished(&mut self, outcome: Outcome, now_ms: i64) -> Step {
        let Some((id, revision)) = self.starting.take() else {
            return Step::Idle;
        };
        match outcome {
            Outcome::Blocked => Step::Wait(RETRY),
            Outcome::Ineligible => {
                // A newer ineligible event must not hide an older overlapping one for the
                // whole grace window, so the next meeting gets a chance right away.
                let current = self.revisions.get(&id).copied().unwrap_or_default();
                if current == revision {
                    self.ineligible.insert(id, current);
                }
                Step::Wait(Duration::from_millis(1))
            }
            Outcome::Ignored => {
                self.fired.insert(id);
                Step::Wait(Duration::ZERO)
            }
            Outcome::Started => {
                // Anything overlapping the started meeting is one the user walked out of.
                let due = select_due(&self.meetings, now_ms, &self.fired);
                self.fired.insert(id);
                self.fired.extend(due.into_iter().map(|meeting| meeting.id));
                Step::Wait(Duration::ZERO)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_763_373_600_000; // 2025-11-17T10:00:00Z

    fn iso(ms: i64) -> Arc<str> {
        DateTime::from_timestamp_millis(ms)
            .unwrap()
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
            .into()
    }

    fn meeting(id: &str, start: i64, end: i64, observed: i64) -> Meeting {
        Meeting {
            id: id.into(),
            started_at: iso(start),
            ended_at: iso(end),
            meeting_link: "https://meet.example/abc".into(),
            tracking_id: id.into(),
            series_id: "".into(),
            attendance_json: Some(
                format!(
                    r#"{{"version":1,"self_status":"accepted","roster_status":"complete","others":{{"accepted":1,"tentative":0,"pending":0,"declined":0,"unknown":0}},"observed_at":"{}"}}"#,
                    iso(observed)
                )
                .into(),
            ),
            ignored: false,
        }
    }

    const MIN: i64 = 60_000;

    #[test]
    fn waits_for_the_start_then_starts_the_newest_due_meeting_once() {
        let mut scheduler = Scheduler::default();
        scheduler.replace(vec![
            meeting("earlier", T0 - 3 * MIN, T0 + 30 * MIN, T0),
            meeting("standup", T0 + 2 * MIN, T0 + 20 * MIN, T0 + 2 * MIN),
        ]);
        // The earlier meeting is still inside its grace window.
        let Step::Start(first) = scheduler.tick(true, Live::Idle, T0) else {
            panic!("the due meeting should start");
        };
        assert_eq!(&*first.id, "earlier");
        assert_eq!(
            scheduler.finished(Outcome::Started, T0),
            Step::Wait(Duration::ZERO)
        );
        assert_eq!(
            scheduler.tick(true, Live::Recording, T0),
            Step::Wait(Duration::from_millis(2 * MIN as u64))
        );
        // Recording when the next meeting begins claims it, so it never starts afterwards.
        assert_eq!(
            scheduler.tick(true, Live::Recording, T0 + 2 * MIN),
            Step::Idle
        );
        assert_eq!(scheduler.tick(true, Live::Idle, T0 + 3 * MIN), Step::Idle);
    }

    #[test]
    fn missed_meetings_ignored_events_and_switched_off_auto_start_never_record() {
        let mut scheduler = Scheduler::default();
        scheduler.replace(vec![meeting("late", T0 - 6 * MIN, T0 + 30 * MIN, T0)]);
        assert_eq!(scheduler.tick(true, Live::Idle, T0), Step::Idle);
        scheduler.replace(vec![meeting("now", T0, T0 + 30 * MIN, T0)]);
        assert_eq!(scheduler.tick(false, Live::Idle, T0), Step::Idle);
        let Step::Start(_) = scheduler.tick(true, Live::Idle, T0) else {
            panic!("an enabled tick should start");
        };
        assert_eq!(
            scheduler.finished(Outcome::Ignored, T0),
            Step::Wait(Duration::ZERO)
        );
        assert_eq!(scheduler.tick(true, Live::Idle, T0 + MIN), Step::Idle);
    }

    #[test]
    fn busy_capture_and_blocked_starts_retry_while_ineligible_waits_for_a_calendar_change() {
        let mut scheduler = Scheduler::default();
        let now = meeting("now", T0, T0 + 30 * MIN, T0);
        scheduler.replace(vec![now.clone()]);
        assert_eq!(scheduler.tick(true, Live::Busy, T0), Step::Wait(RETRY));
        scheduler.tick(true, Live::Idle, T0);
        assert_eq!(scheduler.finished(Outcome::Blocked, T0), Step::Wait(RETRY));
        scheduler.tick(true, Live::Idle, T0);
        scheduler.finished(Outcome::Ineligible, T0);
        assert_eq!(scheduler.tick(true, Live::Idle, T0 + 1000), Step::Idle);
        let mut moved = now;
        moved.attendance_json = meeting("now", T0, T0, T0 + 1000).attendance_json;
        scheduler.replace(vec![moved]);
        assert!(matches!(
            scheduler.tick(true, Live::Idle, T0 + 1000),
            Step::Start(_)
        ));
    }

    #[test]
    fn a_call_held_since_an_earlier_meeting_defers_the_next_one_until_it_ends() {
        let mut scheduler = Scheduler::default();
        scheduler.replace(vec![
            meeting("planning", T0 - 60 * MIN, T0 + 5 * MIN, T0),
            meeting("review", T0, T0 + 30 * MIN, T0),
        ]);
        let zoom = MicApp::new("us.zoom.xos", "zoom.us");
        assert!(scheduler.watches_mic(true, T0 - 30 * MIN));
        scheduler.observe_mic(
            &[zoom.clone(), MicApp::new("com.example.tool", "Tool")],
            T0 - 58 * MIN,
        );
        assert_eq!(scheduler.tick(true, Live::Idle, T0), Step::Wait(RETRY));
        scheduler.mic_stopped(&[zoom]);
        assert!(matches!(
            scheduler.tick(true, Live::Idle, T0),
            Step::Start(_)
        ));
    }
}
