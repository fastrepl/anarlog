use std::{
    collections::HashMap,
    sync::{Arc, Mutex as StdMutex},
    time::Duration,
};

use anlg_db_app::{AppendOutcome, LiveTranscriptInsert, append_live_transcript_deltas};
use anlg_transcript::WordState;
use anlg_transcription_core::listener::LiveTranscriptDelta;
use futures_util::future::BoxFuture;
use tauri::Manager;
use tauri_specta::Event;
use tokio::{
    sync::{Mutex, mpsc, oneshot},
    time::Instant,
};

use crate::{LiveTranscriptPersistence, LiveTranscriptPersistenceEvent, LiveTranscriptTarget};

const BATCH_WINDOW: Duration = Duration::from_millis(250);
const INITIAL_RETRY_DELAY: Duration = Duration::from_millis(500);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(5);
const RELEASE_FAILURE_TIMEOUT: Duration = Duration::from_secs(60);
const FLUSH_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_BACKLOG_WORDS: usize = 10_000;
const MAX_BACKLOG_TEXT_CHARS: usize = 1_000_000;
const MAX_BACKLOG_REPLACED_IDS: usize = 10_000;
const BACKLOG_OVERFLOW_ERROR: &str = "transcript persistence backlog overflowed";
const FLUSH_TIMEOUT_ERROR: &str = "transcript persistence flush timed out";

type JournalStoreHandle = Arc<dyn JournalStore>;
type EventSink = Arc<dyn Fn(LiveTranscriptPersistence) + Send + Sync>;

trait JournalStore: Send + Sync {
    fn append(
        &self,
        target: LiveTranscriptInsert,
        delta_jsons: Vec<String>,
    ) -> BoxFuture<'static, Result<AppendOutcome, String>>;
}

#[derive(Default)]
pub(crate) struct LiveJournalRegistry {
    sessions: StdMutex<HashMap<String, LiveTranscriptJournal>>,
}

impl LiveJournalRegistry {
    fn register(
        &self,
        session_id: String,
        target: LiveTranscriptTarget,
        store: JournalStoreHandle,
        event_sink: EventSink,
    ) -> Result<(LiveTranscriptJournal, Option<LiveTranscriptJournal>), String> {
        let journal = LiveTranscriptJournal::spawn(session_id.clone(), target, store, event_sink);
        let previous = self
            .sessions
            .lock()
            .map_err(|error| error.to_string())?
            .insert(session_id, journal.clone());
        Ok((journal, previous))
    }

    pub(crate) fn rollback_registration(
        &self,
        session_id: &str,
        registered: &LiveTranscriptJournal,
        previous: Option<LiveTranscriptJournal>,
    ) -> Result<(), String> {
        let result = self
            .sessions
            .lock()
            .map_err(|error| error.to_string())
            .map(|mut sessions| {
                if sessions
                    .get(session_id)
                    .is_some_and(|journal| journal.is_same_instance(registered))
                {
                    match previous {
                        Some(previous) => {
                            sessions.insert(session_id.to_string(), previous);
                        }
                        None => {
                            sessions.remove(session_id);
                        }
                    }
                }
            });
        registered.release();
        result
    }

    pub(crate) fn append(&self, session_id: &str, delta: LiveTranscriptDelta) {
        let journal = self
            .sessions
            .lock()
            .ok()
            .and_then(|sessions| sessions.get(session_id).cloned());
        if let Some(journal) = journal {
            journal.append(delta);
        }
    }

    pub(crate) fn get(&self, session_id: &str) -> Result<Option<LiveTranscriptJournal>, String> {
        self.sessions
            .lock()
            .map(|sessions| sessions.get(session_id).cloned())
            .map_err(|error| error.to_string())
    }

