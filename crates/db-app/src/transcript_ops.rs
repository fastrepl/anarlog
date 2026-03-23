use hypr_transcript::{FinalizedWord, SpeakerHintData, WordState};
use sqlx::SqlitePool;

use crate::{PersistableSpeakerHint, TranscriptDeltaPersist};

pub async fn apply_delta(
    pool: &SqlitePool,
    meeting_id: &str,
    delta: &TranscriptDeltaPersist,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    for id in &delta.replaced_ids {
        sqlx::query("DELETE FROM speaker_hints WHERE word_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM words WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }

    for w in &delta.new_words {
        let state_str = match w.state {
            WordState::Final => "final",
            WordState::Pending => "pending",
        };
        sqlx::query(
            "INSERT OR REPLACE INTO words (id, meeting_id, text, start_ms, end_ms, channel, state) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&w.id)
        .bind(meeting_id)
        .bind(&w.text)
        .bind(w.start_ms)
        .bind(w.end_ms)
        .bind(w.channel)
        .bind(state_str)
        .execute(&mut *tx)
        .await?;
    }

    for h in &delta.hints {
        let (kind, speaker_index, provider, channel, human_id) = match &h.data {
            SpeakerHintData::ProviderSpeakerIndex {
                speaker_index,
                provider,
                channel,
            } => (
                "provider_speaker_index",
                Some(*speaker_index),
                provider.as_deref(),
                *channel,
                None,
            ),
            SpeakerHintData::UserSpeakerAssignment { human_id } => (
                "user_speaker_assignment",
                None,
                None,
                None,
                Some(human_id.as_str()),
            ),
        };
        let hint_id = format!("{meeting_id}:{}:{kind}", h.word_id);
        sqlx::query(
            "INSERT OR REPLACE INTO speaker_hints (id, meeting_id, word_id, kind, speaker_index, provider, channel, human_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&hint_id)
        .bind(meeting_id)
        .bind(&h.word_id)
        .bind(kind)
        .bind(speaker_index)
        .bind(provider)
        .bind(channel)
        .bind(human_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn load_words(
    pool: &SqlitePool,
    meeting_id: &str,
) -> Result<Vec<FinalizedWord>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, i64, i64, i32, String)>(
        "SELECT id, text, start_ms, end_ms, channel, state FROM words WHERE meeting_id = ? ORDER BY start_ms",
    )
    .bind(meeting_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, text, start_ms, end_ms, channel, state)| {
            let state = match state.as_str() {
                "pending" => WordState::Pending,
                _ => WordState::Final,
            };
            FinalizedWord {
                id,
                text,
                start_ms,
                end_ms,
                channel,
                state,
            }
        })
        .collect())
}

pub async fn load_hints(
    pool: &SqlitePool,
    meeting_id: &str,
) -> Result<Vec<PersistableSpeakerHint>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (String, String, Option<i32>, Option<String>, Option<i32>, Option<String>)>(
        "SELECT word_id, kind, speaker_index, provider, channel, human_id FROM speaker_hints WHERE meeting_id = ? ORDER BY word_id",
    )
    .bind(meeting_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(
            |(word_id, kind, speaker_index, provider, channel, human_id)| {
                let data = match kind.as_str() {
                    "provider_speaker_index" => SpeakerHintData::ProviderSpeakerIndex {
                        speaker_index: speaker_index.unwrap_or(0),
                        provider,
                        channel,
                    },
                    "user_speaker_assignment" => SpeakerHintData::UserSpeakerAssignment {
                        human_id: human_id.unwrap_or_default(),
                    },
                    _ => return None,
                };
                Some(PersistableSpeakerHint { word_id, data })
            },
        )
        .collect())
}
