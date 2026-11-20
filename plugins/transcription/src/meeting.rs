//! Tauri adapter for the one meeting runtime: every capture the record path starts is registered
//! here, ends here, and only here may its helper staging be released.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anlg_audio::meeting::{CaptureEndReason, CaptureEngine, CaptureIdentity, CaptureInstant};
use anlg_transcript::MeetingSpeakerEvidence;
use anlg_transcription_core::meeting::{
    MeetingAudioRouter, MeetingError, MeetingRuntime, STAGING_MAX_AGE, sweep_staging,
};
use serde_json::Value;
use tauri::Manager;
use tauri_specta::Event;

use crate::{CaptureParams, ListenerPluginExt};

/// Recordings started without a signed-in library still need one consistent owner.
const LOCAL_ACCOUNT: &str = "local";

#[derive(Clone)]
pub struct MeetingCapture {
    pub runtime: Arc<MeetingRuntime>,
    pub router: Arc<MeetingAudioRouter>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum MeetingCaptureEngineKind {
    Legacy,
    NativeMeeting,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct MeetingCaptureCapability {
    pub native_available: bool,
    pub selects_microphone: bool,
    /// The engine a recording started now would use.
    pub engine: MeetingCaptureEngineKind,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum MeetingCaptureAutoStopReason {
    AppClosed,
    DurationLimit,
    AccountChanged,
}

/// A recording Rust stopped on its own (the call's app quit, the maximum length, or sign-out or
/// an account switch). Sent only when that stop ended this capture's recording; a person's own
/// stop and a stop refused for a newer capture send nothing.
#[derive(Debug, Clone, serde::Serialize, specta::Type, tauri_specta::Event)]
pub struct MeetingCaptureAutoStopEvent {
    pub session_id: String,
    pub capture_id: String,
    pub reason: MeetingCaptureAutoStopReason,
}

pub fn account_of(params: &CaptureParams) -> String {
    params
        .live_transcript
        .as_ref()
        .map(|target| target.owner_user_id.clone())
        .or_else(|| params.self_human_id.clone())
        .filter(|account| !account.is_empty())
        .unwrap_or_else(|| LOCAL_ACCOUNT.to_string())
}

pub(crate) fn state<R: tauri::Runtime>(app: &impl Manager<R>) -> Option<MeetingCapture> {
    app.try_state::<MeetingCapture>()
        .map(|state| state.inner().clone())
}

/// Registers a new capture before the recorder opens audio, so the engine choice and every later
/// completion are scoped to it.
pub(crate) fn begin<R: tauri::Runtime>(
    app: &impl Manager<R>,
    params: &CaptureParams,
    running_session_id: Option<&str>,
) -> Result<Option<CaptureIdentity>, crate::Error> {
    let extension_start = params.extension_start.as_deref();
    let Some(state) = state(app) else {
        // Nothing reserved an extension start without the runtime.
        return match extension_start {
            Some(_) => Err(crate::Error::MeetingCapture(
                MeetingError::StaleExtensionStart.to_string(),
            )),
            None => Ok(None),
        };
    };
    let begin = || {
        state
            .runtime
            .begin_recording(&account_of(params), &params.session_id, extension_start)
    };
    match begin() {
        Ok(capture) => Ok(Some(capture)),
        Err(MeetingError::CaptureActive) => {
            // A capture the recorder no longer runs must not block the next recording.
            let stale = state
                .runtime
                .active()
                .filter(|active| running_session_id != Some(active.identity.session_id.as_str()));
            let Some(stale) = stale else {
                return Err(crate::Error::SessionAlreadyRunning);
            };
            state
                .router
                .finish_capture(&stale.identity, CaptureEndReason::CaptureFailed);
            begin()
                .map(Some)
                .map_err(|error| crate::Error::MeetingCapture(error.to_string()))
        }
        Err(error) => Err(crate::Error::MeetingCapture(error.to_string())),
    }
}

pub(crate) fn finish<R: tauri::Runtime>(
    app: &impl Manager<R>,
    capture: &CaptureIdentity,
    reason: CaptureEndReason,
) {
    if let Some(state) = state(app) {
        state.router.finish_capture(capture, reason);
    }
}

/// Ends the capture a recorder lifecycle event belongs to; a late event of an earlier recording
/// never ends a newer capture of the same session.
pub(crate) fn finish_recorded<R: tauri::Runtime>(
    app: &impl Manager<R>,
    capture_id: Option<&str>,
    reason: CaptureEndReason,
) {
    let (Some(state), Some(capture_id)) = (state(app), capture_id) else {
        return;
    };
    let capture = state
        .runtime
        .active()
        .map(|active| active.identity)
        .filter(|identity| identity.capture_id == capture_id);
    if let Some(capture) = capture {
        state.router.finish_capture(&capture, reason);
    }
}

/// Stops a recording the native helper itself ended (the call's app quit, or the maximum length).
/// The runtime has already finished the capture with that reason.
pub(crate) fn stop_on_helper_end(app: &tauri::AppHandle, router: &MeetingAudioRouter) {
    let app = app.clone();
    router.on_helper_end(move |capture, reason| {
        tracing::info!(?reason, "meeting_capture_ended_by_helper");
        let app = app.clone();
        let capture = capture.clone();
        tauri::async_runtime::spawn(async move {
            let reason = match reason {
                CaptureEndReason::AppClosed => Some(MeetingCaptureAutoStopReason::AppClosed),
                CaptureEndReason::DurationLimit => {
                    Some(MeetingCaptureAutoStopReason::DurationLimit)
                }
                _ => None,
            };
            match reason {
                Some(reason) => stop_automatically(&app, &capture, reason).await,
                None => {
                    stop_capture(&app, &capture).await;
                }
            }
        });
    });
}

/// Whether the meeting runtime still holds this capture, so its recording may start.
pub(crate) fn is_current_capture<R: tauri::Runtime>(
    app: &impl Manager<R>,
    capture_id: &str,
) -> bool {
    state(app).is_none_or(|state| {
        state
            .runtime
            .active()
            .is_some_and(|active| active.identity.capture_id == capture_id)
    })
}

/// Stops the recording of exactly this capture through the record path. The recorder refuses
/// once it records another capture, so a late stop never ends the session's next recording.
/// A capture begun whose start hasn't reached the recorder yet ends instead, so that start is
/// refused; asking again catches a start that got in between.
pub async fn stop_capture<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    capture: &CaptureIdentity,
) -> bool {
    let listener = app.listener();
    let mut stopped = listener
        .stop_capture_if_current(capture.capture_id.clone())
        .await;
    if !stopped && is_current_capture(app, &capture.capture_id) {
        finish(app, capture, CaptureEndReason::UserStopped);
        stopped = listener
            .stop_capture_if_current(capture.capture_id.clone())
            .await;
    }
    if stopped {
        crate::stop_recovery_transcription(app, &capture.session_id).await;
    }
    stopped
}

async fn stop_automatically<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    capture: &CaptureIdentity,
    reason: MeetingCaptureAutoStopReason,
) {
    if !stop_capture(app, capture).await {
        return;
    }
    let event = MeetingCaptureAutoStopEvent {
        session_id: capture.session_id.clone(),
        capture_id: capture.capture_id.clone(),
        reason,
    };
    if let Err(error) = event.emit(app) {
        tracing::warn!(%error, "meeting_capture_auto_stop_event_failed");
    }
}

/// The signed-in account changed (`None`: signed out), reported by the main window. Signing out or
/// switching accounts ends the recording of the previous account at once (its pending notice and
/// camera work cancel) and stops it through the record path; signing in keeps a local recording.
#[tauri::command]
#[specta::specta]
pub async fn set_meeting_account<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    account_id: Option<String>,
) -> Result<(), String> {
    let state = state(&app).ok_or("meeting capture is not available")?;
    let account_id = account_id.filter(|account| !account.is_empty());
    if let Some(capture) = state.router.set_account(account_id) {
        tracing::info!("meeting_capture_ended_by_account_change");
        stop_automatically(&app, &capture, MeetingCaptureAutoStopReason::AccountChanged).await;
    }
    Ok(())
}

