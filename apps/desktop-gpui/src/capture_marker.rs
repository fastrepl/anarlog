//! `stt/capture-lifecycle-storage.ts`: the durable `capture_lifecycle_pending:`
//! marker a capture writes so a crash mid-recording or mid-finalization can
//! be recovered on the next launch, by either shell.

use serde::{Deserialize, Serialize};

pub const SETTING_PREFIX: &str = "capture_lifecycle_pending:";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Capturing,
    Finalizing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummaryMode {
    Regenerate,
    IfEmpty,
}

/// `CaptureLifecycleMarker`, serialised with the frontend's field names and
/// key order so both shells read each other's markers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Marker {
    pub version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<Phase>,
    pub session_id: String,
    pub transcript_id: String,
    pub started_at: i64,
    pub created_at: String,
    pub audio_offset_ms: i64,
    pub preserve_existing_transcript: bool,
    /// A scheduled auto-start (`startedAutomatically`); the shell only
    /// records manual captures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub automatic: Option<bool>,
    /// Whether audio existed before the capture (always kept for manual ones).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preserve_existing_audio: Option<bool>,
    /// The session title when the capture started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_title: Option<String>,
    pub owner_user_id: String,
    pub memo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_mode: Option<SummaryMode>,
    /// The summary already ran on the live text; the batch repair regenerates it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub refresh_summary_after_repair: bool,
}

pub fn setting_id(session_id: &str) -> String {
    format!("{SETTING_PREFIX}{session_id}")
}

/// `parseCaptureLifecycleMarker`: version 1 for this session with every
/// required field; unknown optional values are dropped rather than rejected.
pub fn parse(value: &str, session_id: &str) -> Option<Marker> {
    if session_id.is_empty() {
        return None;
    }
    let raw: serde_json::Value = serde_json::from_str(value).ok()?;
    if raw.get("version")?.as_i64()? != 1 || raw.get("sessionId")?.as_str()? != session_id {
        return None;
    }
    let transcript_id = raw.get("transcriptId")?.as_str()?.to_string();
    if transcript_id.is_empty() {
        return None;
    }
    let started_at = finite_i64(raw.get("startedAt")?)?;
    let created_at = raw.get("createdAt")?.as_str()?.to_string();
    let audio_offset_ms = finite_i64(raw.get("audioOffsetMs")?)?.max(0);
    let preserve_existing_transcript = raw.get("preserveExistingTranscript")?.as_bool()?;
    let owner_user_id = raw.get("ownerUserId")?.as_str()?.to_string();
    let memo = raw.get("memo")?.as_str()?.to_string();
    let optional_string = |key: &str| raw.get(key).and_then(|v| v.as_str()).map(str::to_string);
    Some(Marker {
        version: 1,
        phase: match raw.get("phase").and_then(|v| v.as_str()) {
            Some("capturing") => Some(Phase::Capturing),
            Some("finalizing") => Some(Phase::Finalizing),
            _ => None,
        },
        session_id: session_id.to_string(),
        transcript_id,
        started_at,
        created_at,
        audio_offset_ms,
        preserve_existing_transcript,
        automatic: raw.get("automatic").and_then(|v| v.as_bool()),
        preserve_existing_audio: raw.get("preserveExistingAudio").and_then(|v| v.as_bool()),
        initial_title: optional_string("initialTitle"),
        owner_user_id,
        memo,
        provider: optional_string("provider"),
        model: optional_string("model"),
        summary_mode: match raw.get("summaryMode").and_then(|v| v.as_str()) {
            Some("regenerate") => Some(SummaryMode::Regenerate),
            Some("if_empty") => Some(SummaryMode::IfEmpty),
            _ => None,
        },
        refresh_summary_after_repair: raw
            .get("refreshSummaryAfterRepair")
            .and_then(|v| v.as_bool())
            == Some(true),
    })
}

fn finite_i64(value: &serde_json::Value) -> Option<i64> {
    let number = value.as_f64()?;
    number.is_finite().then_some(number as i64)
}

