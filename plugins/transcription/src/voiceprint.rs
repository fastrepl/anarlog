use std::collections::HashMap;
use std::path::PathBuf;

use anlg_voiceprint::{SelectedSpan, SpanConfig, SpanWord, select_speaker_spans};
use base64::Engine;
use tauri::Manager;

const MODEL_PROVIDER: &str = "pyannote";
const MODEL_VERSION: &str = "wespeaker-embedding-onnx-1";
const CANDIDATE_TTL_DAYS: i64 = 45;
const EMBEDDING_SAMPLE_RATE: u32 = anlg_embedding::SAMPLE_RATE_HZ;

#[derive(Debug, serde::Deserialize)]
struct StoredWord {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    start_ms: f64,
    #[serde(default)]
    end_ms: f64,
    #[serde(default)]
    channel: i64,
    #[serde(default)]
    speaker_index: Option<i64>,
}

#[derive(Debug, serde::Deserialize)]
struct StoredHint {
    #[serde(default)]
    word_id: String,
    #[serde(default, rename = "type")]
    hint_type: String,
    #[serde(default)]
    value: serde_json::Value,
}

#[tauri::command]
#[specta::specta]
pub async fn extract_voiceprint_candidates<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    session_id: String,
    transcript_id: String,
    audio_path: String,
) -> Result<u32, String> {
    let pool = app
        .try_state::<tauri_plugin_db::ManagedState>()
        .map(|state| state.pool().clone())
        .ok_or_else(|| "database is not ready yet".to_string())?;

    let Some((workspace_id, words_json, hints_json)) =
        sqlx::query_as::<_, (String, String, String)>(
            "SELECT workspace_id, words_json, speaker_hints_json
             FROM transcripts
             WHERE id = ? AND session_id = ? AND deleted_at IS NULL
             LIMIT 1",
        )
        .bind(&transcript_id)
        .bind(&session_id)
        .fetch_optional(&pool)
        .await
        .map_err(|error| error.to_string())?
    else {
        return Ok(0);
    };

    let Some(attachment_id) = sqlx::query_scalar::<_, String>(
        "SELECT id FROM session_attachments
         WHERE session_id = ? AND source_type = 'session_audio' AND deleted_at IS NULL
         ORDER BY created_at
         LIMIT 1",
    )
    .bind(&session_id)
    .fetch_optional(&pool)
    .await
    .map_err(|error| error.to_string())?
    else {
        return Ok(0);
    };

    // Repair and re-transcription flows can finalize the same transcript
    // more than once; extract only for transcripts we have not seen.
    let already_extracted: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1 FROM voiceprint_candidates
           WHERE source_transcript_id = ? AND deleted_at IS NULL
         )",
    )
    .bind(&transcript_id)
    .fetch_one(&pool)
    .await
    .map_err(|error| error.to_string())?;
    if already_extracted {
        return Ok(0);
    }

    let spans = spans_from_transcript(&words_json, &hints_json)?;
    if spans.is_empty() {
        return Ok(0);
    }

    let path = PathBuf::from(&audio_path);
    if !path.exists() {
        return Ok(0);
    }

    let embeddings = tauri::async_runtime::spawn_blocking(move || compute_embeddings(&path, spans))
        .await
        .map_err(|error| error.to_string())??;

    let expires_at = (chrono::Utc::now() + chrono::Duration::days(CANDIDATE_TTL_DAYS))
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();

    let mut stored: u32 = 0;
    for (span, embedding) in embeddings {
        let id = uuid::Uuid::new_v4().to_string();
        let encoded = encode_embedding(&embedding);

        if let Err(error) = tauri_plugin_store2::write_secret(
            app.clone(),
            anlg_db_app::VOICEPRINT_CANDIDATE_KEYRING_SCOPE.to_string(),
            id.clone(),
            encoded,
        )
        .await
        {
            tracing::warn!(?error, "voiceprint_candidate_secret_write_failed");
            continue;
        }

        let inserted = anlg_db_app::insert_voiceprint_candidate(
            &pool,
            anlg_db_app::NewVoiceprintCandidate {
                id: &id,
                workspace_id: &workspace_id,
                keyring_key: &id,
                model_provider: MODEL_PROVIDER,
                model_version: MODEL_VERSION,
                capture_domain: capture_domain(span.channel),
                source_session_id: &session_id,
                source_transcript_id: &transcript_id,
                source_attachment_id: &attachment_id,
                source_speaker_label: &speaker_label(&span),
                speaker_channel: span.channel,
                speaker_index: span.speaker_index,
                source_start_ms: span.start_ms,
                source_end_ms: span.end_ms,
                quality_score: span.quality_score,
                expires_at: &expires_at,
            },
        )
        .await;

        match inserted {
            Ok(_) => stored += 1,
            Err(error) => {
                tracing::warn!(%error, "voiceprint_candidate_insert_failed");
                delete_secret(
                    &app,
                    anlg_db_app::VOICEPRINT_CANDIDATE_KEYRING_SCOPE.to_string(),
                    id,
                )
                .await;
            }
        }
    }

    tracing::info!(
        session_id = %session_id,
        transcript_id = %transcript_id,
        stored,
        "voiceprint_candidates_extracted"
    );
    Ok(stored)
}

