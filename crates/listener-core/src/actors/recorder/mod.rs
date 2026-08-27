mod disk;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use ractor::{Actor, ActorName, ActorProcessingErr, ActorRef, RpcReplyPort};

pub enum RecMsg {
    AudioSingle(Arc<[f32]>, RpcReplyPort<RecorderEnqueueResult>),
    AudioDual(Arc<[f32]>, Arc<[f32]>, RpcReplyPort<RecorderEnqueueResult>),
    WriterFailed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecorderEnqueueResult {
    Accepted,
    Backpressured,
    Closed,
}

pub struct RecArgs {
    pub app_dir: PathBuf,
    pub session_id: String,
}

pub struct RecState {
    writer_tx: Option<tokio::sync::mpsc::Sender<WriteRequest>>,
    writer_task: Option<tokio::task::JoinHandle<Result<(), String>>>,
}

enum WriteRequest {
    AudioSingle(Arc<[f32]>),
    AudioDual(Arc<[f32]>, Arc<[f32]>),
}

const WRITE_QUEUE_CAPACITY: usize = 32;
const MAX_CONCURRENT_WRITERS: usize = 4;
const WRITER_SHUTDOWN_WARNING: Duration = Duration::from_secs(10);

static WRITER_SLOTS: LazyLock<Arc<tokio::sync::Semaphore>> =
    LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_WRITERS)));
static ACTIVE_WRITER_PATHS: LazyLock<Mutex<HashSet<PathBuf>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

struct WriterPathGuard {
    path: PathBuf,
}

impl Drop for WriterPathGuard {
    fn drop(&mut self) {
        if let Ok(mut paths) = ACTIVE_WRITER_PATHS.lock() {
            paths.remove(&self.path);
        }
    }
}

pub struct RecorderActor;

impl Default for RecorderActor {
    fn default() -> Self {
        Self::new()
    }
}

impl RecorderActor {
    pub fn new() -> Self {
        Self
    }

    pub fn name() -> ActorName {
        "recorder_actor".into()
    }
}

#[ractor::async_trait]
impl Actor for RecorderActor {
    type Msg = RecMsg;
    type State = RecState;
    type Arguments = RecArgs;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let writer_permit = try_acquire_writer_slot(WRITER_SLOTS.clone())?;
        let session_dir = find_session_dir(&args.app_dir, &args.session_id);
        let writer_path = try_reserve_writer_path(session_dir.clone())?;
        let (sink, writer_permit, writer_path) = tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&session_dir)?;
            let sink = disk::create_disk_sink(&session_dir)?;
            Ok::<_, ActorProcessingErr>((sink, writer_permit, writer_path))
        })
        .await
        .map_err(into_actor_err)??;

        let (writer_tx, writer_rx) = tokio::sync::mpsc::channel(WRITE_QUEUE_CAPACITY);
        let writer_task = tokio::task::spawn_blocking(move || {
            let _writer_permit = writer_permit;
            let _writer_path = writer_path;
            run_writer(sink, writer_rx, myself)
        });

        Ok(RecState {
            writer_tx: Some(writer_tx),
            writer_task: Some(writer_task),
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        st: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        let (request, reply) = match msg {
            RecMsg::AudioSingle(samples, reply) => (WriteRequest::AudioSingle(samples), reply),
            RecMsg::AudioDual(mic, spk, reply) => (WriteRequest::AudioDual(mic, spk), reply),
            RecMsg::WriterFailed(error) => return Err(actor_error(error)),
        };

        let Some(writer_tx) = st.writer_tx.as_ref() else {
            let _ = reply.send(RecorderEnqueueResult::Closed);
            return Err(actor_error("recorder writer is unavailable"));
        };

        match writer_tx.try_send(request) {
            Ok(()) => {
                let _ = reply.send(RecorderEnqueueResult::Accepted);
            }
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                let _ = reply.send(RecorderEnqueueResult::Backpressured);
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                let _ = reply.send(RecorderEnqueueResult::Closed);
                return Err(actor_error("recorder writer stopped unexpectedly"));
            }
        }

        Ok(())
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        st: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        st.writer_tx.take();

        if let Some(writer_task) = st.writer_task.take() {
            await_writer_shutdown(writer_task, WRITER_SHUTDOWN_WARNING).await?;
        }

        Ok(())
    }
}

fn try_acquire_writer_slot(
    slots: Arc<tokio::sync::Semaphore>,
) -> Result<tokio::sync::OwnedSemaphorePermit, ActorProcessingErr> {
    slots
        .try_acquire_owned()
        .map_err(|_| actor_error("too many recorder writer jobs are still active"))
}

