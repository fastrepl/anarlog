use std::{
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

use desktop_runtime::{DocumentSnapshot, RuntimeHandle, SaveDocument, ServiceError};
use futures::{
    SinkExt,
    channel::{mpsc, oneshot},
    executor::block_on,
};

use super::document::Document;

const DEBOUNCE: Duration = Duration::from_millis(500);
const MAX_WAIT: Duration = Duration::from_secs(10);
const MAX_FLUSH_WAITERS: usize = 32;

pub type Flush = oneshot::Receiver<Result<(), ServiceError>>;

pub enum SaveEvent {
    Saved {
        revision: u64,
        snapshot: DocumentSnapshot,
    },
    Failed(ServiceError),
}

#[derive(Clone)]
struct Draft {
    revision: u64,
    document: Document,
}

struct State {
    base: DocumentSnapshot,
    pending: Option<Draft>,
    acknowledged: u64,
    first_dirty: Option<Instant>,
    last_edit: Instant,
    saving: bool,
    retry: bool,
    stopped: bool,
    error: Option<ServiceError>,
    waiters: Vec<(u64, oneshot::Sender<Result<(), ServiceError>>)>,
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
}

pub struct SaveJournal {
    shared: Arc<Shared>,
}

impl SaveJournal {
    pub fn start(
        runtime: RuntimeHandle,
        base: DocumentSnapshot,
    ) -> Result<(Self, mpsc::Receiver<SaveEvent>), ServiceError> {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                base,
                pending: None,
                acknowledged: 0,
                first_dirty: None,
                last_edit: Instant::now(),
                saving: false,
                retry: false,
                stopped: false,
                error: None,
                waiters: Vec::new(),
            }),
            wake: Condvar::new(),
        });
        let guard = Arc::downgrade(&shared);
        block_on(runtime.register_flush(Box::new(move || Box::pin(async move {
            if let Some(shared) = guard.upgrade() {
                let state = shared.state.lock().map_err(|_| failed("Editor journal lock failed"))?;
                if state.pending.is_some() || state.saving {
                    return Err(state.error.clone().unwrap_or_else(|| failed(
                        "Editor has unsaved changes. Await EditorPane::flush before runtime shutdown."
                    )));
                }
            }
            Ok(())
        })))?.receive())?;
        let (events, receiver) = mpsc::channel(8);
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("anarlog-editor-save".into())
            .spawn(move || run(runtime, worker, events))
            .map_err(|error| failed(error.to_string()))?;
        Ok((Self { shared }, receiver))
    }

    pub fn publish(&self, revision: u64, document: Document) {
        let mut state = self.shared.state.lock().expect("editor journal");
        let now = Instant::now();
        state.first_dirty.get_or_insert(now);
        state.last_edit = now;
        state.pending = Some(Draft { revision, document });
        self.shared.wake.notify_one();
    }

    pub fn retry(&self) {
        let mut state = self.shared.state.lock().expect("editor journal");
        state.retry = true;
        self.shared.wake.notify_one();
    }

    pub fn flush(&self, revision: u64) -> Flush {
        let (tx, rx) = oneshot::channel();
        let mut state = self.shared.state.lock().expect("editor journal");
        if revision <= state.acknowledged {
            let _ = tx.send(Ok(()));
        } else if state.waiters.len() >= MAX_FLUSH_WAITERS {
            let _ = tx.send(Err(ServiceError::Busy));
        } else {
            state.waiters.push((revision, tx));
            state.retry = true;
            self.shared.wake.notify_one();
        }
        rx
    }

    pub fn rebase_if_clean(&self, base: DocumentSnapshot) -> bool {
        let mut state = self.shared.state.lock().expect("editor journal");
        if state.pending.is_some() || state.saving {
            return false;
        }
        state.base = base;
        state.acknowledged = 0;
        true
    }

    pub fn resolve_conflict(
        &self,
        base: DocumentSnapshot,
        revision: u64,
        document: Document,
    ) -> Result<(), ServiceError> {
        let mut state = self.shared.state.lock().expect("editor journal");
        if state.saving {
            return Err(ServiceError::Busy);
        }
        if !matches!(state.error, Some(ServiceError::Conflict)) && state.pending.is_some() {
            return Err(ServiceError::Busy);
        }
        state.base = base;
        state.pending = Some(Draft { revision, document });
        state.error = None;
        state.retry = true;
        state.first_dirty = Some(Instant::now());
        self.shared.wake.notify_one();
        Ok(())
    }
}

impl Drop for SaveJournal {
    fn drop(&mut self) {
        if let Ok(mut state) = self.shared.state.lock() {
            state.stopped = true;
            state.retry = true;
            self.shared.wake.notify_one();
        }
    }
}