#[tauri::command]
#[specta::specta]
pub async fn promote_voiceprint_candidates<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    transcript_id: String,
    speaker_channel: i32,
    speaker_index: Option<i32>,
    human_id: String,
) -> Result<u32, String> {
    let pool = app
        .try_state::<tauri_plugin_db::ManagedState>()
        .map(|state| state.pool().clone())
        .ok_or_else(|| "database is not ready yet".to_string())?;

    let Some(workspace_id) = sqlx::query_scalar::<_, String>(
        "SELECT workspace_id FROM transcripts WHERE id = ? AND deleted_at IS NULL LIMIT 1",
    )
    .bind(&transcript_id)
    .fetch_optional(&pool)
    .await
    .map_err(|error| error.to_string())?
    else {
        return Ok(0);
    };

    let label = speaker_label(&SelectedSpan {
        channel: speaker_channel as i64,
        speaker_index: speaker_index.map(i64::from),
        start_ms: 0,
        end_ms: 0,
        quality_score: 0.0,
    });

    let stale = anlg_db_app::tombstone_voiceprint_exemplars_for_source_speaker(
        &pool,
        &workspace_id,
        &transcript_id,
        &label,
        &human_id,
    )
    .await
    .map_err(|error| error.to_string())?;
    for secret in stale {
        delete_secret(&app, secret.keyring_scope, secret.keyring_key.clone()).await;
        let _ = anlg_db_app::purge_tombstoned_voiceprint_exemplar(
            &pool,
            &workspace_id,
            &secret.keyring_key,
        )
        .await;
    }

    let candidates = anlg_db_app::list_active_voiceprint_candidates_for_speaker(
        &pool,
        &workspace_id,
        &transcript_id,
        speaker_channel as i64,
        speaker_index.map(i64::from),
    )
    .await
    .map_err(|error| error.to_string())?;

    let mut promoted: u32 = 0;
    for candidate in candidates {
        let Ok(Some(secret_value)) = tauri_plugin_store2::read_secret(
            app.clone(),
            candidate.keyring_scope.clone(),
            candidate.keyring_key.clone(),
        )
        .await
        else {
            tracing::warn!(candidate_id = %candidate.id, "voiceprint_candidate_secret_missing");
            continue;
        };

        // Secret moves before the row transition so a crash can leave an
        // orphan secret but never an exemplar without its vector.
        if let Err(error) = tauri_plugin_store2::write_secret(
            app.clone(),
            anlg_db_app::VOICEPRINT_KEYRING_SCOPE.to_string(),
            candidate.id.clone(),
            secret_value,
        )
        .await
        {
            tracing::warn!(?error, "voiceprint_exemplar_secret_write_failed");
            continue;
        }

        match anlg_db_app::promote_voiceprint_candidate(
            &pool,
            anlg_db_app::PromoteVoiceprintCandidate {
                candidate_id: &candidate.id,
                workspace_id: &workspace_id,
                human_id: &human_id,
                confirmation_source: "manual_speaker_assignment",
                label_confidence: 1.0,
            },
        )
        .await
        {
            Ok((_, candidate_secret)) => {
                promoted += 1;
                delete_secret(
                    &app,
                    candidate_secret.keyring_scope,
                    candidate_secret.keyring_key,
                )
                .await;
            }
            Err(error) => {
                tracing::warn!(%error, "voiceprint_candidate_promotion_failed");
                delete_secret(
                    &app,
                    anlg_db_app::VOICEPRINT_KEYRING_SCOPE.to_string(),
                    candidate.id.clone(),
                )
                .await;
            }
        }
    }

    tracing::info!(
        transcript_id = %transcript_id,
        speaker_channel,
        promoted,
        "voiceprint_candidates_promoted"
    );
    Ok(promoted)
}