pub(crate) fn transcription_connected<R: tauri::Runtime>(app: &impl Manager<R>, session_id: &str) {
    let Some(state) = state(app) else {
        return;
    };
    if let Some(capture) = state.runtime.active_for_session(session_id) {
        let _ = state.runtime.transcription_connected(&capture);
    }
}

pub(crate) fn adopt<R: tauri::Runtime>(app: &impl Manager<R>, session_id: &str) {
    if let Some(state) = state(app) {
        state.router.adopt(session_id);
    }
}

/// How often open evidence is saved while recording, so a crash loses at most this much.
const EVIDENCE_SAVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

/// Keeps the capture's speaker evidence in its session, key-scoped beside the interval poller's
/// writes: every few seconds while it records and once, closed, when it ends. Until that closed
/// save lands, a local note of the capture lets the next launch close what a crash left open.
pub(crate) fn spawn_evidence_persistence<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    capture: CaptureIdentity,
) {
    use anlg_audio::meeting::CaptureAuthority;

    let journal = open_evidence_dir(&app);
    if let Some(dir) = &journal
        && let Err(error) = note_open_evidence(dir, &capture)
    {
        tracing::warn!(%error, "meeting_speaker_evidence_journal_failed");
    }
    tauri::async_runtime::spawn(async move {
        let Some(state) = state(&app) else {
            return;
        };
        loop {
            tokio::time::sleep(EVIDENCE_SAVE_INTERVAL).await;
            let current = state.runtime.is_current(&capture);
            let saved = persist_evidence(&app, &state, &capture).await;
            if let Err(error) = &saved {
                tracing::warn!(%error, "meeting_speaker_evidence_save_failed");
            }
            if !current {
                if saved.is_ok()
                    && let Some(dir) = &journal
                {
                    clear_open_evidence(dir, &capture.capture_id);
                }
                return;
            }
        }
    });
}