/// `saveCaptureLifecycleMarker`: upsert, but only over a marker for the same
/// transcript (another capture's marker is never overwritten).
pub const SAVE_SQL: &str = "
    INSERT INTO app_settings (id, value_json, updated_at)
    VALUES (?, ?, ?)
    ON CONFLICT(id) DO UPDATE SET
      value_json = excluded.value_json,
      updated_at = excluded.updated_at
    WHERE json_valid(app_settings.value_json)
      AND json_extract(app_settings.value_json, '$.transcriptId')
        = json_extract(excluded.value_json, '$.transcriptId')
";

/// `clearCaptureLifecycleMarker`
pub const CLEAR_SQL: &str = "
    DELETE FROM app_settings
    WHERE id = ?
      AND json_valid(value_json)
      AND json_extract(
        CASE WHEN json_valid(value_json) THEN value_json ELSE '{}' END,
        '$.transcriptId'
      ) = ?
";

/// `loadCaptureLifecycleMarkers`
pub const LOAD_ALL_SQL: &str = "
    SELECT id, value_json
    FROM app_settings
    WHERE id GLOB ?
    ORDER BY updated_at, id
";

#[cfg(test)]
mod tests {
    use super::*;

    fn marker() -> Marker {
        Marker {
            version: 1,
            phase: Some(Phase::Capturing),
            session_id: "s1".into(),
            transcript_id: "t1".into(),
            started_at: 1_700_000_000_000,
            created_at: "2026-09-07T09:00:00.000Z".into(),
            audio_offset_ms: 0,
            preserve_existing_transcript: false,
            automatic: Some(false),
            preserve_existing_audio: Some(true),
            initial_title: Some("Standup".into()),
            owner_user_id: "u1".into(),
            memo: String::new(),
            provider: Some("deepgram".into()),
            model: Some("nova-3".into()),
            summary_mode: None,
            refresh_summary_after_repair: false,
        }
    }

    #[test]
    fn serialises_with_the_frontend_field_names_and_order() {
        let json = serde_json::to_string(&marker()).unwrap();
        assert_eq!(
            json,
            r#"{"version":1,"phase":"capturing","sessionId":"s1","transcriptId":"t1","startedAt":1700000000000,"createdAt":"2026-09-07T09:00:00.000Z","audioOffsetMs":0,"preserveExistingTranscript":false,"automatic":false,"preserveExistingAudio":true,"initialTitle":"Standup","ownerUserId":"u1","memo":"","provider":"deepgram","model":"nova-3"}"#
        );
        let mut finalizing = marker();
        finalizing.phase = Some(Phase::Finalizing);
        finalizing.summary_mode = Some(SummaryMode::IfEmpty);
        let json = serde_json::to_string(&finalizing).unwrap();
        assert!(json.contains(r#""phase":"finalizing""#));
        assert!(json.ends_with(r#""summaryMode":"if_empty"}"#));
        // `refreshSummaryAfterRepair` is written only when set, after the mode.
        let mut refreshing = marker();
        refreshing.refresh_summary_after_repair = true;
        let json = serde_json::to_string(&refreshing).unwrap();
        assert!(json.ends_with(r#""model":"nova-3","refreshSummaryAfterRepair":true}"#));
        assert_eq!(parse(&json, "s1"), Some(refreshing));
        assert!(
            !parse(&json.replace("true}", "\"yes\"}"), "s1")
                .unwrap()
                .refresh_summary_after_repair
        );
    }

    #[test]
    fn parses_like_parse_capture_lifecycle_marker() {
        let json = serde_json::to_string(&marker()).unwrap();
        assert_eq!(parse(&json, "s1"), Some(marker()));
        // Another session's marker, a bad version, or a missing field is rejected.
        assert_eq!(parse(&json, "s2"), None);
        assert_eq!(
            parse(&json.replace(r#""version":1"#, r#""version":2"#), "s1"),
            None
        );
        assert_eq!(parse(&json.replace(r#""memo":"","#, ""), "s1"), None);
        // Unknown optional values are dropped, negative offsets clamp.
        let odd = json
            .replace(r#""phase":"capturing""#, r#""phase":"weird""#)
            .replace(r#""audioOffsetMs":0"#, r#""audioOffsetMs":-5"#);
        let parsed = parse(&odd, "s1").unwrap();
        assert_eq!(parsed.phase, None);
        assert_eq!(parsed.audio_offset_ms, 0);
        assert_eq!(parse("not json", "s1"), None);
    }
}