#[tauri::command]
#[specta::specta]
pub async fn cleanup_expired_voiceprint_candidates<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<u32, String> {
    let pool = app
        .try_state::<tauri_plugin_db::ManagedState>()
        .map(|state| state.pool().clone())
        .ok_or_else(|| "database is not ready yet".to_string())?;

    let now = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    let expired = anlg_db_app::tombstone_expired_voiceprint_candidates(&pool, &now)
        .await
        .map_err(|error| error.to_string())?;
    let removed = expired.len() as u32;

    for secret in expired {
        delete_secret(&app, secret.keyring_scope, secret.keyring_key).await;
    }
    let _ = anlg_db_app::purge_expired_tombstoned_voiceprint_candidates(&pool, &now).await;

    if removed > 0 {
        tracing::info!(removed, "voiceprint_candidates_expired");
    }
    Ok(removed)
}

async fn delete_secret<R: tauri::Runtime>(app: &tauri::AppHandle<R>, scope: String, key: String) {
    let app = app.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || {
        tauri_plugin_store2::delete_secret_blocking(&app, &scope, &key)
    })
    .await;
}

fn spans_from_transcript(words_json: &str, hints_json: &str) -> Result<Vec<SelectedSpan>, String> {
    let words: Vec<StoredWord> =
        serde_json::from_str(words_json).map_err(|error| error.to_string())?;
    let hints: Vec<StoredHint> = serde_json::from_str(hints_json).unwrap_or_default();

    let hinted_speaker_by_word: HashMap<String, i64> = hints
        .iter()
        .filter(|hint| hint.hint_type == "provider_speaker_index")
        .filter_map(|hint| {
            speaker_index_from_hint_value(&hint.value).map(|index| (hint.word_id.clone(), index))
        })
        .collect();

    let span_words: Vec<SpanWord> = words
        .iter()
        .map(|word| SpanWord {
            start_ms: word.start_ms as i64,
            end_ms: word.end_ms as i64,
            channel: word.channel,
            speaker_index: word
                .id
                .as_ref()
                .and_then(|id| hinted_speaker_by_word.get(id).copied())
                .or(word.speaker_index),
        })
        .collect();

    Ok(select_speaker_spans(&span_words, &SpanConfig::default()))
}

// Hint values are stored either as JSON objects or as JSON-encoded strings,
// depending on which writer produced them.
fn speaker_index_from_hint_value(value: &serde_json::Value) -> Option<i64> {
    let object = match value {
        serde_json::Value::String(raw) => serde_json::from_str::<serde_json::Value>(raw).ok()?,
        other => other.clone(),
    };
    object.get("speaker_index")?.as_i64()
}

type SpanEmbedding = (SelectedSpan, Vec<f32>);

fn compute_embeddings(
    path: &std::path::Path,
    spans: Vec<SelectedSpan>,
) -> Result<Vec<SpanEmbedding>, String> {
    let source = anlg_audio_utils::source_from_path(path).map_err(|error| error.to_string())?;
    let channels = anlg_audio_utils::resample_audio_channels(source, EMBEDDING_SAMPLE_RATE)
        .map_err(|error| error.to_string())?;
    if channels.is_empty() || channels[0].is_empty() {
        return Ok(Vec::new());
    }

    let mut extractor =
        anlg_embedding::EmbeddingExtractor::new().map_err(|error| error.to_string())?;

    let mut results = Vec::new();
    for span in spans {
        let Some(samples) = span_samples(&channels, &span) else {
            continue;
        };
        match extractor.compute_optional(&samples) {
            Ok(Some(embedding)) => results.push((span, embedding)),
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(%error, "voiceprint_embedding_failed");
            }
        }
    }
    Ok(results)
}