/// One writer of `speaker_context.meetings` at a time: each save reads the array and writes it
/// back, so concurrent saves of one session would drop each other's capture.
static MEETINGS_WRITE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const READ_MEETINGS: &str = "SELECT owner_user_id, json_extract(metadata_json, '$.speaker_context.meetings') FROM sessions WHERE id = ? AND deleted_at IS NULL";
const WRITE_MEETINGS: &str = "UPDATE sessions SET metadata_json = json_set(metadata_json, '$.speaker_context', json_patch(COALESCE(json_extract(metadata_json, '$.speaker_context'), '{}'), json_object('meetings', json(?)))), updated_at = ? WHERE id = ? AND deleted_at IS NULL";

fn database<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<tauri_plugin_db::ManagedState, String> {
    app.try_state::<tauri_plugin_db::ManagedState>()
        .map(|db| db.inner().clone())
        .ok_or_else(|| "database is not ready yet".to_string())
}

/// The session's owner and stored `meetings`, or `None` when the session is gone.
async fn read_meetings(
    db: &tauri_plugin_db::ManagedState,
    session_id: &str,
) -> Result<Option<(String, Value)>, String> {
    let row: Option<(String, Option<String>)> = sqlx::query_as(READ_MEETINGS)
        .bind(session_id)
        .fetch_optional(db.pool())
        .await
        .map_err(|error| error.to_string())?;
    Ok(row.map(|(owner, meetings)| {
        let meetings = meetings
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or(Value::Null);
        (owner, meetings)
    }))
}