fn run(runtime: RuntimeHandle, shared: Arc<Shared>, mut events: mpsc::Sender<SaveEvent>) {
    loop {
        let (draft, base, stopping) = {
            let mut state = shared.state.lock().expect("editor journal");
            loop {
                if state.stopped && state.pending.is_none() {
                    return;
                }
                if state.pending.is_some() && (state.error.is_none() || state.retry) {
                    let due = state.first_dirty.unwrap_or_else(Instant::now) + MAX_WAIT;
                    let due = due.min(state.last_edit + DEBOUNCE);
                    if state.retry || state.stopped || Instant::now() >= due {
                        state.saving = true;
                        state.retry = false;
                        break (
                            state.pending.as_ref().expect("pending draft").clone(),
                            state.base.clone(),
                            state.stopped,
                        );
                    }
                    let wait = due.saturating_duration_since(Instant::now());
                    state = shared
                        .wake
                        .wait_timeout(state, wait)
                        .expect("editor wake")
                        .0;
                } else {
                    state = shared.wake.wait(state).expect("editor wake");
                }
            }
        };
        let result = draft
            .document
            .serialize_for_save()
            .map_err(failed)
            .and_then(|body| {
                runtime
                    .save_document(SaveDocument { base, body })
                    .and_then(|reply| block_on(reply.receive()))
            });
        let event = {
            let mut state = shared.state.lock().expect("editor journal");
            state.saving = false;
            let wait_result = match &result {
                Ok(snapshot) => {
                    state.base = snapshot.clone();
                    state.acknowledged = draft.revision;
                    state.error = None;
                    if state
                        .pending
                        .as_ref()
                        .is_some_and(|pending| pending.revision <= draft.revision)
                    {
                        state.pending = None;
                        state.first_dirty = None;
                    } else {
                        state.first_dirty = Some(Instant::now());
                    }
                    Ok(())
                }
                Err(error) => {
                    state.error = Some(error.clone());
                    Err(error.clone())
                }
            };
            let waiters = std::mem::take(&mut state.waiters);
            for (revision, waiter) in waiters {
                if wait_result.is_err() || revision <= state.acknowledged {
                    let _ = waiter.send(wait_result.clone());
                } else {
                    state.waiters.push((revision, waiter));
                }
            }
            match result {
                Ok(snapshot) => SaveEvent::Saved {
                    revision: draft.revision,
                    snapshot,
                },
                Err(error) => SaveEvent::Failed(error),
            }
        };
        let failed = matches!(event, SaveEvent::Failed(_));
        let _ = block_on(events.send(event));
        if stopping && failed {
            return;
        }
    }
}

