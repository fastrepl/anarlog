//! `settings/stats/badges.ts` + `badge-queries.ts`: the collectible badges,
//! their progress over the activity records, and the collected set persisted
//! in `app_settings` under `personal-badges.v1:<owner>:<id>`.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Datelike, Duration, TimeZone, Utc, Weekday};

use crate::stats::ActivityRecord;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    Signup,
    Onboarding,
    Conversations,
    Weeks,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Badge {
    /// Persisted; stable when names or artwork change.
    pub id: &'static str,
    pub metric: Metric,
    pub target: u64,
}

/// `BADGES`
pub const BADGES: [Badge; 9] = [
    Badge {
        id: "hello",
        metric: Metric::Signup,
        target: 1,
    },
    Badge {
        id: "all-set",
        metric: Metric::Onboarding,
        target: 1,
    },
    Badge {
        id: "first-words",
        metric: Metric::Conversations,
        target: 1,
    },
    Badge {
        id: "good-listener",
        metric: Metric::Conversations,
        target: 10,
    },
    Badge {
        id: "memory-keeper",
        metric: Metric::Conversations,
        target: 50,
    },
    Badge {
        id: "story-collector",
        metric: Metric::Conversations,
        target: 100,
    },
    Badge {
        id: "living-library",
        metric: Metric::Conversations,
        target: 250,
    },
    Badge {
        id: "finding-rhythm",
        metric: Metric::Weeks,
        target: 4,
    },
    Badge {
        id: "familiar-face",
        metric: Metric::Weeks,
        target: 12,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BadgeProgress {
    pub badge: Badge,
    /// `Math.min(metric, target)`
    pub value: u64,
}

impl BadgeProgress {
    pub fn complete(&self) -> bool {
        self.value >= self.badge.target
    }
}

/// `BadgeGallery`'s `details`: the name and description per badge.
pub fn badge_details(id: &str) -> (&'static str, &'static str) {
    match id {
        "hello" => (
            "Hello, Anarlog",
            "Create your Anarlog account. A place for your conversations to call home.",
        ),
        "all-set" => (
            "All Set",
            "Complete onboarding. You're ready for your next conversation.",
        ),
        "first-words" => (
            "First Words",
            "Capture your first conversation. Every collection starts somewhere.",
        ),
        "good-listener" => (
            "Good Listener",
            "Capture 10 conversations. More moments you can return to.",
        ),
        "memory-keeper" => (
            "Memory Keeper",
            "Capture 50 conversations. A growing collection of ideas and decisions.",
        ),
        "story-collector" => (
            "Story Collector",
            "Capture 100 conversations. A hundred stories, saved in your own words.",
        ),
        "living-library" => (
            "Living Library",
            "Capture 250 conversations. Your own library of shared knowledge.",
        ),
        "finding-rhythm" => (
            "Finding Your Rhythm",
            "Capture conversations in 4 different weeks. They don't need to be consecutive.",
        ),
        "familiar-face" => (
            "Familiar Face",
            "Capture conversations in 12 different weeks. A little at a time, at your own pace.",
        ),
        _ => ("", ""),
    }
}

/// date-fns `startOfWeek`
fn start_of_week(date: chrono::NaiveDate, week_starts_on: Weekday) -> chrono::NaiveDate {
    let offset = (7 + date.weekday().num_days_from_sunday() as i64
        - week_starts_on.num_days_from_sunday() as i64)
        % 7;
    date - Duration::days(offset)
}

/// `getBadgeProgress`: deliberate conversations (the welcome demo, invalid
/// and future activity excluded) and the distinct calendar weeks they fall
/// in, plus the account and onboarding state.
pub fn badge_progress<Tz: TimeZone>(
    records: &[ActivityRecord],
    signed_up: bool,
    onboarding_complete: bool,
    now: DateTime<Utc>,
    tz: &Tz,
    week_starts_on: Weekday,
) -> Vec<BadgeProgress> {
    let mut sessions: BTreeSet<&str> = BTreeSet::new();
    let mut weeks: BTreeSet<chrono::NaiveDate> = BTreeSet::new();
    for record in records {
        if record.is_demo != 0 {
            continue;
        }
        let started_at = if record.started_at_ms > 0 {
            Some(record.started_at_ms)
        } else {
            crate::timeline::parse_date(&record.created_at, tz).map(|d| d.timestamp_millis())
        };
        let Some(started_at) = started_at else {
            continue;
        };
        if started_at > now.timestamp_millis() {
            continue;
        }
        let Some(date) = Utc.timestamp_millis_opt(started_at).single() else {
            continue;
        };
        sessions.insert(&record.session_id);
        weeks.insert(start_of_week(
            date.with_timezone(tz).date_naive(),
            week_starts_on,
        ));
    }
    BADGES
        .iter()
        .map(|badge| {
            let metric = match badge.metric {
                Metric::Signup => u64::from(signed_up),
                Metric::Onboarding => u64::from(onboarding_complete),
                Metric::Conversations => sessions.len() as u64,
                Metric::Weeks => weeks.len() as u64,
            };
            BadgeProgress {
                badge: *badge,
                value: metric.min(badge.target),
            }
        })
        .collect()
}

/// `collectionPrefix`
pub fn collection_prefix(owner_id: &str) -> String {
    format!("personal-badges.v1:{owner_id}:")
}

/// `parseCollectedBadges`: the `collectedAt` per known badge from the stored
/// `{"id","collectedAt"}` rows; malformed rows are skipped.
pub fn parse_collected_badges(rows: &[String]) -> BTreeMap<&'static str, String> {
    let mut collected = BTreeMap::new();
    for row in rows {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(row) else {
            continue;
        };
        let Some(object) = value.as_object() else {
            continue;
        };
        let (Some(id), Some(collected_at)) = (
            object.get("id").and_then(|id| id.as_str()),
            object.get("collectedAt").and_then(|at| at.as_str()),
        ) else {
            continue;
        };
        let Some(badge) = BADGES.iter().find(|badge| badge.id == id) else {
            continue;
        };
        if DateTime::parse_from_rfc3339(collected_at).is_err() {
            continue;
        }
        collected.insert(badge.id, collected_at.to_string());
    }
    collected
}

/// `collectBadges`' stored value: `JSON.stringify({ id, collectedAt })`.
pub fn collected_value(id: &str, collected_at: &str) -> String {
    format!(r#"{{"id":"{id}","collectedAt":"{collected_at}"}}"#)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        "2026-09-08T12:00:00Z".parse().unwrap()
    }

    fn record(id: &str, date: &str) -> ActivityRecord {
        ActivityRecord {
            session_id: id.to_string(),
            started_at_ms: date
                .parse::<DateTime<Utc>>()
                .map(|d| d.timestamp_millis())
                .unwrap_or(0),
            created_at: date.to_string(),
            is_demo: 0,
            duration_ms: 60_000,
        }
    }

    fn progress<Tz: TimeZone>(records: &[ActivityRecord], tz: &Tz) -> Vec<BadgeProgress> {
        badge_progress(records, false, false, now(), tz, Weekday::Mon)
    }

    fn complete(progress: &[BadgeProgress]) -> Vec<&'static str> {
        progress
            .iter()
            .filter(|badge| badge.complete())
            .map(|badge| badge.badge.id)
            .collect()
    }

    #[test]
    fn awards_only_deliberate_conversation_milestones_and_deduplicates_resumed_recordings() {
        let mut records: Vec<ActivityRecord> = (0..50)
            .map(|i| record(&i.to_string(), "2026-09-01T12:00:00Z"))
            .collect();
        records.push(record("0", "2026-09-02T12:00:00Z"));
        assert_eq!(
            complete(&progress(&records, &Utc)),
            ["first-words", "good-listener", "memory-keeper"]
        );
    }

    #[test]
    fn excludes_the_welcome_demo_and_invalid_or_future_activity() {
        let mut demo = record("demo", "2026-09-01T12:00:00Z");
        demo.is_demo = 1;
        let records = [
            demo,
            record("invalid", "invalid"),
            record("future", "2027-01-01T12:00:00Z"),
        ];
        assert!(
            progress(&records, &Utc)
                .iter()
                .all(|badge| badge.value == 0)
        );
    }

    #[test]
    fn counts_separate_active_weeks_in_the_calendar_timezone_without_requiring_a_streak() {
        let records = [
            record("a", "2026-07-05T15:00:00Z"),
            record("b", "2026-07-26T15:00:00Z"),
            record("c", "2026-08-23T15:00:00Z"),
            record("d", "2026-08-30T14:59:00Z"),
        ];
        let rhythm = |progress: &[BadgeProgress]| {
            progress
                .iter()
                .find(|badge| badge.badge.id == "finding-rhythm")
                .map(|badge| badge.value)
        };
        assert_eq!(
            rhythm(&progress(&records, &chrono_tz::Asia::Seoul)),
            Some(3)
        );
        assert_eq!(rhythm(&progress(&records, &Utc)), Some(4));
    }

    #[test]
    fn only_awards_the_getting_started_badges_from_confirmed_account_and_onboarding_state() {
        let badges = badge_progress(&[], true, true, now(), &Utc, Weekday::Sun);
        assert_eq!(complete(&badges), ["hello", "all-set"]);
    }

    #[test]
    fn ignores_unknown_or_malformed_stored_badges_while_preserving_valid_collection_dates() {
        let at = "2026-09-08T12:00:00.000Z";
        let rows = vec![
            "invalid".to_string(),
            "null".to_string(),
            collected_value("unknown", at),
            collected_value("all-set", "invalid"),
            collected_value("hello", at),
        ];
        let collected = parse_collected_badges(&rows);
        assert_eq!(collected.len(), 1);
        assert_eq!(collected.get("hello").map(String::as_str), Some(at));
        assert_eq!(
            collected_value("hello", at),
            r#"{"id":"hello","collectedAt":"2026-09-08T12:00:00.000Z"}"#
        );
        assert_eq!(collection_prefix("owner"), "personal-badges.v1:owner:");
    }
}