async fn write_meetings(
    db: &tauri_plugin_db::ManagedState,
    session_id: &str,
    meetings: &Value,
) -> Result<(), String> {
    sqlx::query(WRITE_MEETINGS)
        .bind(meetings.to_string())
        .bind(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        .bind(session_id)
        .execute(db.pool())
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) async fn persist_evidence<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: &MeetingCapture,
    capture: &CaptureIdentity,
) -> Result<(), String> {
    let Some(evidence) = state.runtime.evidence(capture) else {
        return Ok(());
    };
    let db = database(app)?;
    let _write = MEETINGS_WRITE.lock().await;
    let _guard = db.synced_write_guard().await;
    let Some((_, stored)) = read_meetings(&db, &capture.session_id).await? else {
        return Ok(());
    };
    let Some(meetings) = anlg_transcript::upsert_meeting_evidence(&stored, &evidence) else {
        return Ok(());
    };
    write_meetings(&db, &capture.session_id, &meetings).await
}

const OPEN_EVIDENCE_DIR: &str = "meeting-evidence-open";
/// How long the database may take to open after launch before recovery waits for the next one.
const RECOVERY_DB_WAIT: std::time::Duration = std::time::Duration::from_secs(300);

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct OpenEvidence {
    capture: CaptureIdentity,
    opened_at_ms: i64,
}

fn open_evidence_dir<R: tauri::Runtime>(app: &impl Manager<R>) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join(OPEN_EVIDENCE_DIR))
}

fn open_evidence_path(dir: &Path, capture_id: &str) -> Option<PathBuf> {
    (!capture_id.is_empty()
        && capture_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-'))
    .then(|| dir.join(format!("{capture_id}.json")))
}

fn note_open_evidence(dir: &Path, capture: &CaptureIdentity) -> std::io::Result<()> {
    let path = open_evidence_path(dir, &capture.capture_id)
        .ok_or_else(|| std::io::Error::other("invalid capture id"))?;
    std::fs::create_dir_all(dir)?;
    let entry = OpenEvidence {
        capture: capture.clone(),
        opened_at_ms: chrono::Utc::now().timestamp_millis(),
    };
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, serde_json::to_vec(&entry)?)?;
    std::fs::rename(temp, path)
}

fn clear_open_evidence(dir: &Path, capture_id: &str) {
    if let Some(path) = open_evidence_path(dir, capture_id)
        && let Err(error) = std::fs::remove_file(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(%error, "meeting_speaker_evidence_journal_clear_failed");
    }
}

fn list_open_evidence(dir: &Path) -> Vec<OpenEvidence> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|entry| std::fs::read(entry.path()).ok())
        .filter_map(|bytes| serde_json::from_slice::<OpenEvidence>(&bytes).ok())
        .collect()
}

/// The latest capture time the stored evidence shows the capture reached. A crashed capture ends
/// there: nothing later was heard, so nothing later is claimed.
fn last_evidence_ms(evidence: &MeetingSpeakerEvidence) -> i64 {
    let gap_ms = |gap: &anlg_transcript::MeetingEvidenceGap| gap.end_ms.unwrap_or(gap.start_ms);
    let tracks = [&evidence.browser, &evidence.native].into_iter().flatten();
    tracks
        .flat_map(|track| {
            std::iter::once(track.last_heard_ms)
                .chain(track.rosters.iter().map(|roster| roster.at_ms))
                .chain(track.cues.iter().map(|cue| cue.at_ms))
                .chain(track.gaps.iter().map(gap_ms))
        })
        .chain(evidence.pauses.iter().map(gap_ms))
        .chain(evidence.provider_restarts_ms.iter().copied())
        .max()
        .unwrap_or(0)
        .max(0)
}

/// The stored `meetings` with exactly this capture's entry closed, if a crash left it open. Other
/// captures are untouched; `None` when there is nothing to close.
fn close_crashed_evidence(stored: &Value, capture_id: &str) -> Option<Value> {
    let entry = stored
        .as_array()?
        .iter()
        .find(|entry| entry.get("capture_id").and_then(Value::as_str) == Some(capture_id))?;
    let mut evidence: MeetingSpeakerEvidence = serde_json::from_value(entry.clone()).ok()?;
    if evidence.ended_at_ms.is_some() {
        return None;
    }
    evidence.close(CaptureInstant::from_millis(
        last_evidence_ms(&evidence) as f64
    )?);
    anlg_transcript::upsert_meeting_evidence(stored, &evidence)
}

