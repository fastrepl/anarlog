use std::collections::HashSet;

use anlg_db_app::{
    PILOT_METADATA_LIMIT, count_pilot_pending_sessions, list_pilot_legacy_session_candidates,
    list_pilot_setting_rows,
};
use serde::{Deserialize, Serialize};

use crate::{Error, Result, output};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CaptureUsage {
    version: u32,
    session_id: String,
    transcript_id: String,
    started_at_ms: i64,
    stopped_at_ms: i64,
    requested_live_transcription: bool,
    live_transcription_active_at_stop: bool,
}

#[derive(Serialize)]
pub struct CaptureInterval {
    started_at_ms: i64,
    ended_at_ms: i64,
    requested_live_transcription: bool,
    live_transcription_active_at_stop: bool,
}

#[derive(Default, Serialize)]
pub struct Coverage {
    measured_captures: usize,
    unmeasured_stops: usize,
    invalid_metadata_records: usize,
    pending_sessions: i64,
    legacy_sessions_without_measurement: usize,
}

#[derive(Serialize)]
pub struct PilotUsage {
    from_ms: i64,
    until_ms: i64,
    measurement_source: &'static str,
    intervals: Vec<CaptureInterval>,
    coverage: Coverage,
}

fn failure() -> Error {
    Error::operation("read pilot capture usage", "capture metadata unavailable")
}

pub async fn read(db: &anlg_db_core::Db, from_ms: i64, until_ms: i64) -> Result<PilotUsage> {
    if from_ms < 0 || until_ms <= from_ms || until_ms - from_ms > 366 * 86_400_000 {
        return Err(Error::operation(
            "read pilot capture usage",
            "invalid UTC window (maximum 366 days)",
        ));
    }
    let mut tx = db.pool().begin().await.map_err(|_| failure())?;
    let settings = list_pilot_setting_rows(&mut tx)
        .await
        .map_err(|_| failure())?;
    let candidates = list_pilot_legacy_session_candidates(&mut tx, from_ms, until_ms)
        .await
        .map_err(|_| failure())?;
    if settings.len() > PILOT_METADATA_LIMIT || candidates.len() > PILOT_METADATA_LIMIT {
        return Err(Error::operation(
            "read pilot capture usage",
            "capture metadata limit exceeded",
        ));
    }
    let mut coverage = Coverage {
        pending_sessions: count_pilot_pending_sessions(&mut tx, until_ms)
            .await
            .map_err(|_| failure())?,
        ..Coverage::default()
    };
    let mut measured_keys = HashSet::new();
    let mut measured_sessions = HashSet::new();
    let mut intervals = Vec::new();
    for row in settings
        .iter()
        .filter(|row| row.id.starts_with("capture_usage:"))
    {
        let value = serde_json::from_str::<CaptureUsage>(&row.value_json)
            .ok()
            .filter(|value| {
                value.version == 1
                    && !value.session_id.is_empty()
                    && value.session_id.len() <= 500
                    && !value.transcript_id.is_empty()
                    && value.transcript_id.len() <= 500
                    && row.id
                        == format!("capture_usage:{}:{}", value.session_id, value.transcript_id)
                    && value.started_at_ms >= 0
                    && value.stopped_at_ms > value.started_at_ms
                    && (!value.live_transcription_active_at_stop
                        || value.requested_live_transcription)
            });
        let Some(value) = value else {
            coverage.invalid_metadata_records += 1;
            continue;
        };
        measured_keys.insert(format!(
            "capture_stop:{}:{}",
            value.session_id, value.transcript_id
        ));
        measured_sessions.insert(value.session_id);
        if value.started_at_ms < until_ms && value.stopped_at_ms > from_ms {
            intervals.push(CaptureInterval {
                started_at_ms: value.started_at_ms,
                ended_at_ms: value.stopped_at_ms,
                requested_live_transcription: value.requested_live_transcription,
                live_transcription_active_at_stop: value.live_transcription_active_at_stop,
            });
        }
    }
    for row in settings
        .iter()
        .filter(|row| row.id.starts_with("capture_stop:"))
    {
        match serde_json::from_str::<i64>(&row.value_json) {
            Ok(stop) if stop > 0 => {
                if stop >= from_ms && stop < until_ms && !measured_keys.contains(&row.id) {
                    coverage.unmeasured_stops += 1;
                }
            }
            _ => coverage.invalid_metadata_records += 1,
        }
    }
    coverage.legacy_sessions_without_measurement = candidates
        .into_iter()
        .filter(|id| !measured_sessions.contains(id))
        .count();
    intervals.sort_by_key(|value| (value.started_at_ms, value.ended_at_ms));
    coverage.measured_captures = intervals.len();
    tx.rollback().await.map_err(|_| failure())?;
    Ok(PilotUsage {
        from_ms,
        until_ms,
        measurement_source: "native_stop_registry",
        intervals,
        coverage,
    })
}

pub async fn run(db: &anlg_db_core::Db, from_ms: i64, until_ms: i64) -> Result<()> {
    output::emit(&output::json(
        "meetings.pilot-usage",
        &read(db, from_ms, until_ms).await?,
        None,
    )?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pilot_usage_reads_capture_clocks_without_exporting_private_meeting_or_account_fields()
    {
        let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        let from = 1_790_812_800_000_i64;
        let until = from + 86_400_000;
        sqlx::query("INSERT INTO sessions(id,title) VALUES('private-meeting-id','Private meeting title'),('legacy-id','Private legacy title')")
            .execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO session_documents(id,session_id,body) VALUES('private-document','private-meeting-id','Private personnel note')")
            .execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO transcripts(id,session_id,source,started_at_ms,words_json) VALUES('legacy-transcript','legacy-id','live_capture',?,'[\"Private transcript\"]')")
            .bind(from + 1000).execute(db.pool()).await.unwrap();
        for (id, value) in [
            ("capture_usage:private-meeting-id:first", serde_json::json!({"version":1,"sessionId":"private-meeting-id","transcriptId":"first","startedAtMs":from-60000,"stoppedAtMs":from+60000,"requestedLiveTranscription":true,"liveTranscriptionActiveAtStop":false}).to_string()),
            ("capture_stop:private-meeting-id:first", (from+60000).to_string()),
            ("capture_stop:missing:second", (from+120000).to_string()),
            ("capture_usage:malformed:third", "{}".into()),
            ("provider-secret", "\"Private provider key\"".into()),
            ("capture_lifecycle_pending:private-pending-id", serde_json::json!({"startedAt":from,"memo":"Private recovery note"}).to_string()),
            ("capture_lifecycle_pending:malformed-private-pending-id", "malformed Private recovery note".into()),
        ] {
            sqlx::query("INSERT INTO app_settings(id,value_json) VALUES(?,?)").bind(id).bind(value).execute(db.pool()).await.unwrap();
        }
        let value = read(&db, from, until).await.unwrap();
        let json = serde_json::to_value(value).unwrap();
        assert_eq!(
            json["coverage"],
            serde_json::json!({"measured_captures":1,"unmeasured_stops":1,"invalid_metadata_records":1,"pending_sessions":2,"legacy_sessions_without_measurement":1})
        );
        assert_eq!(json["intervals"][0]["started_at_ms"], from - 60000);
        let text = json.to_string();
        assert!(
            !text.contains("Private")
                && !text.contains("private-meeting-id")
                && !text.contains("legacy-id")
                && !text.contains("private-pending-id")
        );
        assert!(read(&db, until, from).await.is_err());
    }
}
