//! Shipping's attendance snapshot (`services/calendar/attendance.ts`) and the decision whether
//! a scheduled meeting may start automatically (`stt/scheduled-meeting-attendance.ts`).

use anlg_calendar_interface::EventAttendance;
use chrono::{DateTime, SecondsFormat, Utc};
use serde::Deserialize;

pub const MAX_AGE_MS: i64 = 3 * 60_000;
pub const FUTURE_TOLERANCE_MS: i64 = 30_000;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum SelfStatus {
    Organizer,
    Accepted,
    Tentative,
    Pending,
    Declined,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum RosterStatus {
    Complete,
    Incomplete,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct Others {
    accepted: u32,
    tentative: u32,
    pending: u32,
    declined: u32,
    unknown: u32,
}

#[derive(Clone, Debug, Deserialize)]
struct Snapshot {
    version: u8,
    self_status: SelfStatus,
    roster_status: RosterStatus,
    others: Others,
    observed_at: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Block {
    MissingAttendance,
    InvalidAttendance,
    StaleAttendance,
    FutureAttendance,
    SelfDeclined,
    SelfTentative,
    SelfPending,
    SelfUnknown,
    AllOthersDeclined,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    Eligible,
    Blocked(Vec<Block>),
}

impl Decision {
    pub fn eligible(&self) -> bool {
        matches!(self, Self::Eligible)
    }
}

/// The JSON stored in `events.attendance_json` for an event fetched at `observed_at`.
pub fn snapshot_json(attendance: &EventAttendance, observed_at: DateTime<Utc>) -> Option<String> {
    let mut value = serde_json::to_value(attendance).ok()?;
    let object = value.as_object_mut()?;
    object.insert("version".into(), 1.into());
    object.insert(
        "observed_at".into(),
        observed_at
            .to_rfc3339_opts(SecondsFormat::Millis, true)
            .into(),
    );
    Some(value.to_string())
}

/// Shipping's `decideAutomaticMeetingAttendance`: only a fresh snapshot where the user
/// organizes or accepted, and not everyone else declined, allows an automatic start.
pub fn decide(attendance_json: Option<&str>, now_ms: i64) -> Decision {
    let raw = attendance_json.filter(|json| !json.is_empty());
    let Some(snapshot) = raw
        .and_then(|json| serde_json::from_str::<Snapshot>(json).ok())
        .filter(|snapshot| snapshot.version == 1)
    else {
        return Decision::Blocked(vec![if raw.is_some() {
            Block::InvalidAttendance
        } else {
            Block::MissingAttendance
        }]);
    };
    let Some(observed_ms) = DateTime::parse_from_rfc3339(&snapshot.observed_at)
        .ok()
        .map(|time| time.timestamp_millis())
    else {
        return Decision::Blocked(vec![Block::InvalidAttendance]);
    };
    if observed_ms - now_ms > FUTURE_TOLERANCE_MS {
        return Decision::Blocked(vec![Block::FutureAttendance]);
    }
    if now_ms - observed_ms > MAX_AGE_MS {
        return Decision::Blocked(vec![Block::StaleAttendance]);
    }
    let mut blocks = Vec::new();
    match snapshot.self_status {
        SelfStatus::Organizer | SelfStatus::Accepted => {}
        SelfStatus::Declined => blocks.push(Block::SelfDeclined),
        SelfStatus::Tentative => blocks.push(Block::SelfTentative),
        SelfStatus::Pending => blocks.push(Block::SelfPending),
        SelfStatus::Unknown => blocks.push(Block::SelfUnknown),
    }
    let others = snapshot.others;
    let total =
        others.accepted + others.tentative + others.pending + others.declined + others.unknown;
    if snapshot.roster_status == RosterStatus::Complete && total > 0 && others.declined == total {
        blocks.push(Block::AllOthersDeclined);
    }
    if blocks.is_empty() {
        Decision::Eligible
    } else {
        Decision::Blocked(blocks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(status: &str, roster: &str, declined: u32, accepted: u32, at: &str) -> String {
        format!(
            r#"{{"version":1,"self_status":"{status}","roster_status":"{roster}","others":{{"accepted":{accepted},"tentative":0,"pending":0,"declined":{declined},"unknown":0}},"observed_at":"{at}"}}"#
        )
    }

    #[test]
    fn only_fresh_accepted_or_organized_meetings_with_someone_attending_start_automatically() {
        let now = DateTime::parse_from_rfc3339("2026-11-17T10:00:00Z")
            .unwrap()
            .timestamp_millis();
        let fresh = "2026-11-17T09:58:30.000Z";
        assert!(decide(Some(&snapshot("accepted", "complete", 0, 2, fresh)), now).eligible());
        assert!(decide(Some(&snapshot("organizer", "incomplete", 3, 0, fresh)), now).eligible());
        assert_eq!(
            decide(Some(&snapshot("organizer", "complete", 3, 0, fresh)), now),
            Decision::Blocked(vec![Block::AllOthersDeclined])
        );
        assert_eq!(
            decide(Some(&snapshot("tentative", "complete", 0, 1, fresh)), now),
            Decision::Blocked(vec![Block::SelfTentative])
        );
        assert_eq!(
            decide(
                Some(&snapshot(
                    "accepted",
                    "complete",
                    0,
                    1,
                    "2026-11-17T09:56:59Z"
                )),
                now
            ),
            Decision::Blocked(vec![Block::StaleAttendance])
        );
        assert_eq!(
            decide(
                Some(&snapshot(
                    "accepted",
                    "complete",
                    0,
                    1,
                    "2026-11-17T10:00:31Z"
                )),
                now
            ),
            Decision::Blocked(vec![Block::FutureAttendance])
        );
        assert_eq!(
            decide(None, now),
            Decision::Blocked(vec![Block::MissingAttendance])
        );
        assert_eq!(
            decide(Some("{\"version\":2}"), now),
            Decision::Blocked(vec![Block::InvalidAttendance])
        );
    }
}