#[derive(Debug, PartialEq, Eq)]
enum Recovery {
    /// Closed now, already closed, never saved, or its session is gone: nothing left to do.
    Settled,
    /// The session now belongs to another library owner; leave it until this one returns.
    OtherAccount,
}

async fn recover_open_evidence(
    db: &tauri_plugin_db::ManagedState,
    capture: &CaptureIdentity,
) -> Result<Recovery, String> {
    let _write = MEETINGS_WRITE.lock().await;
    let _guard = db.synced_write_guard().await;
    let Some((owner, stored)) = read_meetings(db, &capture.session_id).await? else {
        return Ok(Recovery::Settled);
    };
    if capture.account_id != LOCAL_ACCOUNT && owner != capture.account_id {
        return Ok(Recovery::OtherAccount);
    }
    if let Some(meetings) = close_crashed_evidence(&stored, &capture.capture_id) {
        write_meetings(db, &capture.session_id, &meetings).await?;
        tracing::info!("meeting_speaker_evidence_closed_after_crash");
    }
    Ok(Recovery::Settled)
}

/// Closes speaker evidence an earlier run left open: it crashed or quit before the closed save.
pub(crate) fn spawn_evidence_recovery(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let Some(dir) = open_evidence_dir(&app) else {
            return;
        };
        let listed = dir.clone();
        let Ok(open) =
            tauri::async_runtime::spawn_blocking(move || list_open_evidence(&listed)).await
        else {
            return;
        };
        if open.is_empty() {
            return;
        }
        let deadline = tokio::time::Instant::now() + RECOVERY_DB_WAIT;
        let db = loop {
            match database(&app) {
                Ok(db) => break db,
                Err(_) if tokio::time::Instant::now() < deadline => {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
                Err(error) => {
                    tracing::warn!(%error, "meeting_speaker_evidence_recovery_skipped");
                    return;
                }
            }
        };
        let active = state(&app)
            .and_then(|state| state.runtime.active())
            .map(|active| active.identity.capture_id);
        let expired_before =
            chrono::Utc::now().timestamp_millis() - STAGING_MAX_AGE.as_millis() as i64;
        for entry in open {
            if active.as_deref() == Some(entry.capture.capture_id.as_str()) {
                continue;
            }
            match recover_open_evidence(&db, &entry.capture).await {
                Ok(Recovery::OtherAccount) if entry.opened_at_ms >= expired_before => {}
                Ok(_) => clear_open_evidence(&dir, &entry.capture.capture_id),
                Err(error) => tracing::warn!(%error, "meeting_speaker_evidence_recovery_failed"),
            }
        }
    });
}

/// Drops helper staging left by earlier runs once its session has no recovery pending.
pub(crate) fn spawn_staging_sweep(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let Some(state) = state(&app) else {
            return;
        };
        let pending = match crate::capture_markers::list_markers(&app).await {
            Ok(markers) => markers
                .into_iter()
                .map(|marker| marker.session_id)
                .collect::<std::collections::HashSet<_>>(),
            // Without the recovery markers nothing can be shown to be adopted.
            Err(error) => {
                tracing::warn!(%error, "meeting_staging_sweep_skipped");
                return;
            }
        };
        let active = state
            .runtime
            .active()
            .map(|active| active.identity.capture_id);
        let root = state.runtime.staging_root().to_path_buf();
        let result = tauri::async_runtime::spawn_blocking(move || {
            sweep_staging(
                &root,
                active.as_deref(),
                |staged| !pending.contains(&staged.capture.session_id),
                STAGING_MAX_AGE,
            )
        })
        .await;
        match result {
            Ok(Ok(removed)) if !removed.is_empty() => {
                tracing::info!(count = removed.len(), "meeting_staging_swept");
            }
            Ok(Err(error)) => tracing::warn!(%error, "meeting_staging_sweep_failed"),
            _ => {}
        }
    });
}