    pub(crate) fn release(&self, session_id: &str, transcript_id: &str) -> Result<(), String> {
        let removed = {
            let mut sessions = self.sessions.lock().map_err(|error| error.to_string())?;
            if sessions
                .get(session_id)
                .is_some_and(|journal| journal.transcript_id == transcript_id)
            {
                sessions.remove(session_id)
            } else {
                None
            }
        };
        if let Some(journal) = removed {
            journal.release();
        }
        Ok(())
    }

    pub(crate) fn release_session(&self, session_id: &str) -> Result<(), String> {
        let removed = self
            .sessions
            .lock()
            .map_err(|error| error.to_string())?
            .remove(session_id);
        if let Some(journal) = removed {
            journal.release();
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct LiveTranscriptJournal {
    transcript_id: String,
    sender: mpsc::UnboundedSender<WriterMessage>,
    status: Arc<Mutex<LiveTranscriptPersistence>>,
    backlog: Arc<StdMutex<BacklogTracker>>,
    event_sink: EventSink,
}

impl LiveTranscriptJournal {
    fn spawn(
        session_id: String,
        target: LiveTranscriptTarget,
        store: JournalStoreHandle,
        event_sink: EventSink,
    ) -> Self {
        let transcript_id = target.transcript_id.clone();
        let status = Arc::new(Mutex::new(LiveTranscriptPersistence {
            session_id,
            transcript_id: transcript_id.clone(),
            transcript_created: false,
            persisted_through_ms: None,
            error: None,
        }));
        let backlog = Arc::new(StdMutex::new(BacklogTracker::default()));
        let (sender, receiver) = mpsc::unbounded_channel();
        tokio::spawn(run_writer(
            target,
            store,
            event_sink.clone(),
            status.clone(),
            backlog.clone(),
            receiver,
        ));
        Self {
            transcript_id,
            sender,
            status,
            backlog,
            event_sink,
        }
    }

    pub(crate) fn append(&self, delta: LiveTranscriptDelta) {
        if delta.new_words.is_empty() && delta.replaced_ids.is_empty() {
            return;
        }

        let metrics = BacklogMetrics::from_delta(&delta);
        let overflowed = {
            let Ok(mut backlog) = self.backlog.lock() else {
                return;
            };
            if backlog.overflowed {
                return;
            }
            backlog.metrics.add(metrics);
            if backlog.metrics.exceeds_bounds() {
                backlog.overflowed = true;
                backlog.metrics = BacklogMetrics::default();
                true
            } else {
                false
            }
        };

        if overflowed {
            let _ = self.sender.send(WriterMessage::Overflow);
        } else if let Err(error) = self.sender.send(WriterMessage::Delta(delta))
            && let WriterMessage::Delta(delta) = error.0
            && let Ok(mut backlog) = self.backlog.lock()
        {
            backlog.metrics.subtract(BacklogMetrics::from_delta(&delta));
        }
    }

    pub(crate) async fn flush(&self) -> Result<LiveTranscriptPersistence, String> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(WriterMessage::Flush(reply))
            .map_err(|error| error.to_string())?;

        match tokio::time::timeout(FLUSH_TIMEOUT, response).await {
            Ok(Ok(status)) => Ok(status),
            Ok(Err(error)) => Err(error.to_string()),
            Err(_) => {
                let status = {
                    let mut status = self.status.lock().await;
                    status.error = Some(FLUSH_TIMEOUT_ERROR.to_string());
                    status.clone()
                };
                (self.event_sink)(status.clone());
                Ok(status)
            }
        }
    }

    fn is_same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.status, &other.status)
    }

    pub(crate) fn release(&self) {
        let _ = self.sender.send(WriterMessage::Release);
    }
}

