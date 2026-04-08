mod disk;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ractor::{Actor, ActorName, ActorProcessingErr, ActorRef};

pub enum RecMsg {
    AudioSingle(Arc<[f32]>),
    AudioDual(Arc<[f32]>, Arc<[f32]>),
}

pub struct RecArgs {
    pub app_dir: PathBuf,
    pub session_id: String,
}

pub struct RecState {
    sink: RecorderSink,
}

enum RecorderSink {
    Disk(disk::DiskSink),
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
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let session_dir = find_session_dir(&args.app_dir, &args.session_id);
        std::fs::create_dir_all(&session_dir)?;

        Ok(RecState {
            sink: RecorderSink::Disk(disk::create_disk_sink(&session_dir)?),
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        st: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match (&mut st.sink, msg) {
            (RecorderSink::Disk(sink), RecMsg::AudioSingle(samples)) => {
                disk::write_single(sink, &samples)?;
            }
            (RecorderSink::Disk(sink), RecMsg::AudioDual(mic, spk)) => {
                disk::write_dual(sink, &mic, &spk)?;
            }
        }

        Ok(())
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        st: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match &mut st.sink {
            RecorderSink::Disk(sink) => {
                disk::finalize_disk_sink(sink)?;
            }
        }

        Ok(())
    }
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
