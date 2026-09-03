use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use anlg_voiceprint::{
    MIN_UNIQUE_MARGIN, MIN_UNIQUE_SCORE, SelectedSpan, SpanConfig, SpanWord, VoiceprintAssignment,
    VoiceprintSpeakerKey, collect_match_scores, complete_by_elimination,
    pick_unique_voiceprint_assignments, remote_participant_human_ids, select_speaker_spans,
};
use base64::Engine;
use tauri::Manager;
use tauri_plugin_store2::Store2PluginExt;

const MODEL_PROVIDER: &str = "pyannote";
const MODEL_VERSION: &str = "wespeaker-embedding-onnx-1";
const CANDIDATE_TTL_DAYS: i64 = 45;
const EMBEDDING_SAMPLE_RATE: u32 = anlg_embedding::SAMPLE_RATE_HZ;
const DIRECT_MIC_CHANNEL: i64 = 0;
/// System-audio channel of a stereo capture: every remote participant, nobody else.
const REMOTE_CHANNEL: i64 = 1;
const ISOLATED_MIC_CONFIRMATION_SOURCE: &str = "isolated_mic_capture";
// Below a manual assignment: someone else could still lean into the laptop mic.
const ISOLATED_MIC_LABEL_CONFIDENCE: f64 = 0.9;
const MIC_ISOLATION_STORE_SCOPE: &str = "transcription.mic_isolation";

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
    // more than once; extract only for transcripts we have not seen. Promotion
    // tombstones candidates, so exemplars sourced here count as seen too.
    let already_extracted: bool = sqlx::query_scalar(
        "SELECT EXISTS(
           SELECT 1 FROM voiceprint_candidates
           WHERE source_transcript_id = ? AND deleted_at IS NULL
         ) OR EXISTS(
           SELECT 1 FROM voiceprint_exemplars
           WHERE source_transcript_id = ? AND deleted_at IS NULL
         )",
    )
    .bind(&transcript_id)
    .bind(&transcript_id)
    .fetch_one(&pool)
    .await
    .map_err(|error| error.to_string())?;
    if already_extracted {
        if let Err(error) = maybe_assign_speakers_from_voiceprints(
            &app,
            &pool,
            &session_id,
            &transcript_id,
            &workspace_id,
            None,
        )
        .await
        {
            tracing::warn!(%error, "voiceprint_assignment_failed");
        }
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
    for (span, embedding) in &embeddings {
        let id = uuid::Uuid::new_v4().to_string();
        let encoded = encode_embedding(embedding);

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

    if stored > 0 && mic_isolated_for_session(&app, &session_id) == Some(true) {
        let promoted = confirm_isolated_mic_candidates(
            &app,
            &pool,
            &session_id,
            &transcript_id,
            &workspace_id,
            &embeddings,
        )
        .await;
        tracing::info!(
            session_id = %session_id,
            transcript_id = %transcript_id,
            promoted,
            "isolated_mic_self_exemplars_confirmed"
        );
    }

    if let Err(error) = maybe_assign_speakers_from_voiceprints(
        &app,
        &pool,
        &session_id,
        &transcript_id,
        &workspace_id,
        Some((&embeddings, &words_json, &hints_json)),
    )
    .await
    {
        tracing::warn!(%error, "voiceprint_assignment_failed");
    }

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

    promote_speaker_candidates(
        &app,
        &pool,
        SpeakerPromotion {
            workspace_id: &workspace_id,
            transcript_id: &transcript_id,
            speaker_channel: i64::from(speaker_channel),
            speaker_index: speaker_index.map(i64::from),
            human_id: &human_id,
            confirmation_source: "manual_speaker_assignment",
            label_confidence: 1.0,
        },
    )
    .await
}

struct SpeakerPromotion<'a> {
    workspace_id: &'a str,
    transcript_id: &'a str,
    speaker_channel: i64,
    speaker_index: Option<i64>,
    human_id: &'a str,
    confirmation_source: &'static str,
    label_confidence: f64,
}