pub(crate) fn register_app_journal<R: tauri::Runtime>(
    registry: &LiveJournalRegistry,
    app: tauri::AppHandle<R>,
    session_id: String,
    target: LiveTranscriptTarget,
) -> Result<(LiveTranscriptJournal, Option<LiveTranscriptJournal>), String> {
    let store: JournalStoreHandle = Arc::new(AppJournalStore(app.clone()));
    let event_app = app;
    let event_sink: EventSink = Arc::new(move |status| {
        if let Some(registry) = event_app.try_state::<crate::CaptureGapRegistry>() {
            if status.error.is_some() {
                registry.persistence_failed(&status.session_id, crate::capture_gaps::unix_now_ms());
            } else if let Some(persisted_through_ms) = status.persisted_through_ms {
                registry.persisted_through(&status.session_id, persisted_through_ms);
            }
        }
        if let Err(error) = (LiveTranscriptPersistenceEvent { status }).emit(&event_app) {
            tracing::error!(?error, "failed_to_emit_live_transcript_persistence_event");
        }
    });
    registry.register(session_id, target, store, event_sink)
}

struct AppJournalStore<R: tauri::Runtime>(tauri::AppHandle<R>);

impl<R: tauri::Runtime> JournalStore for AppJournalStore<R> {
    fn append(
        &self,
        target: LiveTranscriptInsert,
        delta_jsons: Vec<String>,
    ) -> BoxFuture<'static, Result<AppendOutcome, String>> {
        let app = self.0.clone();
        Box::pin(async move {
            let runtime = app
                .try_state::<tauri_plugin_db::ManagedState>()
                .map(|state| state.inner().clone())
                .ok_or_else(|| "database is not ready yet".to_string())?;
            let _guard = runtime.synced_write_guard().await;
            append_live_transcript_deltas(runtime.pool(), &target, &delta_jsons)
                .await
                .map_err(|error| error.to_string())
        })
    }
}

enum WriterMessage {
    Delta(LiveTranscriptDelta),
    Overflow,
    Flush(oneshot::Sender<LiveTranscriptPersistence>),
    Release,
}

#[derive(Default)]
struct BacklogTracker {
    metrics: BacklogMetrics,
    overflowed: bool,
}

#[derive(Clone, Copy, Default)]
struct BacklogMetrics {
    words: usize,
    text_chars: usize,
    replaced_ids: usize,
}

impl BacklogMetrics {
    fn from_delta(delta: &LiveTranscriptDelta) -> Self {
        Self {
            words: delta.new_words.len(),
            text_chars: delta
                .new_words
                .iter()
                .map(|word| word.text.chars().count())
                .sum(),
            replaced_ids: delta.replaced_ids.len(),
        }
    }

    fn add(&mut self, other: Self) {
        self.words = self.words.saturating_add(other.words);
        self.text_chars = self.text_chars.saturating_add(other.text_chars);
        self.replaced_ids = self.replaced_ids.saturating_add(other.replaced_ids);
    }

    fn subtract(&mut self, other: Self) {
        self.words = self.words.saturating_sub(other.words);
        self.text_chars = self.text_chars.saturating_sub(other.text_chars);
        self.replaced_ids = self.replaced_ids.saturating_sub(other.replaced_ids);
    }

    fn exceeds_bounds(self) -> bool {
        self.words > MAX_BACKLOG_WORDS
            || self.text_chars > MAX_BACKLOG_TEXT_CHARS
            || self.replaced_ids > MAX_BACKLOG_REPLACED_IDS
    }

    fn is_empty(self) -> bool {
        self.words == 0 && self.text_chars == 0 && self.replaced_ids == 0
    }
}

struct JournalEntry {
    delta_json: String,
    metrics: BacklogMetrics,
    max_final_end_ms: Option<i64>,
}

impl JournalEntry {
    fn new(mut delta: LiveTranscriptDelta) -> Result<Self, String> {
        delta.partials.clear();
        let metrics = BacklogMetrics::from_delta(&delta);
        let max_final_end_ms = delta
            .new_words
            .iter()
            .filter(|word| word.state == WordState::Final)
            .map(|word| word.end_ms)
            .max();
        let delta_json = serde_json::to_string(&delta).map_err(|error| error.to_string())?;
        Ok(Self {
            delta_json,
            metrics,
            max_final_end_ms,
        })
    }
}

