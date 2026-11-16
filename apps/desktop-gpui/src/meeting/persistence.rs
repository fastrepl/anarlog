use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anlg_listener_core::LiveTranscriptDelta;
use desktop_runtime::{Result, RuntimeHandle, ServiceError, Services};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot, watch};

use super::model::{MAX_TEXT, MAX_WORDS, failure, validate_delta};
use super::store;

const BATCH_WINDOW: Duration = Duration::from_millis(250);
const WRITE_TIMEOUT: Duration = Duration::from_secs(15);
pub const FLUSH_TIMEOUT: Duration = Duration::from_secs(20);

struct Entry {
    delta: LiveTranscriptDelta,
    _permits: Vec<OwnedSemaphorePermit>,
}

enum Command {
    Delta(Entry),
    Flush(oneshot::Sender<Result<()>>),
}

#[derive(Clone)]
pub struct Persistence {
    sender: mpsc::Sender<Command>,
    words: Arc<Semaphore>,
    replacements: Arc<Semaphore>,
    text: Arc<Semaphore>,
    pub error: watch::Receiver<Option<ServiceError>>,
    retired: Arc<AtomicBool>,
    admission: Arc<tokio::sync::Mutex<()>>,
}

impl Persistence {
    async fn spawn(runtime: &RuntimeHandle, transcript: Arc<str>) -> Result<Self> {
        runtime
            .submit(move |services| async move {
                let (sender, receiver) = mpsc::channel(16);
                let (error, state) = watch::channel(None);
                tokio::spawn(run(services, transcript, receiver, error));
                Ok(Self {
                    sender,
                    words: Arc::new(Semaphore::new(MAX_WORDS)),
                    replacements: Arc::new(Semaphore::new(MAX_WORDS)),
                    text: Arc::new(Semaphore::new(MAX_TEXT)),
                    error: state,
                    retired: Arc::new(AtomicBool::new(false)),
                    admission: Arc::new(tokio::sync::Mutex::new(())),
                })
            })?
            .receive()
            .await
    }

    pub async fn install(runtime: &RuntimeHandle, transcript: Arc<str>) -> Result<Self> {
        let handle = Self::spawn(runtime, transcript).await?;
        let flush = handle.clone();
        runtime
            .register_flush(Box::new(move || {
                Box::pin(async move { flush.flush().await })
            }))?
            .receive()
            .await?;
        Ok(handle)
    }

    pub async fn push(&self, mut delta: LiveTranscriptDelta) -> Result<()> {
        let _admission = self.admission.lock().await;
        if self.retired.load(Ordering::Acquire) {
            return Err(ServiceError::Closed);
        }
        validate_delta(&delta)?;
        delta.partials.clear();
        if delta.new_words.is_empty() && delta.replaced_ids.is_empty() {
            return Ok(());
        }
        let text = delta
            .new_words
            .iter()
            .map(|word| word.id.len() + word.text.len())
            .sum::<usize>()
            + delta.replaced_ids.iter().map(String::len).sum::<usize>();
        let mut permits = Vec::new();
        for (semaphore, size) in [
            (&self.text, text),
            (&self.words, delta.new_words.len()),
            (&self.replacements, delta.replaced_ids.len()),
        ] {
            permits.push(
                semaphore
                    .clone()
                    .acquire_many_owned(size as u32)
                    .await
                    .map_err(|_| ServiceError::Closed)?,
            );
        }
        self.sender
            .send(Command::Delta(Entry {
                delta,
                _permits: permits,
            }))
            .await
            .map_err(|_| ServiceError::Closed)
    }

    pub fn try_push(&self, mut delta: LiveTranscriptDelta) -> Result<()> {
        let _admission = self.admission.try_lock().map_err(|_| ServiceError::Busy)?;
        if self.retired.load(Ordering::Acquire) {
            return Err(ServiceError::Closed);
        }
        validate_delta(&delta)?;
        delta.partials.clear();
        if delta.new_words.is_empty() && delta.replaced_ids.is_empty() {
            return Ok(());
        }
        let text = delta
            .new_words
            .iter()
            .map(|word| word.id.len() + word.text.len())
            .sum::<usize>()
            + delta.replaced_ids.iter().map(String::len).sum::<usize>();
        let mut permits = Vec::new();
        for (semaphore, size) in [
            (&self.text, text),
            (&self.words, delta.new_words.len()),
            (&self.replacements, delta.replaced_ids.len()),
        ] {
            permits.push(
                semaphore
                    .clone()
                    .try_acquire_many_owned(size as u32)
                    .map_err(|_| ServiceError::Busy)?,
            );
        }
        self.sender
            .try_send(Command::Delta(Entry {
                delta,
                _permits: permits,
            }))
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => ServiceError::Busy,
                mpsc::error::TrySendError::Closed(_) => ServiceError::Closed,
            })
    }

    pub async fn retire(&self) -> Result<()> {
        let _admission = self.admission.lock().await;
        self.flush().await?;
        self.retired.store(true, Ordering::Release);
        Ok(())
    }

    pub async fn flush(&self) -> Result<()> {
        tokio::time::timeout(FLUSH_TIMEOUT, async {
            let (reply, receive) = oneshot::channel();
            self.sender
                .send(Command::Flush(reply))
                .await
                .map_err(|_| ServiceError::Closed)?;
            receive.await.map_err(|_| ServiceError::Closed)?
        })
        .await
        .map_err(|_| failure("Transcript flush timed out; journal and pending writes retained."))?
    }
}

