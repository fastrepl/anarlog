use hypr_transcript::{RuntimeSpeakerHint, WordRef};
use sqlx::SqlitePool;
use tokio::sync::mpsc;

pub enum RuntimeEvent {
    Loaded {
        meeting: hypr_db_app::MeetingRow,
        segments: Vec<hypr_transcript::Segment>,
        memo: Option<hypr_db_app::NoteRow>,
    },
    LoadError(String),
    Saved,
    SaveError(String),
}

pub struct Runtime {
    pool: SqlitePool,
    tx: mpsc::UnboundedSender<RuntimeEvent>,
}

impl Runtime {
    pub fn new(pool: SqlitePool, tx: mpsc::UnboundedSender<RuntimeEvent>) -> Self {
        Self { pool, tx }
    }

    pub fn load(&self, meeting_id: String) {
        let pool = self.pool.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            match load_meeting_data(&pool, &meeting_id).await {
                Ok((meeting, segments, memo)) => {
                    let _ = tx.send(RuntimeEvent::Loaded {
                        meeting,
                        segments,
                        memo,
                    });
                }
                Err(e) => {
                    let _ = tx.send(RuntimeEvent::LoadError(e));
                }
            }
        });
    }

    pub fn save_memo(&self, meeting_id: String, memo: String) {
        let pool = self.pool.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            match do_save_memo(&pool, &meeting_id, &memo).await {
                Ok(()) => {
                    let _ = tx.send(RuntimeEvent::Saved);
                }
                Err(e) => {
                    let _ = tx.send(RuntimeEvent::SaveError(e));
                }
            }
        });
    }
}

async fn load_meeting_data(
    pool: &SqlitePool,
    meeting_id: &str,
) -> Result<
    (
        hypr_db_app::MeetingRow,
        Vec<hypr_transcript::Segment>,
        Option<hypr_db_app::NoteRow>,
    ),
    String,
> {
    let meeting = hypr_db_app::get_meeting(pool, meeting_id)
        .await
        .map_err(|e| format!("query failed: {e}"))?
        .ok_or_else(|| format!("meeting not found: {meeting_id}"))?;

    let words = hypr_db_app::load_words(pool, meeting_id)
        .await
        .map_err(|e| format!("load words failed: {e}"))?;

    let hints = hypr_db_app::load_hints(pool, meeting_id)
        .await
        .map_err(|e| format!("load hints failed: {e}"))?;

    let memo = hypr_db_app::get_note_by_meeting_and_kind(pool, meeting_id, "memo")
        .await
        .map_err(|e| format!("load memo failed: {e}"))?;

    let runtime_hints: Vec<RuntimeSpeakerHint> = hints
        .into_iter()
        .map(|h| RuntimeSpeakerHint {
            target: WordRef::FinalWordId(h.word_id),
            data: h.data,
        })
        .collect();

    let segments = hypr_transcript::build_segments(&words, &[], &runtime_hints, None);

    Ok((meeting, segments, memo))
}

async fn do_save_memo(pool: &SqlitePool, meeting_id: &str, memo: &str) -> Result<(), String> {
    let existing = hypr_db_app::get_note_by_meeting_and_kind(pool, meeting_id, "memo")
        .await
        .map_err(|e| format!("query failed: {e}"))?;

    match existing {
        Some(note) => {
            hypr_db_app::update_note(pool, &note.id, memo)
                .await
                .map_err(|e| format!("update failed: {e}"))?;
        }
        None => {
            let note_id = format!("{meeting_id}:memo");
            hypr_db_app::insert_note(pool, &note_id, meeting_id, "memo", "", memo)
                .await
                .map_err(|e| format!("insert failed: {e}"))?;
        }
    }

    Ok(())
}