async fn publish_status(
    status: &LiveTranscriptPersistence,
    shared_status: &Mutex<LiveTranscriptPersistence>,
    event_sink: &EventSink,
) {
    *shared_status.lock().await = status.clone();
    event_sink(status.clone());
}

async fn run_writer(
    target: LiveTranscriptTarget,
    store: JournalStoreHandle,
    event_sink: EventSink,
    shared_status: Arc<Mutex<LiveTranscriptPersistence>>,
    backlog: Arc<StdMutex<BacklogTracker>>,
    mut receiver: mpsc::UnboundedReceiver<WriterMessage>,
) {
    let db_target = LiveTranscriptInsert {
        id: target.transcript_id,
        session_id: shared_status.lock().await.session_id.clone(),
        owner_user_id: target.owner_user_id,
        created_at: target.created_at,
        started_at_ms: target.started_at_ms,
        memo: target.memo,
        provider: target.provider,
        model: target.model,
    };
    let mut status = shared_status.lock().await.clone();
    let mut pending = Vec::<JournalEntry>::new();
    let mut flush_replies = Vec::<oneshot::Sender<LiveTranscriptPersistence>>::new();
    let mut batch_deadline = None;
    let mut retry_deadline = None;
    let mut retry_delay = INITIAL_RETRY_DELAY;
    let mut released_at = None;
    let mut release_failure_since = None;

    loop {
        let deadline = if !flush_replies.is_empty() && !pending.is_empty() {
            Some(Instant::now())
        } else {
            retry_deadline.or(batch_deadline)
        };
        tokio::select! {
            message = receiver.recv() => {
                match message {
                    Some(WriterMessage::Delta(delta)) => {
                        let overflowed = backlog
                            .lock()
                            .map(|backlog| backlog.overflowed)
                            .unwrap_or(true);
                        if !overflowed {
                            match JournalEntry::new(delta) {
                                Ok(entry) => {
                                    if pending.is_empty() {
                                        batch_deadline = Some(Instant::now() + BATCH_WINDOW);
                                    }
                                    pending.push(entry);
                                }
                                Err(error) => {
                                    status.error = Some(error);
                                    publish_status(&status, &shared_status, &event_sink).await;
                                }
                            }
                        }
                    }
                    Some(WriterMessage::Overflow) => {
                        pending.clear();
                        batch_deadline = None;
                        retry_deadline = None;
                        status.error = Some(BACKLOG_OVERFLOW_ERROR.to_string());
                        publish_status(&status, &shared_status, &event_sink).await;
                        for reply in flush_replies.drain(..) {
                            let _ = reply.send(status.clone());
                        }
                    }
                    Some(WriterMessage::Flush(reply)) => {
                        let is_empty = backlog
                            .lock()
                            .map(|backlog| backlog.metrics.is_empty())
                            .unwrap_or(true);
                        if pending.is_empty() && is_empty {
                            let _ = reply.send(status.clone());
                        } else {
                            flush_replies.push(reply);
                            retry_deadline = None;
                            if !pending.is_empty() {
                                batch_deadline = Some(Instant::now());
                            }
                        }
                    }
                    Some(WriterMessage::Release) => {
                        let now = Instant::now();
                        released_at.get_or_insert(now);
                        if status.error.is_some() {
                            release_failure_since.get_or_insert(now);
                        }
                        if !pending.is_empty() {
                            batch_deadline = Some(now);
                        }
                    }
                    None => {
                        let now = Instant::now();
                        released_at.get_or_insert(now);
                        if status.error.is_some() {
                            release_failure_since.get_or_insert(now);
                        }
                        if !pending.is_empty() {
                            batch_deadline = Some(now);
                        }
                    }
                }
            }
            _ = tokio::time::sleep_until(deadline.unwrap_or_else(|| Instant::now() + Duration::from_secs(86_400))), if deadline.is_some() => {
                batch_deadline = None;
                retry_deadline = None;
                if pending.is_empty() {
                    continue;
                }

                let delta_jsons = pending
                    .iter()
                    .map(|entry| entry.delta_json.clone())
                    .collect::<Vec<_>>();
                let result = store.append(db_target.clone(), delta_jsons).await;

                match result {
                    Ok(AppendOutcome { transcript_exists }) => {
                        let committed = std::mem::take(&mut pending);
                        let metrics = committed.iter().fold(
                            BacklogMetrics::default(),
                            |mut total, entry| {
                                total.add(entry.metrics);
                                total
                            },
                        );
                        let max_final_end_ms = committed
                            .iter()
                            .filter_map(|entry| entry.max_final_end_ms)
                            .max();
                        if let Ok(mut backlog) = backlog.lock() {
                            backlog.metrics.subtract(metrics);
                        }

                        status.transcript_created |= transcript_exists;
                        status.persisted_through_ms = match (
                            status.persisted_through_ms,
                            max_final_end_ms,
                        ) {
                            (Some(previous), Some(next)) => Some(previous.max(next)),
                            (None, next) => next,
                            (current, None) => current,
                        };
                        let (backlog_empty, overflowed) = backlog
                            .lock()
                            .map(|backlog| (backlog.metrics.is_empty(), backlog.overflowed))
                            .unwrap_or((false, true));
                        if overflowed {
                            status.error = Some(BACKLOG_OVERFLOW_ERROR.to_string());
                        } else if backlog_empty {
                            status.error = None;
                        }
                        publish_status(&status, &shared_status, &event_sink).await;
                        retry_delay = INITIAL_RETRY_DELAY;
                        release_failure_since = None;

                        let is_empty = backlog
                            .lock()
                            .map(|backlog| backlog.metrics.is_empty())
                            .unwrap_or(true);
                        if !flush_replies.is_empty() && is_empty {
                            for reply in flush_replies.drain(..) {
                                let _ = reply.send(status.clone());
                            }
                        }
                    }
                    Err(error) => {
                        status.error = Some(error);
                        publish_status(&status, &shared_status, &event_sink).await;
                        if !flush_replies.is_empty() {
                            for reply in flush_replies.drain(..) {
                                let _ = reply.send(status.clone());
                            }
                        }

                        let now = Instant::now();
                        if released_at.is_some() {
                            let failure_since = release_failure_since.get_or_insert(now);
                            if now.duration_since(*failure_since) >= RELEASE_FAILURE_TIMEOUT {
                                pending.clear();
                                break;
                            }
                        }
                        retry_deadline = Some(now + retry_delay);
                        retry_delay = retry_delay
                            .saturating_mul(2)
                            .min(MAX_RETRY_DELAY);
                    }
                }
            }
        }

        if released_at.is_some() && pending.is_empty() {
            let is_empty = backlog
                .lock()
                .map(|backlog| backlog.metrics.is_empty())
                .unwrap_or(true);
            if is_empty {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_db_core::Db;
    use anlg_transcript::{FinalizedWord, PartialWord, WordState};
    use sqlx::SqlitePool;

    struct PoolJournalStore(SqlitePool);

    impl JournalStore for PoolJournalStore {
        fn append(
            &self,
            target: LiveTranscriptInsert,
            delta_jsons: Vec<String>,
        ) -> BoxFuture<'static, Result<AppendOutcome, String>> {
            let pool = self.0.clone();
            Box::pin(async move {
                append_live_transcript_deltas(&pool, &target, &delta_jsons)
                    .await
                    .map_err(|error| error.to_string())
            })
        }
    }

    fn target() -> LiveTranscriptTarget {
        LiveTranscriptTarget {
            transcript_id: "transcript-1".to_string(),
            owner_user_id: "owner-1".to_string(),
            created_at: "2026-08-15T12:00:00.000Z".to_string(),
            started_at_ms: 100,
            memo: String::new(),
            provider: Some("test-provider".to_string()),
            model: Some("test-model".to_string()),
        }
    }

    fn word(id: &str, end_ms: i64, state: WordState) -> FinalizedWord {
        FinalizedWord {
            id: id.to_string(),
            text: id.to_string(),
            start_ms: end_ms - 100,
            end_ms,
            channel: 0,
            state,
            speaker_index: None,
        }
    }

    fn delta(words: Vec<FinalizedWord>) -> LiveTranscriptDelta {
        LiveTranscriptDelta {
            new_words: words,
            replaced_ids: vec!["replaced-word".to_string()],
            partials: vec![PartialWord {
                text: "uncommitted".to_string(),
                start_ms: 0,
                end_ms: 50,
                channel: 0,
                speaker_index: None,
            }],
        }
    }

    #[tokio::test]
    async fn appends_deltas_and_flushes_with_final_word_progress() {
        let db = Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id)
             VALUES ('session-1', 'workspace-1', 'owner-1')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let pool = db.pool().clone();
        let registry = LiveJournalRegistry::default();
        let events = Arc::new(StdMutex::new(Vec::new()));
        let event_sink_events = events.clone();
        registry
            .register(
                "session-1".to_string(),
                target(),
                Arc::new(PoolJournalStore(pool.clone())),
                Arc::new(move |status| {
                    event_sink_events.lock().unwrap().push(status);
                }),
            )
            .unwrap();

        let journal = registry.get("session-1").unwrap().unwrap();
        journal.append(delta(vec![word("final-1", 500, WordState::Final)]));
        journal.append(delta(vec![
            word("final-2", 800, WordState::Final),
            word("pending", 1200, WordState::Pending),
        ]));

        let status = journal.flush().await.unwrap();
        assert!(status.transcript_created);
        assert_eq!(status.persisted_through_ms, Some(800));
        assert_eq!(status.error, None);

        let deltas = sqlx::query_scalar::<_, String>(
            "SELECT delta_json FROM transcript_live_deltas
             WHERE transcript_id = 'transcript-1'
             ORDER BY sequence",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(deltas.len(), 2);
        for delta_json in deltas {
            let stored: serde_json::Value = serde_json::from_str(&delta_json).unwrap();
            assert_eq!(stored["partials"], serde_json::json!([]));
        }

        assert!(
            events
                .lock()
                .unwrap()
                .iter()
                .any(|status| status.transcript_created
                    && status.persisted_through_ms == Some(800)
                    && status.error.is_none())
        );
    }

    #[tokio::test]
    async fn rejected_registration_restores_previous_journal() {
        let db = Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id)
             VALUES ('session-1', 'workspace-1', 'owner-1')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let registry = LiveJournalRegistry::default();
        let store = || Arc::new(PoolJournalStore(db.pool().clone())) as JournalStoreHandle;
        let event_sink: EventSink = Arc::new(|_| {});
        let mut target_a = target();
        target_a.transcript_id = "transcript-A".to_string();
        let (_, previous) = registry
            .register(
                "session-1".to_string(),
                target_a,
                store(),
                event_sink.clone(),
            )
            .unwrap();
        assert!(previous.is_none());

        let mut target_b = target();
        target_b.transcript_id = "transcript-B".to_string();
        let (registered_b, previous_a) = registry
            .register("session-1".to_string(), target_b, store(), event_sink)
            .unwrap();
        registry
            .rollback_registration("session-1", &registered_b, previous_a)
            .unwrap();

        let journal_a = registry.get("session-1").unwrap().unwrap();
        assert_eq!(journal_a.transcript_id, "transcript-A");
        journal_a.append(delta(vec![word("restored-word", 500, WordState::Final)]));
        assert!(journal_a.flush().await.unwrap().transcript_created);

        let journaled_deltas: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM transcript_live_deltas
             WHERE transcript_id = 'transcript-A'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(journaled_deltas, 1);

        registry.release_session("session-1").unwrap();
    }
}