#[tauri::command]
#[specta::specta]
pub async fn get_meeting_capture_capability<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    mic_device: Option<String>,
) -> Result<MeetingCaptureCapability, String> {
    use anlg_audio::meeting::EngineAvailability;

    let state = state(&app).ok_or("meeting capture is not available")?;
    let capability = state.router.capability(mic_device.as_deref());
    let (native_available, selects_microphone) = match capability.availability {
        Some(EngineAvailability::Available { selects_microphone }) => (true, selects_microphone),
        _ => (false, false),
    };
    Ok(MeetingCaptureCapability {
        native_available,
        selects_microphone,
        engine: match capability.engine {
            CaptureEngine::Legacy => MeetingCaptureEngineKind::Legacy,
            CaptureEngine::NativeMeeting => MeetingCaptureEngineKind::NativeMeeting,
        },
        reason: capability.reason,
    })
}

#[cfg(test)]
mod tests {
    use anlg_audio::meeting::{
        CallPlatform, ObservationSource, ObservedParticipant, SpeakerObservation,
    };

    use super::*;

    fn identity(capture_id: &str) -> CaptureIdentity {
        CaptureIdentity {
            account_id: "acct".into(),
            session_id: "session".into(),
            capture_id: capture_id.into(),
        }
    }

    fn heard_at(evidence: &mut MeetingSpeakerEvidence, capture_id: &str, ms: f64) {
        evidence.observe(&SpeakerObservation {
            capture: identity(capture_id),
            source: ObservationSource::Accessibility,
            platform: CallPlatform::Zoom,
            at: CaptureInstant::from_millis(ms).unwrap(),
            participants: vec![ObservedParticipant {
                participant_id: "p1".into(),
                display_name: Some("Ada".into()),
                is_self: Some(false),
            }],
            roster_complete: false,
            participant_count: None,
            speaking: None,
        });
    }

    #[test]
    fn a_crashed_capture_closes_at_its_last_evidence_alone_and_stays_closed() {
        let mut crashed = MeetingSpeakerEvidence::new("crashed", 1_000);
        heard_at(&mut crashed, "crashed", 2_000.0);
        heard_at(&mut crashed, "crashed", 7_000.0);
        // Another capture of the session, still recording elsewhere, stays open.
        let mut other = MeetingSpeakerEvidence::new("other", 50_000);
        heard_at(&mut other, "other", 3_000.0);
        let stored = anlg_transcript::upsert_meeting_evidence(&Value::Null, &crashed).unwrap();
        let stored = anlg_transcript::upsert_meeting_evidence(&stored, &other).unwrap();

        let closed = close_crashed_evidence(&stored, "crashed").unwrap();
        let ended = |capture_id: &str| {
            closed
                .as_array()
                .unwrap()
                .iter()
                .find(|entry| entry["capture_id"] == capture_id)
                .map(|entry| entry.get("ended_at_ms").cloned())
                .unwrap()
        };
        assert_eq!(ended("crashed"), Some(Value::from(7_000)));
        assert_eq!(ended("other"), None);

        assert_eq!(close_crashed_evidence(&closed, "crashed"), None);
        assert_eq!(close_crashed_evidence(&closed, "unknown"), None);
        // A late snapshot taken while it still recorded never reopens it.
        heard_at(&mut crashed, "crashed", 9_000.0);
        assert_eq!(
            anlg_transcript::upsert_meeting_evidence(&closed, &crashed),
            None
        );
    }

    #[test]
    fn the_open_evidence_journal_keeps_one_note_per_capture_inside_its_directory() {
        let dir = std::env::temp_dir().join(format!("anlg-open-evidence-{}", uuid::Uuid::new_v4()));
        note_open_evidence(&dir, &identity("c-1")).unwrap();
        note_open_evidence(&dir, &identity("c-1")).unwrap();
        assert!(note_open_evidence(&dir, &identity("../escape")).is_err());
        let listed = list_open_evidence(&dir);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].capture, identity("c-1"));

        clear_open_evidence(&dir, "c-1");
        assert!(list_open_evidence(&dir).is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