#[derive(Clone, Default)]
pub struct PersistenceGroup {
    members: Arc<Mutex<Vec<Persistence>>>,
}

impl PersistenceGroup {
    pub async fn install(runtime: &RuntimeHandle) -> Result<Self> {
        let group = Self::default();
        let flush = group.clone();
        runtime
            .register_flush(Box::new(move || {
                Box::pin(async move {
                    let members = flush.members.lock().map_err(failure)?.clone();
                    let mut result = Ok(());
                    for member in members {
                        if let Err(error) = member.flush().await {
                            result = Err(error);
                        }
                    }
                    result
                })
            }))?
            .receive()
            .await?;
        Ok(group)
    }

    pub async fn transcript(
        &self,
        runtime: &RuntimeHandle,
        transcript: Arc<str>,
    ) -> Result<Persistence> {
        let handle = Persistence::spawn(runtime, transcript).await?;
        let mut members = self.members.lock().map_err(failure)?;
        members.retain(|member| !member.retired.load(Ordering::Acquire));
        if members.len() >= 16 {
            return Err(ServiceError::Busy);
        }
        members.push(handle.clone());
        Ok(handle)
    }
}

pub fn coalesce(deltas: impl IntoIterator<Item = LiveTranscriptDelta>) -> LiveTranscriptDelta {
    let mut words = BTreeMap::new();
    let mut replacements = BTreeSet::new();
    for delta in deltas {
        for id in delta.replaced_ids {
            words.remove(&id);
            replacements.insert(id);
        }
        for word in delta.new_words {
            words.insert(word.id.clone(), word);
        }
    }
    let mut new_words: Vec<_> = words.into_values().collect();
    new_words.sort_by_key(|word| word.start_ms);
    LiveTranscriptDelta {
        new_words,
        replaced_ids: replacements.into_iter().collect(),
        partials: Vec::new(),
    }
}

async fn run(
    services: Services,
    transcript: Arc<str>,
    mut receiver: mpsc::Receiver<Command>,
    errors: watch::Sender<Option<ServiceError>>,
) {
    while let Some(first) = receiver.recv().await {
        let mut entries = Vec::new();
        let mut barrier = None;
        match first {
            Command::Delta(entry) => entries.push(entry),
            Command::Flush(reply) => barrier = Some(reply),
        }
        let deadline = tokio::time::Instant::now() + BATCH_WINDOW;
        while barrier.is_none() {
            match tokio::time::timeout_at(deadline, receiver.recv()).await {
                Ok(Some(Command::Delta(entry))) => entries.push(entry),
                Ok(Some(Command::Flush(reply))) => barrier = Some(reply),
                _ => break,
            }
        }
        if !entries.is_empty() {
            let operation = uuid::Uuid::new_v4().to_string();
            let delta = coalesce(entries.iter_mut().map(|entry| {
                std::mem::replace(
                    &mut entry.delta,
                    LiveTranscriptDelta {
                        new_words: Vec::new(),
                        replaced_ids: Vec::new(),
                        partials: Vec::new(),
                    },
                )
            }));
            let mut backoff = Duration::from_millis(250);
            loop {
                let write = store::journal(&services, &transcript, &operation, &delta);
                tokio::pin!(write);
                let result = match tokio::time::timeout(WRITE_TIMEOUT, &mut write).await {
                    Ok(result) => result,
                    Err(_) => {
                        errors.send_replace(Some(failure(
                            "Transcript storage is stalled; waiting for the accepted write.",
                        )));
                        write.await
                    }
                };
                match result {
                    Ok(()) => {
                        errors.send_replace(None);
                        break;
                    }
                    Err(error) => {
                        errors.send_replace(Some(error));
                        tokio::time::sleep(backoff).await;
                        backoff = (backoff * 2).min(Duration::from_secs(5));
                    }
                }
            }
        }
        drop(entries);
        if let Some(reply) = barrier {
            let result = async {
                for _ in 0..5 {
                    let snapshot = store::load(&services, transcript.clone()).await?;
                    match store::save(&services, snapshot).await {
                        Err(ServiceError::Conflict) => continue,
                        result => return result,
                    }
                }
                Err(ServiceError::Conflict)
            }
            .await;
            if let Err(error) = &result {
                errors.send_replace(Some(error.clone()));
            }
            let _ = reply.send(result);
        }
    }
}