fn span_samples(channels: &[Vec<f32>], span: &SelectedSpan) -> Option<Vec<f32>> {
    let ms_to_index =
        |ms: i64| -> usize { (ms.max(0) as usize) * (EMBEDDING_SAMPLE_RATE as usize) / 1000 };
    let start = ms_to_index(span.start_ms);
    let end = ms_to_index(span.end_ms);

    let slice = |channel: &Vec<f32>| -> Option<Vec<f32>> {
        let end = end.min(channel.len());
        (start < end).then(|| channel[start..end].to_vec())
    };

    match (span.channel, channels.len()) {
        (_, 0) => None,
        (channel, count) if channel >= 0 && (channel as usize) < count && count > 1 => {
            slice(&channels[channel as usize])
        }
        (_, 1) => slice(&channels[0]),
        // A mixed-capture span over a multi-channel file: average the channels.
        (_, count) => {
            let end = end.min(channels[0].len());
            (start < end).then(|| {
                (start..end)
                    .map(|index| {
                        channels.iter().map(|channel| channel[index]).sum::<f32>() / count as f32
                    })
                    .collect()
            })
        }
    }
}

fn capture_domain(channel: i64) -> &'static str {
    match channel {
        0 => "direct_mic",
        1 => "system_audio",
        _ => "mixed_capture",
    }
}

fn speaker_label(span: &SelectedSpan) -> String {
    match span.speaker_index {
        Some(index) => format!("ch{}:s{}", span.channel, index),
        None => format!("ch{}", span.channel),
    }
}

fn encode_embedding(embedding: &[f32]) -> String {
    let mut bytes = Vec::with_capacity(embedding.len() * 4);
    for value in embedding {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_use_provider_hints_over_word_speaker_index() {
        let words = r#"[
            {"id": "w1", "text": "hello", "start_ms": 0, "end_ms": 2000, "channel": 1},
            {"id": "w2", "text": "there", "start_ms": 2100, "end_ms": 4000, "channel": 1}
        ]"#;
        let hints = r#"[
            {"id": "h1", "word_id": "w1", "type": "provider_speaker_index", "value": "{\"speaker_index\": 3, \"channel\": 1}"},
            {"id": "h2", "word_id": "w2", "type": "provider_speaker_index", "value": {"speaker_index": 3, "channel": 1}}
        ]"#;

        let spans = spans_from_transcript(words, hints).unwrap();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].speaker_index, Some(3));
        assert_eq!((spans[0].start_ms, spans[0].end_ms), (0, 4000));
    }

    #[test]
    fn mixed_capture_span_averages_multi_channel_audio() {
        let channels = vec![vec![0.2_f32; 32_000], vec![0.4_f32; 32_000]];
        let span = SelectedSpan {
            channel: 2,
            speaker_index: Some(0),
            start_ms: 0,
            end_ms: 1_000,
            quality_score: 0.1,
        };
        let samples = span_samples(&channels, &span).unwrap();
        assert_eq!(samples.len(), 16_000);
        assert!((samples[0] - 0.3).abs() < 1e-6);
    }

    #[test]
    fn stereo_span_uses_matching_channel() {
        let channels = vec![vec![0.1_f32; 32_000], vec![0.9_f32; 32_000]];
        let span = SelectedSpan {
            channel: 1,
            speaker_index: Some(0),
            start_ms: 500,
            end_ms: 1_500,
            quality_score: 0.1,
        };
        let samples = span_samples(&channels, &span).unwrap();
        assert_eq!(samples.len(), 16_000);
        assert!((samples[0] - 0.9).abs() < 1e-6);
    }

    #[test]
    fn embedding_round_trips_through_base64() {
        let embedding = vec![0.5_f32, -1.25, 3.0];
        let encoded = encode_embedding(&embedding);
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        let decoded: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
            .collect();
        assert_eq!(decoded, embedding);
    }
}