async fn promote_speaker_candidates<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    pool: &sqlx::SqlitePool,
    promotion: SpeakerPromotion<'_>,
) -> Result<u32, String> {
    let SpeakerPromotion {
        workspace_id,
        transcript_id,
        speaker_channel,
        speaker_index,
        human_id,
        confirmation_source,
        label_confidence,
    } = promotion;

    let label = speaker_label(&SelectedSpan {
        channel: speaker_channel,
        speaker_index,
        start_ms: 0,
        end_ms: 0,
        quality_score: 0.0,
    });

    let stale = anlg_db_app::tombstone_voiceprint_exemplars_for_source_speaker(
        pool,
        workspace_id,
        transcript_id,
        &label,
        human_id,
    )
    .await
    .map_err(|error| error.to_string())?;
    for secret in stale {
        delete_secret(app, secret.keyring_scope, secret.keyring_key.clone()).await;
        let _ = anlg_db_app::purge_tombstoned_voiceprint_exemplar(
            pool,
            workspace_id,
            &secret.keyring_key,
        )
        .await;
    }

    let candidates = anlg_db_app::list_active_voiceprint_candidates_for_speaker(
        pool,
        workspace_id,
        transcript_id,
        speaker_channel,
        speaker_index,
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
            pool,
            anlg_db_app::PromoteVoiceprintCandidate {
                candidate_id: &candidate.id,
                workspace_id,
                human_id,
                confirmation_source,
                label_confidence,
            },
        )
        .await
        {
            Ok((_, candidate_secret)) => {
                promoted += 1;
                delete_secret(
                    app,
                    candidate_secret.keyring_scope,
                    candidate_secret.keyring_key,
                )
                .await;
            }
            Err(error) => {
                tracing::warn!(%error, "voiceprint_candidate_promotion_failed");
                delete_secret(
                    app,
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
        confirmation_source,
        promoted,
        "voiceprint_candidates_promoted"
    );
    Ok(promoted)
}

/// With headphone output the direct-mic channel can only carry the session owner, so its
/// candidates are confirmed self exemplars without waiting for a manual speaker assignment.
/// Returns the number of promoted candidates; storage problems are logged, never fatal.
async fn confirm_isolated_mic_candidates<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    pool: &sqlx::SqlitePool,
    session_id: &str,
    transcript_id: &str,
    workspace_id: &str,
    embeddings: &[SpanEmbedding],
) -> u32 {
    let speakers = direct_mic_speakers(embeddings);
    if speakers.is_empty() {
        return 0;
    }

    let owner = match sqlx::query_scalar::<_, String>(
        "SELECT owner_user_id FROM sessions WHERE id = ? AND deleted_at IS NULL LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(owner)) if !owner.is_empty() => owner,
        Ok(_) => return 0,
        Err(error) => {
            tracing::warn!(%error, "isolated_mic_owner_lookup_failed");
            return 0;
        }
    };

    let mut promoted = 0;
    for speaker_index in speakers {
        match promote_speaker_candidates(
            app,
            pool,
            SpeakerPromotion {
                workspace_id,
                transcript_id,
                speaker_channel: DIRECT_MIC_CHANNEL,
                speaker_index,
                human_id: &owner,
                confirmation_source: ISOLATED_MIC_CONFIRMATION_SOURCE,
                label_confidence: ISOLATED_MIC_LABEL_CONFIDENCE,
            },
        )
        .await
        {
            Ok(count) => promoted += count,
            Err(error) => tracing::warn!(%error, "isolated_mic_promotion_failed"),
        }
    }
    promoted
}

/// Remembers a stopped session's mic isolation on disk so extraction after a relaunch or crash
/// recovery still sees it; the in-memory cache alone dies with the process.
pub(crate) fn persist_mic_isolation<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    session_id: &str,
    value: Option<bool>,
) {
    let store = match app
        .store2()
        .scoped_store::<String>(MIC_ISOLATION_STORE_SCOPE)
    {
        Ok(store) => store,
        Err(error) => {
            tracing::warn!(%error, "mic_isolation_store_unavailable");
            return;
        }
    };
    let result = match value {
        Some(value) => store.set(session_id.to_string(), value),
        None => store.delete(session_id.to_string()),
    };
    if let Err(error) = result {
        tracing::warn!(%error, "mic_isolation_persist_failed");
    }
}

fn mic_isolated_for_session<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    session_id: &str,
) -> Option<bool> {
    let cached = app
        .try_state::<crate::MicIsolationCache>()
        .and_then(|cache| cache.lock().ok()?.get(session_id).copied());
    if cached.is_some() {
        return cached;
    }
    app.store2()
        .scoped_store::<String>(MIC_ISOLATION_STORE_SCOPE)
        .ok()?
        .get::<bool>(session_id.to_string())
        .ok()
        .flatten()
}