fn try_reserve_writer_path(path: PathBuf) -> Result<WriterPathGuard, ActorProcessingErr> {
    let mut paths = ACTIVE_WRITER_PATHS
        .lock()
        .map_err(|_| actor_error("recorder writer path registry is unavailable"))?;
    if !paths.insert(path.clone()) {
        return Err(actor_error(
            "a recorder writer is already active for this session",
        ));
    }

    Ok(WriterPathGuard { path })
}

async fn await_writer_shutdown(
    mut writer_task: tokio::task::JoinHandle<Result<(), String>>,
    timeout: Duration,
) -> Result<(), ActorProcessingErr> {
    match tokio::time::timeout(timeout, &mut writer_task).await {
        Ok(result) => result.map_err(into_actor_err)?.map_err(actor_error),
        Err(_) => {
            tracing::warn!(
                warning_after_seconds = timeout.as_secs(),
                "recorder_writer_finalization_slow"
            );
            writer_task
                .await
                .map_err(into_actor_err)?
                .map_err(actor_error)
        }
    }
}

fn run_writer(
    mut sink: disk::DiskSink,
    mut writer_rx: tokio::sync::mpsc::Receiver<WriteRequest>,
    actor: ActorRef<RecMsg>,
) -> Result<(), String> {
    while let Some(request) = writer_rx.blocking_recv() {
        let result = match &request {
            WriteRequest::AudioSingle(samples) => disk::write_single(&mut sink, samples),
            WriteRequest::AudioDual(mic, spk) => disk::write_dual(&mut sink, mic, spk),
        }
        .map_err(|error| error.to_string());

        match result {
            Ok(()) => {}
            Err(error) => {
                let _ = actor.cast(RecMsg::WriterFailed(error.clone()));
                return Err(error);
            }
        }
    }

    disk::finalize_disk_sink(&mut sink).map_err(|error| error.to_string())
}

fn actor_error(error: impl Into<String>) -> ActorProcessingErr {
    into_actor_err(std::io::Error::other(error.into()))
}

pub fn find_session_dir(sessions_base: &Path, session_id: &str) -> PathBuf {
    if let Some(found) = find_session_dir_recursive(sessions_base, session_id) {
        return found;
    }
    sessions_base.join(session_id)
}

pub fn resolve_final_audio_path(sessions_base: &Path, session_id: &str) -> Option<PathBuf> {
    let session_dir = find_session_dir(sessions_base, session_id);
    let mp3_path = session_dir.join("audio.mp3");
    if mp3_path.exists() {
        return Some(mp3_path);
    }

    let wav_path = session_dir.join("audio.wav");
    if wav_path.exists() {
        return Some(wav_path);
    }

    let ogg_path = session_dir.join("audio.ogg");
    if ogg_path.exists() {
        return Some(ogg_path);
    }

    None
}

fn find_session_dir_recursive(dir: &Path, session_id: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = path.file_name()?.to_str()?;

        if name == session_id {
            return Some(path);
        }

        if uuid::Uuid::try_parse(name).is_err()
            && let Some(found) = find_session_dir_recursive(&path, session_id)
        {
            return Some(found);
        }
    }

    None
}

fn into_actor_err<E>(err: E) -> ActorProcessingErr
where
    E: std::error::Error + Send + Sync + 'static,
{
    Box::new(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writer_admission_rejects_jobs_above_capacity() {
        let slots = Arc::new(tokio::sync::Semaphore::new(2));
        let first = try_acquire_writer_slot(slots.clone()).unwrap();
        let _second = try_acquire_writer_slot(slots.clone()).unwrap();

        assert!(try_acquire_writer_slot(slots.clone()).is_err());
        drop(first);
        assert!(try_acquire_writer_slot(slots).is_ok());
    }

    #[test]
    fn writer_path_admission_rejects_overlapping_session_writers() {
        let path = std::env::temp_dir().join(format!("anarlog-recorder-{}", uuid::Uuid::new_v4()));
        let first = try_reserve_writer_path(path.clone()).unwrap();

        assert!(try_reserve_writer_path(path.clone()).is_err());
        drop(first);
        assert!(try_reserve_writer_path(path).is_ok());
    }

    #[tokio::test]
    async fn writer_shutdown_warning_does_not_detach_finalization() {
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let task = tokio::task::spawn_blocking(move || {
            release_rx.recv().unwrap();
            Ok(())
        });
        let waiter = tokio::spawn(await_writer_shutdown(task, Duration::from_millis(5)));

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!waiter.is_finished());

        release_tx.send(()).unwrap();
        assert!(waiter.await.unwrap().is_ok());
    }
}
