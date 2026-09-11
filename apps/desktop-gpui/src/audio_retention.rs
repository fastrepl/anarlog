//! `services/audio-retention{,-policy}.ts`: the `audio_retention` setting,
//! when a session's audio has expired, and the recordings the retention tick
//! and the capture lifecycle delete.

use std::time::Duration;

/// `AUDIO_RETENTION_INTERVAL`
pub const INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    None,
    OneDay,
    ThreeDays,
    OneWeek,
    OneMonth,
    Forever,
}

impl Policy {
    /// `AUDIO_RETENTION_DURATION_MS`
    fn duration_ms(self) -> Option<i64> {
        const DAY: i64 = 24 * 60 * 60 * 1000;
        match self {
            Policy::None => Some(0),
            Policy::OneDay => Some(DAY),
            Policy::ThreeDays => Some(3 * DAY),
            Policy::OneWeek => Some(7 * DAY),
            Policy::OneMonth => Some(30 * DAY),
            Policy::Forever => None,
        }
    }
}

/// `normalizeAudioRetention(value)`: the known names, `false` → `none`,
/// `true` → `forever`, anything else `forever`.
pub fn normalize(value: Option<&serde_json::Value>) -> Policy {
    match value {
        Some(serde_json::Value::String(name)) => match name.as_str() {
            "none" => Policy::None,
            "oneDay" => Policy::OneDay,
            "threeDays" => Policy::ThreeDays,
            "oneWeek" => Policy::OneWeek,
            "oneMonth" => Policy::OneMonth,
            "forever" => Policy::Forever,
            _ => Policy::Forever,
        },
        Some(serde_json::Value::Bool(false)) => Policy::None,
        _ => Policy::Forever,
    }
}

/// `sessionAudioExpired(createdAt, policy, nowMs)`
pub fn session_audio_expired(created_at: Option<&str>, policy: Policy, now_ms: i64) -> bool {
    let Some(duration_ms) = policy.duration_ms() else {
        return false;
    };
    if policy == Policy::None {
        return true;
    }
    let Some(created_at_ms) = created_at
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.timestamp_millis())
    else {
        return false;
    };
    now_ms >= created_at_ms + duration_ms
}

/// A session row of `cleanupExpiredAudio`'s query.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RetentionRow {
    pub id: String,
    pub created_at: String,
    pub has_words: bool,
    pub transcript_processing: bool,
}

pub const RETENTION_ROWS_SQL: &str = "
    SELECT
      session.id,
      session.created_at,
      EXISTS(
        SELECT 1
        FROM transcripts AS transcript
        WHERE transcript.session_id = session.id
          AND transcript.deleted_at IS NULL
          AND json_valid(transcript.words_json)
          AND json_array_length(transcript.words_json) > 0
      ) AS has_words,
      EXISTS(
        SELECT 1
        FROM session_attachments AS audio
        WHERE audio.session_id = session.id
          AND audio.source_type = 'session_audio'
          AND audio.source_id = 'primary'
          AND audio.deleted_at IS NULL
          AND json_valid(audio.metadata_json)
          AND json_extract(audio.metadata_json, '$.transcript_status') = 'processing'
      ) AS transcript_processing
    FROM sessions AS session
    WHERE session.deleted_at IS NULL
    ORDER BY session.created_at, session.id
";

/// `cleanupLogicallyDeletedAudio`'s query: tombstoned primary audio whose
/// file has not been marked absent yet.
pub const LOGICALLY_DELETED_AUDIO_SQL: &str = "
    SELECT DISTINCT attachment.session_id
    FROM session_attachments AS attachment
    LEFT JOIN attachment_local_state AS local
      ON local.attachment_id = attachment.id
    WHERE attachment.source_type = 'session_audio'
      AND attachment.source_id = 'primary'
      AND attachment.deleted_at IS NOT NULL
      AND COALESCE(local.availability, 'present') != 'absent'
    ORDER BY attachment.session_id
";

/// `cleanupExpiredAudio`'s per-session decision, after the idle check.
pub fn should_delete_expired(row: &RetentionRow, policy: Policy, now_ms: i64) -> bool {
    if row.transcript_processing {
        return false;
    }
    if policy == Policy::None && !row.has_words {
        return false;
    }
    session_audio_expired(Some(&row.created_at), policy, now_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_like_the_settings_helper() {
        assert_eq!(
            normalize(Some(&serde_json::json!("oneWeek"))),
            Policy::OneWeek
        );
        assert_eq!(normalize(Some(&serde_json::json!(false))), Policy::None);
        assert_eq!(normalize(Some(&serde_json::json!(true))), Policy::Forever);
        assert_eq!(
            normalize(Some(&serde_json::json!("bogus"))),
            Policy::Forever
        );
        assert_eq!(normalize(None), Policy::Forever);
    }

    #[test]
    fn expiry_follows_the_policy_durations() {
        let created = "2026-09-01T00:00:00.000Z";
        let created_ms = 1_788_220_800_000;
        assert!(!session_audio_expired(
            Some(created),
            Policy::Forever,
            i64::MAX
        ));
        assert!(session_audio_expired(None, Policy::None, 0));
        assert!(!session_audio_expired(None, Policy::OneDay, i64::MAX));
        assert!(!session_audio_expired(
            Some("nope"),
            Policy::OneDay,
            i64::MAX
        ));
        let day = 24 * 60 * 60 * 1000;
        assert!(!session_audio_expired(
            Some(created),
            Policy::OneDay,
            created_ms + day - 1
        ));
        assert!(session_audio_expired(
            Some(created),
            Policy::OneDay,
            created_ms + day
        ));
        assert!(session_audio_expired(
            Some(created),
            Policy::OneMonth,
            created_ms + 30 * day
        ));
    }

    #[test]
    fn expired_rows_skip_processing_and_wordless_none_sessions() {
        let row = |has_words, processing| RetentionRow {
            id: "s".into(),
            created_at: "2026-09-01T00:00:00.000Z".into(),
            has_words,
            transcript_processing: processing,
        };
        assert!(!should_delete_expired(
            &row(true, true),
            Policy::None,
            i64::MAX
        ));
        assert!(!should_delete_expired(
            &row(false, false),
            Policy::None,
            i64::MAX
        ));
        assert!(should_delete_expired(&row(true, false), Policy::None, 0));
        // Expiring policies delete wordless audio too once it is old enough.
        assert!(should_delete_expired(
            &row(false, false),
            Policy::OneDay,
            i64::MAX
        ));
        assert!(!should_delete_expired(
            &row(true, false),
            Policy::Forever,
            i64::MAX
        ));
    }
}
