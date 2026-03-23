mod disk;
mod memory;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use hypr_audio_utils::mix_audio_f32;
use ractor::{Actor, ActorName, ActorProcessingErr, ActorRef, RpcReplyPort};

use crate::{InMemoryRecordingDisposition, RecordingMode};

pub enum RecMsg {
    AudioSingle(Arc<[f32]>),
    AudioDual(Arc<[f32]>, Arc<[f32]>),
    SetStopDispositionAndAck(InMemoryRecordingDisposition, RpcReplyPort<()>),
}

pub struct RecArgs {
    pub app_dir: PathBuf,
    pub session_id: String,
    pub recording_mode: RecordingMode,
}

pub struct RecState {
    sink: RecorderSink,
    stop_disposition: InMemoryRecordingDisposition,
}

enum RecorderSink {
    #[allow(dead_code)]
    Memory(memory::MemorySink),
    Disk(disk::DiskSink),
    Disabled,
}

enum RecorderEncoder {
    Mono(hypr_mp3::MonoStreamEncoder),
    Stereo(hypr_mp3::StereoStreamEncoder),
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

        let sink = match args.recording_mode {
            RecordingMode::Memory => RecorderSink::Disabled,
            RecordingMode::Disk => RecorderSink::Disk(disk::create_disk_sink(&session_dir)?),
        };

        Ok(RecState {
            sink,
            stop_disposition: InMemoryRecordingDisposition::Discard,
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        st: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match (&mut st.sink, msg) {
            (_, RecMsg::SetStopDispositionAndAck(disposition, reply)) => {
                st.stop_disposition = disposition;
                if !reply.is_closed() {
                    let _ = reply.send(());
                }
            }
            (RecorderSink::Memory(sink), RecMsg::AudioSingle(samples)) => {
                sink.encode_single(&samples)?;
            }
            (RecorderSink::Memory(sink), RecMsg::AudioDual(mic, spk)) => {
                sink.encode_dual(&mic, &spk)?;
            }
            (RecorderSink::Disk(sink), RecMsg::AudioSingle(samples)) => {
                disk::write_single(sink, &samples)?;
            }
            (RecorderSink::Disk(sink), RecMsg::AudioDual(mic, spk)) => {
                disk::write_dual(sink, &mic, &spk)?;
            }
            (RecorderSink::Disabled, RecMsg::AudioSingle(_) | RecMsg::AudioDual(_, _)) => {}
        }

        Ok(())
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        st: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match &mut st.sink {
            RecorderSink::Memory(sink) => {
                sink.finalize()?;
                if st.stop_disposition == InMemoryRecordingDisposition::Persist {
                    memory::persist_memory_sink(sink)?;
                }
            }
            RecorderSink::Disk(sink) => {
                disk::finalize_disk_sink(sink)?;
            }
            RecorderSink::Disabled => {}
        }

        Ok(())
    }
}

impl RecorderEncoder {
    fn encode_single(
        &mut self,
        samples: &[f32],
        output: &mut Vec<u8>,
    ) -> Result<(), hypr_mp3::Error> {
        match self {
            Self::Mono(encoder) => encoder.encode_f32(samples, output),
            Self::Stereo(encoder) => encoder.encode_f32(samples, samples, output),
        }
    }

    fn encode_dual(
        &mut self,
        mic: &[f32],
        spk: &[f32],
        output: &mut Vec<u8>,
    ) -> Result<(), hypr_mp3::Error> {
        match self {
            Self::Mono(encoder) => {
                let mixed = mix_audio_f32(mic, spk);
                encoder.encode_f32(&mixed, output)
            }
            Self::Stereo(encoder) => encoder.encode_f32(mic, spk, output),
        }
    }

    fn flush(&mut self, output: &mut Vec<u8>) -> Result<(), hypr_mp3::Error> {
        match self {
            Self::Mono(encoder) => encoder.flush(output),
            Self::Stereo(encoder) => encoder.flush(output),
        }
    }
}

pub fn find_session_dir(sessions_base: &Path, session_id: &str) -> PathBuf {
    if let Some(found) = find_session_dir_recursive(sessions_base, session_id) {
        return found;
    }
    sessions_base.join(session_id)
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