fn failed(message: impl Into<String>) -> ServiceError {
    ServiceError::Failed(message.into().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use desktop_runtime::{CancellationToken, Profile};

    #[test]
    fn flush_persists_latest_real_document_and_reopen() {
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(Profile {
            database: directory.path().join("editor.db"),
        })
        .unwrap();
        block_on(ready.receive()).unwrap();
        let session = block_on(runtime.create_note("fixture".into()).unwrap().receive()).unwrap();
        let (journal, _events) =
            SaveJournal::start(runtime.clone(), session.note.unwrap()).unwrap();
        for revision in 1..=200 {
            let document = Document::parse(format!(
                r#"{{"type":"doc","content":[{{"type":"paragraph","content":[{{"type":"text","text":"revision {revision}"}}]}}]}}"#
            ).into()).unwrap();
            journal.publish(revision, document);
        }
        block_on(journal.flush(200)).unwrap().unwrap();
        let loaded = block_on(
            runtime
                .open_session(session.summary.id, CancellationToken::new())
                .unwrap()
                .receive(),
        )
        .unwrap();
        assert!(loaded.note.unwrap().body.contains("revision 200"));
        drop(journal);
        block_on(runtime.shutdown()).unwrap();
    }

    #[test]
    fn conflict_preserves_pending_and_flush_reports_failure() {
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(Profile {
            database: directory.path().join("editor.db"),
        })
        .unwrap();
        block_on(ready.receive()).unwrap();
        let session = block_on(runtime.create_note("fixture".into()).unwrap().receive()).unwrap();
        let base = session.note.unwrap();
        let (journal, _events) = SaveJournal::start(runtime.clone(), base.clone()).unwrap();
        let remote = r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"remote"}]}]}"#;
        block_on(
            runtime
                .save_document(SaveDocument {
                    base,
                    body: remote.into(),
                })
                .unwrap()
                .receive(),
        )
        .unwrap();
        let local = Document::parse(r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"local"}]}]}"#.into()).unwrap();
        journal.publish(1, local);
        assert!(matches!(
            block_on(journal.flush(1)).unwrap(),
            Err(ServiceError::Conflict)
        ));
        assert!(journal.shared.state.lock().unwrap().pending.is_some());
        assert!(block_on(runtime.shutdown()).is_err());
    }

    #[test]
    fn explicit_conflict_recovery_retries_against_latest_canonical_base() {
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(Profile {
            database: directory.path().join("editor.db"),
        })
        .unwrap();
        block_on(ready.receive()).unwrap();
        let session = block_on(runtime.create_note("fixture".into()).unwrap().receive()).unwrap();
        let base = session.note.unwrap();
        let (journal, _events) = SaveJournal::start(runtime.clone(), base.clone()).unwrap();
        let remote = block_on(runtime.save_document(SaveDocument {
            base, body: r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"remote"}]}]}"#.into(),
        }).unwrap().receive()).unwrap();
        let local = Document::parse(r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"local"}]}]}"#.into()).unwrap();
        journal.publish(1, local.clone());
        assert!(block_on(journal.flush(1)).unwrap().is_err());
        journal.resolve_conflict(remote, 2, local).unwrap();
        assert!(block_on(journal.flush(2)).unwrap().is_ok());
        let reopened = block_on(
            runtime
                .open_session(session.summary.id, CancellationToken::new())
                .unwrap()
                .receive(),
        )
        .unwrap();
        assert!(reopened.note.unwrap().body.contains("local"));
        drop(journal);
        assert!(block_on(runtime.shutdown()).is_ok());
    }

    #[test]
    fn old_acknowledgements_do_not_clear_newer_drafts_and_flush_waiters_are_bounded() {
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(Profile {
            database: directory.path().join("editor.db"),
        })
        .unwrap();
        block_on(ready.receive()).unwrap();
        let session = block_on(runtime.create_note("fixture".into()).unwrap().receive()).unwrap();
        let (journal, _events) =
            SaveJournal::start(runtime.clone(), session.note.unwrap()).unwrap();
        let (release, resume) = oneshot::channel();
        let gate = runtime
            .submit(move |_| async move {
                resume.await.map_err(|_| ServiceError::Cancelled)?;
                Ok(())
            })
            .unwrap();
        let first = Document::parse(r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"first"}]}]}"#.into()).unwrap();
        journal.publish(1, first);
        let first_flush = journal.flush(1);
        let start = Instant::now();
        while !journal.shared.state.lock().unwrap().saving {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "save worker did not start"
            );
            std::thread::yield_now();
        }
        let last = Document::parse(r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"newest"}]}]}"#.into()).unwrap();
        journal.publish(2, last);
        let waiters: Vec<_> = (1..MAX_FLUSH_WAITERS).map(|_| journal.flush(2)).collect();
        assert!(matches!(
            block_on(journal.flush(2)).unwrap(),
            Err(ServiceError::Busy)
        ));
        release.send(()).unwrap();
        block_on(gate.receive()).unwrap();
        block_on(first_flush).unwrap().unwrap();
        for waiter in waiters {
            block_on(waiter).unwrap().unwrap();
        }
        let reopened = block_on(
            runtime
                .open_session(session.summary.id, CancellationToken::new())
                .unwrap()
                .receive(),
        )
        .unwrap();
        assert!(reopened.note.unwrap().body.contains("newest"));
        assert!(journal.shared.state.lock().unwrap().pending.is_none());
        drop(journal);
        block_on(runtime.shutdown()).unwrap();
    }

    #[test]
    fn uninterrupted_typing_is_persisted_at_max_wait() {
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(Profile {
            database: directory.path().join("editor.db"),
        })
        .unwrap();
        block_on(ready.receive()).unwrap();
        let session = block_on(runtime.create_note("fixture".into()).unwrap().receive()).unwrap();
        let (journal, _events) =
            SaveJournal::start(runtime.clone(), session.note.unwrap()).unwrap();
        let start = Instant::now();
        let mut revision = 0;
        while start.elapsed() < Duration::from_millis(11_000) {
            revision += 1;
            let body = format!(
                r#"{{"type":"doc","content":[{{"type":"paragraph","content":[{{"type":"text","text":"revision {revision}"}}]}}]}}"#
            );
            journal.publish(revision, Document::parse(body.into()).unwrap());
            std::thread::sleep(Duration::from_millis(100));
        }
        let reopened = block_on(
            runtime
                .open_session(session.summary.id, CancellationToken::new())
                .unwrap()
                .receive(),
        )
        .unwrap();
        assert!(
            reopened.note.unwrap().body.contains("revision"),
            "continuous typing must save before debounce becomes idle"
        );
        assert!(journal.shared.state.lock().unwrap().pending.is_some());
        assert!(block_on(journal.flush(revision)).unwrap().is_ok());
        drop(journal);
        assert!(block_on(runtime.shutdown()).is_ok());
    }
}
