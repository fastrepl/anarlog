use serde_json::Value;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum CapturePhase {
    Capturing,
    Finalizing,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SummaryMode {
    Regenerate,
    IfEmpty,
    Refresh,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InheritedCapture {
    pub transcript_id: String,
    pub started_at: f64,
    pub created_at: String,
    pub owner_user_id: String,
    pub memo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retain_audio: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureLifecycleMarker {
    pub version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunked_audio: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retain_audio: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<CapturePhase>,
    pub session_id: String,
    pub transcript_id: String,
    pub started_at: f64,
    pub created_at: String,
    pub audio_offset_ms: f64,
    pub preserve_existing_transcript: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub automatic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preserve_existing_audio: Option<bool>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_summary_after_repair: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inherited_captures: Option<Vec<InheritedCapture>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inherited_only: Option<bool>,
}

pub fn parse_marker(value_json: &str, session_id: &str) -> Option<CaptureLifecycleMarker> {
    if session_id.is_empty() {
        return None;
    }

    let value: Value = serde_json::from_str(value_json).ok()?;
    let object = value.as_object()?;
    let version = object.get("version")?.as_f64()?;
    let stored_session_id = object.get("sessionId")?.as_str()?;
    let transcript_id = object.get("transcriptId")?.as_str()?;
    let started_at = finite_number(object.get("startedAt")?)?;
    let created_at = object.get("createdAt")?.as_str()?;
    let audio_offset_ms = finite_number(object.get("audioOffsetMs")?)?;
    let preserve_existing_transcript = object.get("preserveExistingTranscript")?.as_bool()?;
    let owner_user_id = object.get("ownerUserId")?.as_str()?;
    let memo = object.get("memo")?.as_str()?;

    if version != 1.0 || stored_session_id != session_id || transcript_id.is_empty() {
        return None;
    }

    Some(CaptureLifecycleMarker {
        version: 1,
        chunked_audio: object.get("chunkedAudio").and_then(Value::as_bool),
        retain_audio: object.get("retainAudio").and_then(Value::as_bool),
        phase: object
            .get("phase")
            .and_then(Value::as_str)
            .and_then(|phase| match phase {
                "capturing" => Some(CapturePhase::Capturing),
                "finalizing" => Some(CapturePhase::Finalizing),
                _ => None,
            }),
        session_id: session_id.to_string(),
        transcript_id: transcript_id.to_string(),
        started_at,
        created_at: created_at.to_string(),
        audio_offset_ms: audio_offset_ms.max(0.0),
        preserve_existing_transcript,
        automatic: object.get("automatic").and_then(Value::as_bool),
        preserve_existing_audio: object.get("preserveExistingAudio").and_then(Value::as_bool),
        initial_title: object
            .get("initialTitle")
            .and_then(Value::as_str)
            .map(str::to_string),
        owner_user_id: owner_user_id.to_string(),
        memo: memo.to_string(),
        provider: object
            .get("provider")
            .and_then(Value::as_str)
            .map(str::to_string),
        model: object
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_string),
        summary_mode: object.get("summaryMode").and_then(Value::as_str).and_then(
            |mode| match mode {
                "regenerate" => Some(SummaryMode::Regenerate),
                "if_empty" => Some(SummaryMode::IfEmpty),
                "refresh" => Some(SummaryMode::Refresh),
                _ => None,
            },
        ),
        refresh_summary_after_repair: object
            .get("refreshSummaryAfterRepair")
            .and_then(Value::as_bool)
            .filter(|value| *value),
        inherited_captures: object
            .get("inheritedCaptures")
            .and_then(Value::as_array)
            .map(|captures| {
                captures
                    .iter()
                    .filter_map(parse_inherited_capture)
                    .collect()
            }),
        inherited_only: object
            .get("inheritedOnly")
            .and_then(Value::as_bool)
            .filter(|value| *value),
    })
}

pub(crate) fn serialize_marker(marker: &CaptureLifecycleMarker) -> Result<String, String> {
    if marker.version != 1 {
        return Err("unsupported capture lifecycle marker version".to_string());
    }
    if marker.session_id.is_empty() {
        return Err("capture lifecycle marker session ID must not be empty".to_string());
    }
    if marker.transcript_id.is_empty() {
        return Err("capture lifecycle marker transcript ID must not be empty".to_string());
    }
    if !marker.started_at.is_finite() || !marker.audio_offset_ms.is_finite() {
        return Err("capture lifecycle marker timestamps must be finite".to_string());
    }

    serde_json::to_string(marker).map_err(|error| error.to_string())
}

fn finite_number(value: &Value) -> Option<f64> {
    value.as_f64().filter(|value| value.is_finite())
}

fn parse_inherited_capture(value: &Value) -> Option<InheritedCapture> {
    let object = value.as_object()?;
    let transcript_id = object.get("transcriptId")?.as_str()?;
    let started_at = finite_number(object.get("startedAt")?)?;
    let created_at = object.get("createdAt")?.as_str()?;
    let owner_user_id = object.get("ownerUserId")?.as_str()?;
    let memo = object.get("memo")?.as_str()?;

    if transcript_id.is_empty() {
        return None;
    }

    Some(InheritedCapture {
        transcript_id: transcript_id.to_string(),
        started_at,
        created_at: created_at.to_string(),
        owner_user_id: owner_user_id.to_string(),
        memo: memo.to_string(),
        retain_audio: object.get("retainAudio").and_then(Value::as_bool),
        provider: object
            .get("provider")
            .and_then(Value::as_str)
            .map(str::to_string),
        model: object
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn database_runtime<R: tauri::Runtime>(
    app: &AppHandle<R>,
) -> Result<tauri_plugin_db::ManagedState, String> {
    app.try_state::<tauri_plugin_db::ManagedState>()
        .map(|state| state.inner().clone())
        .ok_or_else(|| "database is not ready yet".to_string())
}

pub(crate) async fn save_marker<R: tauri::Runtime>(
    app: &AppHandle<R>,
    session_id: &str,
    value_json: &str,
    replace_transcript_id: &str,
) -> Result<bool, String> {
    let runtime = database_runtime(app)?;
    let _guard = runtime.synced_write_guard().await;
    anlg_db_app::upsert_capture_lifecycle_marker(
        runtime.pool(),
        session_id,
        value_json,
        replace_transcript_id,
    )
    .await
    .map_err(|error| error.to_string())
}

pub(crate) async fn clear_marker(
    app: &AppHandle<impl tauri::Runtime>,
    session_id: &str,
    transcript_id: &str,
) -> Result<bool, String> {
    let runtime = database_runtime(app)?;
    let _guard = runtime.synced_write_guard().await;
    anlg_db_app::delete_capture_lifecycle_marker(runtime.pool(), session_id, transcript_id)
        .await
        .map_err(|error| error.to_string())
}

pub(crate) async fn get_marker(
    app: &AppHandle<impl tauri::Runtime>,
    session_id: &str,
) -> Result<Option<CaptureLifecycleMarker>, String> {
    let runtime = database_runtime(app)?;
    let value = anlg_db_app::get_capture_lifecycle_marker_json(runtime.pool(), session_id)
        .await
        .map_err(|error| error.to_string())?;
    Ok(value.and_then(|value_json| parse_marker(&value_json, session_id)))
}

pub(crate) async fn list_markers(
    app: &AppHandle<impl tauri::Runtime>,
) -> Result<Vec<CaptureLifecycleMarker>, String> {
    let runtime = database_runtime(app)?;
    let rows = anlg_db_app::list_capture_lifecycle_marker_jsons(runtime.pool())
        .await
        .map_err(|error| error.to_string())?;
    Ok(rows
        .into_iter()
        .filter_map(|(session_id, value_json)| parse_marker(&value_json, &session_id))
        .collect())
}

pub(crate) async fn mark_audio_saved(
    app: &AppHandle<impl tauri::Runtime>,
    session_id: &str,
) -> Result<(), String> {
    let runtime = database_runtime(app)?;
    let _guard = runtime.synced_write_guard().await;
    anlg_db_app::mark_capture_audio_saved(runtime.pool(), session_id)
        .await
        .map_err(|error| error.to_string())
}

pub(crate) async fn clear_audio_saved(
    app: &AppHandle<impl tauri::Runtime>,
    session_id: &str,
) -> Result<(), String> {
    let runtime = database_runtime(app)?;
    let _guard = runtime.synced_write_guard().await;
    anlg_db_app::clear_capture_audio_saved(runtime.pool(), session_id)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Map;

    #[test]
    fn parses_capture_markers_with_typescript_compatibility() {
        let marker = CaptureLifecycleMarker {
            version: 1,
            chunked_audio: Some(true),
            retain_audio: Some(false),
            phase: Some(CapturePhase::Finalizing),
            session_id: "session-1".to_string(),
            transcript_id: "transcript-1".to_string(),
            started_at: 123.5,
            created_at: "2026-09-30T00:00:00.000Z".to_string(),
            audio_offset_ms: 25.0,
            preserve_existing_transcript: true,
            automatic: Some(false),
            preserve_existing_audio: Some(true),
            initial_title: Some("Title".to_string()),
            owner_user_id: "owner-1".to_string(),
            memo: "memo".to_string(),
            provider: Some("provider".to_string()),
            model: Some("model".to_string()),
            summary_mode: Some(SummaryMode::IfEmpty),
            refresh_summary_after_repair: Some(true),
            inherited_captures: Some(vec![InheritedCapture {
                transcript_id: "inherited-1".to_string(),
                started_at: 100.0,
                created_at: "2026-09-29T00:00:00.000Z".to_string(),
                owner_user_id: "owner-2".to_string(),
                memo: "inherited memo".to_string(),
                retain_audio: Some(true),
                provider: Some("provider-2".to_string()),
                model: Some("model-2".to_string()),
            }]),
            inherited_only: Some(true),
        };
        let value_json = serde_json::to_string(&marker).unwrap();

        assert_eq!(parse_marker(&value_json, "session-1"), Some(marker.clone()));
        assert_eq!(parse_marker(&value_json, "session-2"), None);
        assert_eq!(parse_marker("{", "session-1"), None);

        let mut altered: Value = serde_json::from_str(&value_json).unwrap();
        let object: &mut Map<String, Value> = altered.as_object_mut().unwrap();
        object.insert("phase".to_string(), Value::String("bogus".to_string()));
        object.insert("audioOffsetMs".to_string(), Value::from(-10.0));
        object.insert(
            "inheritedCaptures".to_string(),
            serde_json::json!([{"transcriptId": "missing-required-fields"}]),
        );
        let parsed = parse_marker(&altered.to_string(), "session-1").unwrap();
        assert_eq!(parsed.phase, None);
        assert_eq!(parsed.audio_offset_ms, 0.0);
        assert_eq!(parsed.inherited_captures, Some(Vec::new()));
    }
}