/// Distinct provider speaker indices seen on the direct-mic channel. Hosted providers may still
/// split an isolated mic into several "speakers"; every one of them is the owner.
fn direct_mic_speakers(embeddings: &[SpanEmbedding]) -> Vec<Option<i64>> {
    let mut speakers: Vec<Option<i64>> = Vec::new();
    for (span, _) in embeddings {
        if span.channel == DIRECT_MIC_CHANNEL && !speakers.contains(&span.speaker_index) {
            speakers.push(span.speaker_index);
        }
    }
    speakers
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

/// Confirmed voiceprints of the session's participants, for the on-device
/// diarizer. Best effort: any storage problem yields an empty list rather than
/// blocking transcription.
pub(crate) async fn known_speakers_for_session<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    session_id: &str,
) -> Vec<anlg_transcription_core::listener2::KnownSpeaker> {
    let Some(pool) = app
        .try_state::<tauri_plugin_db::ManagedState>()
        .map(|state| state.pool().clone())
    else {
        return Vec::new();
    };
    let participants = match anlg_db_app::list_session_participants(&pool, session_id).await {
        Ok(participants) => participants,
        Err(error) => {
            tracing::warn!(%error, "known_speakers_participants_failed");
            return Vec::new();
        }
    };

    let mut known = Vec::new();
    let mut seen_humans = HashSet::new();
    for participant in participants {
        if participant.human_id.is_empty() || !seen_humans.insert(participant.human_id.clone()) {
            continue;
        }
        let exemplars = match anlg_db_app::list_active_voiceprint_exemplars_for_human(
            &pool,
            &participant.workspace_id,
            &participant.human_id,
        )
        .await
        {
            Ok(exemplars) => exemplars,
            Err(error) => {
                tracing::warn!(%error, "known_speakers_exemplars_failed");
                continue;
            }
        };
        for exemplar in exemplars {
            if exemplar.model_provider != MODEL_PROVIDER || exemplar.model_version != MODEL_VERSION
            {
                continue;
            }
            let Ok(Some(secret_value)) = tauri_plugin_store2::read_secret(
                app.clone(),
                exemplar.keyring_scope.clone(),
                exemplar.keyring_key.clone(),
            )
            .await
            else {
                continue;
            };
            let Some(embedding) = decode_embedding(&secret_value) else {
                continue;
            };
            known.push(anlg_transcription_core::listener2::KnownSpeaker {
                id: exemplar.human_id,
                embedding,
            });
        }
    }

    if !known.is_empty() {
        tracing::info!(
            session_id,
            exemplars = known.len(),
            humans = seen_humans.len(),
            "known_speakers_loaded"
        );
    }
    known
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
    let hinted_speaker_by_word = provider_speaker_index_by_word(&hints);

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
    let (collected, _) = collect_span_samples(source, spans)?;

    let mut extractor =
        anlg_embedding::EmbeddingExtractor::new().map_err(|error| error.to_string())?;

    let mut results = Vec::new();
    for CollectedSpan { span, samples } in collected {
        if samples.is_empty() {
            continue;
        }
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

#[derive(Debug)]
struct CollectedSpan {
    span: SelectedSpan,
    samples: Vec<f32>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct SpanCollectionStats {
    decoded_frames: usize,
    retained_samples: usize,
    max_decoder_block_samples: usize,
}

fn collect_span_samples<S>(
    source: S,
    spans: Vec<SelectedSpan>,
) -> Result<(Vec<CollectedSpan>, SpanCollectionStats), String>
where
    S: anlg_audio_utils::Source,
{
    let mut collected = spans
        .into_iter()
        .map(|span| {
            let (start, end) = span_sample_bounds(&span);
            CollectedSpan {
                span,
                samples: Vec::with_capacity(end.saturating_sub(start)),
            }
        })
        .collect::<Vec<_>>();
    let mut block_start = 0usize;
    let mut stats = SpanCollectionStats::default();

    let info = anlg_audio_utils::for_each_resampled_channel_block::<_, anlg_audio_utils::Error>(
        source,
        EMBEDDING_SAMPLE_RATE,
        |channels| {
            let block_frames = channels.first().map_or(0, |channel| channel.len());
            stats.max_decoder_block_samples = stats
                .max_decoder_block_samples
                .max(channels.iter().map(|channel| channel.len()).sum());

            for span in &mut collected {
                append_span_block(span, channels, block_start, block_frames);
            }
            block_start += block_frames;
            Ok(())
        },
    )
    .map_err(|error| error.to_string())?;

    stats.decoded_frames = info.frame_count;
    stats.retained_samples = collected.iter().map(|span| span.samples.len()).sum();
    Ok((collected, stats))
}

fn span_sample_bounds(span: &SelectedSpan) -> (usize, usize) {
    let ms_to_index =
        |ms: i64| -> usize { (ms.max(0) as usize) * (EMBEDDING_SAMPLE_RATE as usize) / 1000 };
    (ms_to_index(span.start_ms), ms_to_index(span.end_ms))
}

fn append_span_block(
    collected: &mut CollectedSpan,
    channels: &[&[f32]],
    block_start: usize,
    block_frames: usize,
) {
    if channels.is_empty() || block_frames == 0 {
        return;
    }

    let (span_start, span_end) = span_sample_bounds(&collected.span);
    let block_end = block_start + block_frames;
    let overlap_start = span_start.max(block_start);
    let overlap_end = span_end.min(block_end);
    if overlap_start >= overlap_end {
        return;
    }

    let local_start = overlap_start - block_start;
    let local_end = overlap_end - block_start;
    let channel_count = channels.len();

    match collected.span.channel {
        channel if channel >= 0 && channel_count > 1 && (channel as usize) < channel_count => {
            collected
                .samples
                .extend_from_slice(&channels[channel as usize][local_start..local_end]);
        }
        _ if channel_count == 1 => {
            collected
                .samples
                .extend_from_slice(&channels[0][local_start..local_end]);
        }
        _ => {
            collected
                .samples
                .extend((local_start..local_end).map(|index| {
                    channels.iter().map(|channel| channel[index]).sum::<f32>()
                        / channel_count as f32
                }));
        }
    }
}

#[cfg(test)]
fn span_samples(channels: &[Vec<f32>], span: &SelectedSpan) -> Option<Vec<f32>> {
    let (start, end) = span_sample_bounds(span);

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

fn decode_embedding(encoded: &str) -> Option<Vec<f32>> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    if bytes.len() % 4 != 0 || bytes.is_empty() {
        return None;
    }
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            let chunk: [u8; 4] = chunk.try_into().ok()?;
            Some(f32::from_le_bytes(chunk))
        })
        .collect()
}

async fn maybe_assign_speakers_from_voiceprints<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    pool: &sqlx::SqlitePool,
    session_id: &str,
    transcript_id: &str,
    workspace_id: &str,
    extracted: Option<(&[SpanEmbedding], &str, &str)>,
) -> Result<(), String> {
    let stored_embeddings;
    let (embeddings, words_json, mut hints_json) = match extracted
        .filter(|(embeddings, _, _)| !embeddings.is_empty())
    {
        Some((embeddings, words_json, hints_json)) => {
            (embeddings, words_json.to_string(), hints_json.to_string())
        }
        None => {
            stored_embeddings =
                embeddings_from_stored_candidates(app, pool, workspace_id, transcript_id).await?;
            if stored_embeddings.is_empty() {
                return Ok(());
            }
            let Some((words_json, hints_json)) =
                load_transcript_words_and_hints(pool, session_id, transcript_id).await?
            else {
                return Ok(());
            };
            let index = speaker_index_from_transcript(&words_json, &hints_json)?;
            if !embeddings_match_current_speakers(&stored_embeddings, &index) {
                return Ok(());
            }
            (stored_embeddings.as_slice(), words_json, hints_json)
        }
    };

    let mut index = speaker_index_from_transcript(&words_json, &hints_json)?;
    let owner_user_id = sqlx::query_scalar::<_, String>(
        "SELECT owner_user_id FROM sessions WHERE id = ? AND deleted_at IS NULL LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| error.to_string())?;
    let Some(owner_user_id) = owner_user_id else {
        return Ok(());
    };

    let owner_email: Option<String> = sqlx::query_scalar::<_, Option<String>>(
        "SELECT NULLIF(lower(email), '') FROM humans WHERE id = ? AND deleted_at IS NULL LIMIT 1",
    )
    .bind(&owner_user_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| error.to_string())?
    .flatten();

    let participants = anlg_db_app::list_session_participants(pool, session_id)
        .await
        .map_err(|error| error.to_string())?;
    let remote_human_ids = remote_participant_human_ids(
        participants
            .iter()
            .map(|participant| (participant.human_id.as_str(), participant.email.as_str())),
        &owner_user_id,
        owner_email.as_deref(),
    );
    // 1:1 meetings are named from the participant list. Voiceprints only
    // identify people when the calendar cannot uniquely name the remotes.
    if remote_human_ids.len() < 2 {
        return Ok(());
    }

    let mut exemplars = Vec::new();
    for human_id in &remote_human_ids {
        if index.assigned_human_ids.contains(human_id) {
            continue;
        }
        let rows =
            anlg_db_app::list_active_voiceprint_exemplars_for_human(pool, workspace_id, human_id)
                .await
                .map_err(|error| error.to_string())?;
        for exemplar in rows {
            if exemplar.model_provider != MODEL_PROVIDER || exemplar.model_version != MODEL_VERSION
            {
                continue;
            }
            let Ok(Some(secret_value)) = tauri_plugin_store2::read_secret(
                app.clone(),
                exemplar.keyring_scope.clone(),
                exemplar.keyring_key.clone(),
            )
            .await
            else {
                continue;
            };
            let Some(embedding) = decode_embedding(&secret_value) else {
                continue;
            };
            exemplars.push((exemplar.human_id, exemplar.capture_domain, embedding));
        }
    }

    let samples: Vec<_> = embeddings
        .iter()
        .filter(|(span, _)| span.channel != 0)
        .filter(|(span, _)| {
            !index.assigned_speakers.contains(&VoiceprintSpeakerKey {
                channel: span.channel,
                speaker_index: span.speaker_index,
            })
        })
        .map(|(span, embedding)| {
            (
                VoiceprintSpeakerKey {
                    channel: span.channel,
                    speaker_index: span.speaker_index,
                },
                capture_domain(span.channel),
                embedding.as_slice(),
            )
        })
        .collect();
    let scores = collect_match_scores(&samples, &exemplars);
    let voiceprint_namings: Vec<SpeakerNaming> = pick_unique_voiceprint_assignments(
        &scores
            .iter()
            .map(|(speaker, human_id, score)| (*speaker, human_id.as_str(), *score))
            .collect::<Vec<_>>(),
        MIN_UNIQUE_SCORE,
        MIN_UNIQUE_MARGIN,
    )
    .into_iter()
    .map(SpeakerNaming::from_voiceprint)
    .collect();

    for attempt in 0..2 {
        // Elimination depends on which pairs the current hints leave open, so
        // it is redone against the hints a retry reloads rather than carried over.
        let mut namings: Vec<SpeakerNaming> = voiceprint_namings
            .iter()
            .filter(|naming| {
                !index.assigned_speakers.contains(&naming.speaker)
                    && !index.assigned_human_ids.contains(&naming.human_id)
            })
            .cloned()
            .collect();
        if let Some(naming) = eliminate_last_remote_speaker(&index, &remote_human_ids, &namings) {
            namings.push(naming);
        }
        if namings.is_empty() {
            return Ok(());
        }
        let Some(next_json) = assignment_hints_json(&hints_json, &index, &namings)? else {
            return Ok(());
        };
        if write_speaker_assignment_hints(
            pool,
            session_id,
            transcript_id,
            &words_json,
            &hints_json,
            &next_json,
        )
        .await?
        {
            tracing::info!(
                session_id,
                transcript_id,
                assigned = namings.len(),
                by_elimination = namings
                    .iter()
                    .filter(|naming| naming.basis == NamingBasis::Elimination)
                    .count(),
                "voiceprint_speakers_assigned"
            );
            return Ok(());
        }
        if attempt == 0 {
            tracing::warn!(transcript_id, "voiceprint_assignment_hints_conflict");
            let Some(latest) =
                load_transcript_words_and_hints(pool, session_id, transcript_id).await?
            else {
                return Ok(());
            };
            // Embeddings were scored against this words snapshot. A newer
            // diarization would map those speaker keys onto the wrong clusters.
            if latest.0 != words_json {
                return Ok(());
            }
            hints_json = latest.1;
            index = speaker_index_from_transcript(&words_json, &hints_json)?;
        }
    }
    Ok(())
}

async fn load_transcript_words_and_hints(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    transcript_id: &str,
) -> Result<Option<(String, String)>, String> {
    sqlx::query_as::<_, (String, String)>(
        "SELECT words_json, speaker_hints_json
         FROM transcripts
         WHERE id = ? AND session_id = ? AND deleted_at IS NULL
         LIMIT 1",
    )
    .bind(transcript_id)
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| error.to_string())
}

