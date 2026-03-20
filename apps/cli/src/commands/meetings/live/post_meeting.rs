use sqlx::SqlitePool;
use tokio::sync::mpsc;

use hypr_db_app::{PersistableSpeakerHint, TranscriptDeltaPersist};
use hypr_transcript::{FinalizedWord, RuntimeSpeakerHint, WordRef};

use crate::llm::ResolvedLlmConfig;

use super::exit::{AUTO_EXIT_DELAY, ExitEvent};

pub fn to_persistable_hints(hints: &[RuntimeSpeakerHint]) -> Vec<PersistableSpeakerHint> {
    hints
        .iter()
        .filter_map(|hint| match &hint.target {
            WordRef::FinalWordId(word_id) => Some(PersistableSpeakerHint {
                word_id: word_id.clone(),
                data: hint.data.clone(),
            }),
            WordRef::RuntimeIndex(_) => None,
        })
        .collect()
}

fn title_from_summary(summary: &str) -> String {
    let first_sentence = summary
        .split_terminator(['.', '!', '?'])
        .next()
        .unwrap_or(summary);
    let trimmed = first_sentence.trim();
    if trimmed.len() <= 80 {
        trimmed.to_string()
    } else {
        let mut end = 80;
        while !trimmed.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &trimmed[..end])
    }
}

fn words_to_transcript_text(words: &[FinalizedWord]) -> String {
    words
        .iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn spawn_post_meeting(
    llm_config: Result<ResolvedLlmConfig, String>,
    tx: mpsc::UnboundedSender<ExitEvent>,
    words: Vec<FinalizedWord>,
    hints: Vec<PersistableSpeakerHint>,
    memo_text: String,
    meeting_id: String,
    event_id: Option<String>,
    pool: SqlitePool,
) {
    tokio::spawn(async move {
        run_post_meeting(
            &tx,
            llm_config,
            words,
            hints,
            memo_text,
            &meeting_id,
            event_id.as_deref(),
            &pool,
        )
        .await;
        let _ = tx.send(ExitEvent::AllDone);
        tokio::time::sleep(AUTO_EXIT_DELAY).await;
        let _ = tx.send(ExitEvent::AutoExit);
    });
}

async fn save_to_db(
    pool: &SqlitePool,
    meeting_id: &str,
    event_id: Option<&str>,
    words: Vec<FinalizedWord>,
    hints: Vec<PersistableSpeakerHint>,
    memo_text: &str,
) -> Result<(), String> {
    hypr_db_app::insert_meeting(pool, meeting_id, event_id)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(eid) = event_id {
        let _ = hypr_db_app::copy_event_participants_to_meeting(pool, meeting_id, eid).await;
    }
    let delta = TranscriptDeltaPersist {
        new_words: words,
        hints,
        replaced_ids: vec![],
    };
    hypr_db_app::apply_delta(pool, meeting_id, &delta)
        .await
        .map_err(|e| e.to_string())?;
    let memo = memo_text.trim();
    if !memo.is_empty() {
        let note_id = format!("{meeting_id}:memo");
        let _ = hypr_db_app::insert_note(pool, &note_id, meeting_id, "memo", "", memo).await;
    }
    Ok(())
}

async fn run_post_meeting(
    tx: &mpsc::UnboundedSender<ExitEvent>,
    llm_config: Result<ResolvedLlmConfig, String>,
    words: Vec<FinalizedWord>,
    hints: Vec<PersistableSpeakerHint>,
    memo_text: String,
    meeting_id: &str,
    event_id: Option<&str>,
    pool: &SqlitePool,
) {
    // Task 0: save to database
    let _ = tx.send(ExitEvent::TaskStarted(0));
    if let Err(e) = save_to_db(pool, meeting_id, event_id, words, hints, &memo_text).await {
        let _ = tx.send(ExitEvent::TaskFailed(0, e));
        let _ = tx.send(ExitEvent::TaskFailed(1, "database unavailable".into()));
        return;
    }
    let _ = tx.send(ExitEvent::TaskDone(0));

    // Task 1: generate summary
    let _ = tx.send(ExitEvent::TaskStarted(1));

    let transcript_text = match hypr_db_app::load_words(pool, meeting_id).await {
        Ok(words) => words_to_transcript_text(&words),
        Err(e) => {
            let _ = tx.send(ExitEvent::TaskFailed(1, e.to_string()));
            return;
        }
    };

    let config = match llm_config {
        Ok(config) => config,
        Err(msg) => {
            let _ = tx.send(ExitEvent::TaskFailed(1, msg));
            return;
        }
    };

    let backend = match crate::agent::Backend::new(config, None) {
        Ok(b) => b,
        Err(e) => {
            let _ = tx.send(ExitEvent::TaskFailed(1, e.to_string()));
            return;
        }
    };

    let prompt = format!(
        "Summarize the following meeting transcript in a few concise paragraphs. \
         Focus on key topics, decisions, and action items.\n\n{transcript_text}"
    );

    match backend
        .stream_text(prompt, vec![], 1, |_chunk| Ok(()))
        .await
    {
        Ok(Some(summary)) => {
            let _ = tx.send(ExitEvent::TaskDone(1));
            let title = title_from_summary(&summary);
            let _ = hypr_db_app::update_meeting(pool, meeting_id, Some(&title)).await;
            let note_id = format!("{meeting_id}:summary");
            let _ =
                hypr_db_app::insert_note(pool, &note_id, meeting_id, "summary", "", &summary).await;
        }
        Ok(None) => {
            let _ = tx.send(ExitEvent::TaskFailed(
                1,
                "LLM returned empty response".into(),
            ));
        }
        Err(e) => {
            let _ = tx.send(ExitEvent::TaskFailed(1, e.to_string()));
        }
    }
}