async fn embeddings_from_stored_candidates<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    pool: &sqlx::SqlitePool,
    workspace_id: &str,
    transcript_id: &str,
) -> Result<Vec<SpanEmbedding>, String> {
    let candidates = anlg_db_app::list_active_voiceprint_candidates_for_transcript(
        pool,
        workspace_id,
        transcript_id,
    )
    .await
    .map_err(|error| error.to_string())?;

    let mut embeddings = Vec::new();
    for candidate in candidates {
        if candidate.model_provider != MODEL_PROVIDER || candidate.model_version != MODEL_VERSION {
            continue;
        }
        let Ok(Some(secret_value)) = tauri_plugin_store2::read_secret(
            app.clone(),
            candidate.keyring_scope.clone(),
            candidate.keyring_key.clone(),
        )
        .await
        else {
            continue;
        };
        let Some(embedding) = decode_embedding(&secret_value) else {
            continue;
        };
        embeddings.push((
            SelectedSpan {
                channel: candidate.speaker_channel,
                speaker_index: candidate.speaker_index,
                start_ms: candidate.source_start_ms,
                end_ms: candidate.source_end_ms,
                quality_score: candidate.quality_score,
            },
            embedding,
        ));
    }
    Ok(embeddings)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum NamingBasis {
    Voiceprint { score: f32 },
    Elimination,
}

#[derive(Debug, Clone, PartialEq)]
struct SpeakerNaming {
    speaker: VoiceprintSpeakerKey,
    human_id: String,
    basis: NamingBasis,
}

impl SpeakerNaming {
    fn from_voiceprint(assignment: VoiceprintAssignment) -> Self {
        Self {
            speaker: assignment.speaker,
            human_id: assignment.human_id,
            basis: NamingBasis::Voiceprint {
                score: assignment.score,
            },
        }
    }
}

/// Once voiceprints have named all remote speakers but one, and the remote
/// channel holds exactly one speaker per remote participant, the remaining
/// pair is implied. Existing hints count as named; a speaker with no clean
/// speech to embed still counts as a speaker.
fn eliminate_last_remote_speaker(
    index: &SpeakerIndex,
    remote_human_ids: &[String],
    namings: &[SpeakerNaming],
) -> Option<SpeakerNaming> {
    let mut remote_speakers: Vec<VoiceprintSpeakerKey> = index
        .first_word_id
        .keys()
        .filter(|speaker| speaker.channel == REMOTE_CHANNEL)
        .copied()
        .collect();
    remote_speakers.sort_by_key(|speaker| speaker.speaker_index);
    let assigned_speakers: Vec<VoiceprintSpeakerKey> = index
        .assigned_speakers
        .iter()
        .copied()
        .chain(namings.iter().map(|naming| naming.speaker))
        .collect();
    let assigned_people: Vec<&str> = index
        .assigned_human_ids
        .iter()
        .map(String::as_str)
        .chain(namings.iter().map(|naming| naming.human_id.as_str()))
        .collect();
    complete_by_elimination(
        &remote_speakers,
        remote_human_ids,
        &assigned_speakers,
        &assigned_people,
    )
    .map(|(speaker, human_id)| SpeakerNaming {
        speaker,
        human_id,
        basis: NamingBasis::Elimination,
    })
}

fn assignment_hints_json(
    hints_json: &str,
    index: &SpeakerIndex,
    namings: &[SpeakerNaming],
) -> Result<Option<String>, String> {
    let Ok(mut hints) = serde_json::from_str::<Vec<serde_json::Value>>(hints_json) else {
        return Ok(None);
    };
    let mut assigned = 0u32;
    for naming in namings {
        if index.assigned_speakers.contains(&naming.speaker)
            || index.assigned_human_ids.contains(&naming.human_id)
        {
            continue;
        }
        let Some(word_id) = index.first_word_id.get(&naming.speaker) else {
            continue;
        };
        hints.push(automatic_speaker_hint(
            word_id,
            &naming.human_id,
            naming.basis,
        ));
        assigned += 1;
    }
    if assigned == 0 {
        return Ok(None);
    }
    serde_json::to_string(&hints)
        .map(Some)
        .map_err(|error| error.to_string())
}

async fn write_speaker_assignment_hints(
    pool: &sqlx::SqlitePool,
    session_id: &str,
    transcript_id: &str,
    words_json: &str,
    hints_json: &str,
    next_json: &str,
) -> Result<bool, String> {
    let now = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();
    let result = sqlx::query(
        "UPDATE transcripts
         SET speaker_hints_json = ?, updated_at = ?
         WHERE id = ? AND session_id = ? AND words_json = ? AND speaker_hints_json = ?
           AND deleted_at IS NULL",
    )
    .bind(next_json)
    .bind(&now)
    .bind(transcript_id)
    .bind(session_id)
    .bind(words_json)
    .bind(hints_json)
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?;
    Ok(result.rows_affected() > 0)
}

#[derive(Debug, Default)]
struct SpeakerIndex {
    first_word_id: HashMap<VoiceprintSpeakerKey, String>,
    assigned_speakers: HashSet<VoiceprintSpeakerKey>,
    assigned_human_ids: HashSet<String>,
}

fn embeddings_match_current_speakers(embeddings: &[SpanEmbedding], index: &SpeakerIndex) -> bool {
    !embeddings.is_empty()
        && embeddings.iter().all(|(span, _)| {
            index.first_word_id.contains_key(&VoiceprintSpeakerKey {
                channel: span.channel,
                speaker_index: span.speaker_index,
            })
        })
}

fn speaker_index_from_transcript(
    words_json: &str,
    hints_json: &str,
) -> Result<SpeakerIndex, String> {
    let words: Vec<StoredWord> =
        serde_json::from_str(words_json).map_err(|error| error.to_string())?;
    let hints: Vec<StoredHint> = serde_json::from_str(hints_json).unwrap_or_default();
    let hinted_speaker_by_word = provider_speaker_index_by_word(&hints);

    let mut ordered: Vec<&StoredWord> = words.iter().collect();
    ordered.sort_by(|left, right| left.start_ms.total_cmp(&right.start_ms));

    let mut first_word_id = HashMap::new();
    let mut speaker_by_word_id = HashMap::new();
    for word in ordered {
        let Some(word_id) = word.id.as_ref() else {
            continue;
        };
        let speaker = speaker_key_for_word(word, hinted_speaker_by_word.get(word_id).copied());
        speaker_by_word_id.insert(word_id.clone(), speaker);
        first_word_id
            .entry(speaker)
            .or_insert_with(|| word_id.clone());
    }

    let mut assigned_speakers = HashSet::new();
    let mut assigned_human_ids = HashSet::new();
    for hint in &hints {
        if hint.hint_type != "automatic_speaker_assignment"
            && hint.hint_type != "user_speaker_assignment"
        {
            continue;
        }
        let Some(human_id) = human_id_from_hint_value(&hint.value) else {
            continue;
        };
        assigned_human_ids.insert(human_id);
        if let Some(speaker) = speaker_key_from_hint_value(&hint.value)
            .or_else(|| speaker_by_word_id.get(&hint.word_id).copied())
        {
            assigned_speakers.insert(speaker);
        }
    }

    Ok(SpeakerIndex {
        first_word_id,
        assigned_speakers,
        assigned_human_ids,
    })
}

fn provider_speaker_index_by_word(hints: &[StoredHint]) -> HashMap<String, i64> {
    hints
        .iter()
        .filter(|hint| hint.hint_type == "provider_speaker_index")
        .filter_map(|hint| {
            speaker_index_from_hint_value(&hint.value).map(|index| (hint.word_id.clone(), index))
        })
        .collect()
}

fn speaker_key_for_word(word: &StoredWord, hinted_index: Option<i64>) -> VoiceprintSpeakerKey {
    VoiceprintSpeakerKey {
        channel: word.channel,
        speaker_index: hinted_index.or(word.speaker_index),
    }
}

fn speaker_key_from_hint_value(value: &serde_json::Value) -> Option<VoiceprintSpeakerKey> {
    let object = hint_value_object(value)?;
    let channel = object.get("channel")?.as_i64()?;
    Some(VoiceprintSpeakerKey {
        channel,
        speaker_index: object.get("speaker_index").and_then(|value| value.as_i64()),
    })
}

fn human_id_from_hint_value(value: &serde_json::Value) -> Option<String> {
    let object = hint_value_object(value)?;
    object
        .get("human_id")?
        .as_str()
        .filter(|human_id| !human_id.is_empty())
        .map(str::to_string)
}

fn hint_value_object(value: &serde_json::Value) -> Option<serde_json::Value> {
    match value {
        serde_json::Value::String(raw) => serde_json::from_str(raw).ok(),
        other => Some(other.clone()),
    }
}

fn automatic_speaker_hint(word_id: &str, human_id: &str, basis: NamingBasis) -> serde_json::Value {
    let value = match basis {
        NamingBasis::Voiceprint { score } => serde_json::json!({
            "human_id": human_id,
            "confidence": score,
            "source": "voiceprint",
        }),
        NamingBasis::Elimination => serde_json::json!({
            "human_id": human_id,
            "source": "elimination",
        }),
    };
    serde_json::json!({
        "id": format!("{word_id}:automatic_speaker_assignment"),
        "word_id": word_id,
        "type": "automatic_speaker_assignment",
        "value": value.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::num::NonZero;
    use std::time::Duration;

    use super::*;

    #[test]
    fn direct_mic_speakers_dedupes_and_ignores_other_channels() {
        let span = |channel: i64, speaker_index: Option<i64>| {
            (
                SelectedSpan {
                    channel,
                    speaker_index,
                    start_ms: 0,
                    end_ms: 2_000,
                    quality_score: 0.5,
                },
                vec![0.0_f32; 4],
            )
        };
        let embeddings = vec![
            span(0, None),
            span(1, Some(0)),
            span(0, Some(2)),
            span(0, None),
            span(2, None),
        ];

        assert_eq!(direct_mic_speakers(&embeddings), vec![None, Some(2)]);
        assert!(direct_mic_speakers(&[span(1, Some(0))]).is_empty());
    }

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
    fn streamed_span_samples_and_embeddings_match_full_audio_extraction() {
        let input_rate = 44_100u32;
        let input_frames = input_rate as usize * 3;
        let audio = (0..input_frames)
            .flat_map(|frame| {
                let phase = frame as f32 * 0.013;
                [phase.sin() * 0.2, phase.cos() * 0.3]
            })
            .collect::<Vec<_>>();
        let spans = vec![
            SelectedSpan {
                channel: 1,
                speaker_index: Some(0),
                start_ms: 400,
                end_ms: 1_900,
                quality_score: 0.15,
            },
            SelectedSpan {
                channel: 2,
                speaker_index: Some(1),
                start_ms: 1_200,
                end_ms: 2_700,
                quality_score: 0.15,
            },
        ];

        let full_channels = anlg_audio_utils::resample_audio_channels(
            rodio::buffer::SamplesBuffer::new(
                NonZero::new(2).unwrap(),
                NonZero::new(input_rate).unwrap(),
                audio.clone(),
            ),
            EMBEDDING_SAMPLE_RATE,
        )
        .unwrap();
        let expected = spans
            .iter()
            .map(|span| span_samples(&full_channels, span).unwrap())
            .collect::<Vec<_>>();
        let (streamed, stats) = collect_span_samples(
            rodio::buffer::SamplesBuffer::new(
                NonZero::new(2).unwrap(),
                NonZero::new(input_rate).unwrap(),
                audio,
            ),
            spans,
        )
        .unwrap();

        assert_eq!(streamed.len(), expected.len());
        for (streamed, expected) in streamed.iter().zip(&expected) {
            assert_eq!(streamed.samples, *expected);
        }
        assert_eq!(
            stats.retained_samples,
            expected.iter().map(Vec::len).sum::<usize>()
        );
        assert!(stats.max_decoder_block_samples < stats.decoded_frames * 2);

        let mut streamed_extractor = anlg_embedding::EmbeddingExtractor::new().unwrap();
        let mut full_extractor = anlg_embedding::EmbeddingExtractor::new().unwrap();
        let streamed_embedding = streamed_extractor.compute(&streamed[0].samples).unwrap();
        let full_embedding = full_extractor.compute(&expected[0]).unwrap();
        assert_eq!(streamed_embedding.len(), full_embedding.len());
        assert!(
            streamed_embedding
                .iter()
                .zip(full_embedding)
                .all(|(streamed, full)| (streamed - full).abs() < 1e-6)
        );
    }

    #[test]
    #[ignore = "long synthetic-audio memory benchmark"]
    fn thirty_minute_span_collection_retains_only_selected_audio() {
        let duration_seconds = 30 * 60usize;
        let spans = vec![
            SelectedSpan {
                channel: 0,
                speaker_index: Some(0),
                start_ms: 10_000,
                end_ms: 20_000,
                quality_score: 1.0,
            },
            SelectedSpan {
                channel: 1,
                speaker_index: Some(1),
                start_ms: 1_780_000,
                end_ms: 1_790_000,
                quality_score: 1.0,
            },
        ];
        let (collected, stats) =
            collect_span_samples(SyntheticStereoSource::new(duration_seconds), spans).unwrap();

        assert_eq!(
            stats.decoded_frames,
            duration_seconds * EMBEDDING_SAMPLE_RATE as usize
        );
        assert_eq!(
            collected[0].samples.len(),
            10 * EMBEDDING_SAMPLE_RATE as usize
        );
        assert_eq!(
            collected[1].samples.len(),
            10 * EMBEDDING_SAMPLE_RATE as usize
        );
        assert_eq!(stats.retained_samples, 20 * EMBEDDING_SAMPLE_RATE as usize);
        assert!(stats.max_decoder_block_samples <= 2 * 1024);
    }

    #[test]
    fn embedding_round_trips_through_base64() {
        let embedding = vec![0.5_f32, -1.25, 3.0];
        assert_eq!(
            decode_embedding(&encode_embedding(&embedding)),
            Some(embedding)
        );
        assert_eq!(decode_embedding("not-base64"), None);
        assert_eq!(decode_embedding(""), None);
    }

    #[test]
    fn speaker_index_reads_existing_assignments() {
        let words = r#"[
            {"id": "w1", "text": "hello", "start_ms": 0, "end_ms": 2000, "channel": 1, "speaker_index": 0},
            {"id": "w2", "text": "there", "start_ms": 2100, "end_ms": 4000, "channel": 1, "speaker_index": 1}
        ]"#;
        let hints = r#"[
            {"word_id": "w1", "type": "automatic_speaker_assignment", "value": {"human_id": "marco"}}
        ]"#;

        let index = speaker_index_from_transcript(words, hints).unwrap();
        assert_eq!(
            index.first_word_id.get(&VoiceprintSpeakerKey {
                channel: 1,
                speaker_index: Some(0)
            }),
            Some(&"w1".to_string())
        );
        assert!(index.assigned_human_ids.contains("marco"));
        assert!(index.assigned_speakers.contains(&VoiceprintSpeakerKey {
            channel: 1,
            speaker_index: Some(0)
        }));
        assert!(!index.assigned_speakers.contains(&VoiceprintSpeakerKey {
            channel: 1,
            speaker_index: Some(1)
        }));
    }

    #[test]
    fn stored_embeddings_require_current_speaker_keys() {
        let words = r#"[
            {"id": "w1", "text": "hello", "start_ms": 0, "end_ms": 2000, "channel": 1, "speaker_index": 0}
        ]"#;
        let index = speaker_index_from_transcript(words, "[]").unwrap();
        let matching = vec![(
            SelectedSpan {
                channel: 1,
                speaker_index: Some(0),
                start_ms: 0,
                end_ms: 2000,
                quality_score: 0.1,
            },
            vec![0.1_f32],
        )];
        let remapped = vec![(
            SelectedSpan {
                channel: 1,
                speaker_index: Some(9),
                start_ms: 0,
                end_ms: 2000,
                quality_score: 0.1,
            },
            vec![0.1_f32],
        )];

        assert!(embeddings_match_current_speakers(&matching, &index));
        assert!(!embeddings_match_current_speakers(&remapped, &index));
        assert!(!embeddings_match_current_speakers(&[], &index));
    }

    #[test]
    fn unique_voiceprint_hint_uses_voiceprint_source() {
        let hint = automatic_speaker_hint("w1", "marco", NamingBasis::Voiceprint { score: 0.84 });
        assert_eq!(hint["type"], "automatic_speaker_assignment");
        assert_eq!(hint["word_id"], "w1");
        let value: serde_json::Value =
            serde_json::from_str(hint["value"].as_str().unwrap()).unwrap();
        assert_eq!(value["human_id"], "marco");
        assert_eq!(value["source"], "voiceprint");
        assert!((value["confidence"].as_f64().unwrap() - 0.84).abs() < 1e-6);
    }

    #[test]
    fn elimination_hint_carries_its_own_source_and_no_score() {
        let hint = automatic_speaker_hint("w2", "ada", NamingBasis::Elimination);
        assert_eq!(hint["type"], "automatic_speaker_assignment");
        let value: serde_json::Value =
            serde_json::from_str(hint["value"].as_str().unwrap()).unwrap();
        assert_eq!(value["human_id"], "ada");
        assert_eq!(value["source"], "elimination");
        assert!(value.get("confidence").is_none());
    }

    fn remote_speaker(index: i64) -> VoiceprintSpeakerKey {
        VoiceprintSpeakerKey {
            channel: REMOTE_CHANNEL,
            speaker_index: Some(index),
        }
    }

    fn voiceprint_naming(speaker: VoiceprintSpeakerKey, human_id: &str) -> SpeakerNaming {
        SpeakerNaming {
            speaker,
            human_id: human_id.to_string(),
            basis: NamingBasis::Voiceprint { score: 0.9 },
        }
    }

    #[test]
    fn assignment_hints_skip_speakers_that_are_already_named() {
        let words = r#"[
            {"id": "w1", "text": "hello", "start_ms": 0, "end_ms": 2000, "channel": 1, "speaker_index": 0},
            {"id": "w2", "text": "there", "start_ms": 2100, "end_ms": 4000, "channel": 1, "speaker_index": 1}
        ]"#;
        let hints = r#"[
            {"word_id": "w1", "type": "automatic_speaker_assignment", "value": {"human_id": "marco"}}
        ]"#;
        let index = speaker_index_from_transcript(words, hints).unwrap();
        let next = assignment_hints_json(
            hints,
            &index,
            &[
                voiceprint_naming(remote_speaker(0), "marco"),
                voiceprint_naming(remote_speaker(1), "ada"),
            ],
        )
        .unwrap()
        .unwrap();

        let parsed: Vec<serde_json::Value> = serde_json::from_str(&next).unwrap();
        let assigned: Vec<_> = parsed
            .iter()
            .filter(|hint| hint["type"] == "automatic_speaker_assignment")
            .map(|hint| hint["word_id"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(assigned, vec!["w1".to_string(), "w2".to_string()]);
    }

    const THREE_REMOTE_SPEAKERS: &str = r#"[
        {"id": "w1", "text": "hello", "start_ms": 0, "end_ms": 2000, "channel": 1, "speaker_index": 0},
        {"id": "w2", "text": "there", "start_ms": 2100, "end_ms": 4000, "channel": 1, "speaker_index": 1},
        {"id": "w3", "text": "hi", "start_ms": 4100, "end_ms": 4400, "channel": 1, "speaker_index": 2},
        {"id": "w4", "text": "yes", "start_ms": 4500, "end_ms": 5000, "channel": 0, "speaker_index": null}
    ]"#;

    fn people(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| id.to_string()).collect()
    }

    #[test]
    fn elimination_names_the_last_remote_speaker_from_prior_hints_and_new_matches() {
        // Marco was named by hand earlier; Ada was just matched by voiceprint.
        // Speaker 2 never had enough clean speech to embed, but three remote
        // speakers for three remote participants leaves only Cy.
        let hints = r#"[
            {"word_id": "w1", "type": "user_speaker_assignment", "value": {"human_id": "marco"}}
        ]"#;
        let index = speaker_index_from_transcript(THREE_REMOTE_SPEAKERS, hints).unwrap();
        let namings = vec![voiceprint_naming(remote_speaker(1), "ada")];

        let naming =
            eliminate_last_remote_speaker(&index, &people(&["ada", "marco", "cy"]), &namings);

        assert_eq!(
            naming,
            Some(SpeakerNaming {
                speaker: remote_speaker(2),
                human_id: "cy".to_string(),
                basis: NamingBasis::Elimination,
            })
        );
    }

    #[test]
    fn elimination_ignores_the_direct_mic_and_refuses_on_count_mismatch() {
        let index = speaker_index_from_transcript(THREE_REMOTE_SPEAKERS, "[]").unwrap();
        let namings = vec![
            voiceprint_naming(remote_speaker(0), "marco"),
            voiceprint_naming(remote_speaker(1), "ada"),
        ];

        // Four invited remotes, three remote speakers: someone did not show.
        assert_eq!(
            eliminate_last_remote_speaker(&index, &people(&["ada", "marco", "cy", "di"]), &namings),
            None
        );
        // Two invited remotes, three remote speakers: someone extra joined.
        assert_eq!(
            eliminate_last_remote_speaker(&index, &people(&["ada", "marco"]), &namings),
            None
        );
        // Exact count: the direct-mic word is not a remote speaker.
        assert_eq!(
            eliminate_last_remote_speaker(&index, &people(&["ada", "marco", "cy"]), &namings)
                .map(|naming| naming.human_id),
            Some("cy".to_string())
        );
    }

    #[test]
    fn elimination_needs_a_single_open_pair() {
        let index = speaker_index_from_transcript(THREE_REMOTE_SPEAKERS, "[]").unwrap();

        assert_eq!(
            eliminate_last_remote_speaker(
                &index,
                &people(&["ada", "marco", "cy"]),
                &[voiceprint_naming(remote_speaker(0), "marco")],
            ),
            None
        );
    }

    struct SyntheticStereoSource {
        sample_index: usize,
        total_samples: usize,
    }

    impl SyntheticStereoSource {
        fn new(duration_seconds: usize) -> Self {
            Self {
                sample_index: 0,
                total_samples: duration_seconds * EMBEDDING_SAMPLE_RATE as usize * 2,
            }
        }
    }

    impl Iterator for SyntheticStereoSource {
        type Item = f32;

        fn next(&mut self) -> Option<Self::Item> {
            if self.sample_index >= self.total_samples {
                return None;
            }
            let frame = self.sample_index / 2;
            let channel = self.sample_index % 2;
            self.sample_index += 1;
            Some(((frame + channel * 17) as f32 * 0.001).sin() * 0.1)
        }
    }

    impl anlg_audio_utils::Source for SyntheticStereoSource {
        fn current_span_len(&self) -> Option<usize> {
            Some(self.total_samples - self.sample_index)
        }

        fn channels(&self) -> NonZero<u16> {
            NonZero::new(2).unwrap()
        }

        fn sample_rate(&self) -> NonZero<u32> {
            NonZero::new(EMBEDDING_SAMPLE_RATE).unwrap()
        }

        fn total_duration(&self) -> Option<Duration> {
            Some(Duration::from_secs(
                (self.total_samples / 2 / EMBEDDING_SAMPLE_RATE as usize) as u64,
            ))
        }
    }
}
