use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use tauri::async_runtime::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::Error;

const SAMPLE_RATE: u32 = 16_000;
const CHUNK_SIZE: usize = 1_600;
const MAX_RECORDING_SECONDS: u64 = 300;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RecordedAudio {
    pub file_path: String,
    pub duration_ms: u64,
}

struct ActiveRecording {
    cancellation: CancellationToken,
    task: JoinHandle<Result<RecordedAudio, Error>>,
}

pub struct Recorder {
    active: Mutex<Option<ActiveRecording>>,
    completed: Mutex<HashSet<PathBuf>>,
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            active: Mutex::new(None),
            completed: Mutex::new(HashSet::new()),
        }
    }

    pub fn start(
        &self,
        audio: Arc<dyn anlg_audio::AudioProvider>,
        microphone_device: Option<String>,
    ) -> Result<(), Error> {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if active.is_some() {
            return Err(Error::AlreadyRecording);
        }

        let stream = audio
            .open_mic_capture(microphone_device, SAMPLE_RATE, CHUNK_SIZE)
            .map_err(|error| Error::Capture(error.to_string()))?;
        let temporary_file = tempfile::Builder::new()
            .prefix("anarlog-dictation-")
            .suffix(".wav")
            .tempfile()
            .map_err(|error| Error::Recording(error.to_string()))?;
        let (_, path) = temporary_file
            .keep()
            .map_err(|error| Error::Recording(error.error.to_string()))?;

        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();
        let task_path = path.clone();
        let task = tauri::async_runtime::spawn(async move {
            let result = record_to_file(stream, task_cancellation, &task_path).await;
            if result.is_err() {
                let _ = std::fs::remove_file(&task_path);
            }
            result
        });

        *active = Some(ActiveRecording { cancellation, task });
        Ok(())
    }

    pub async fn stop(&self) -> Result<RecordedAudio, Error> {
        let active = self.take_active().ok_or(Error::NotRecording)?;
        active.cancellation.cancel();
        let recorded = active
            .task
            .await
            .map_err(|error| Error::Task(error.to_string()))??;
        self.completed
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(PathBuf::from(&recorded.file_path));
        Ok(recorded)
    }

    pub async fn cancel(&self) -> Result<(), Error> {
        let Some(active) = self.take_active() else {
            return Ok(());
        };

        active.cancellation.cancel();
        match active.task.await {
            Ok(Ok(recorded)) => {
                let _ = std::fs::remove_file(recorded.file_path);
            }
            Ok(Err(_)) => {}
            Err(error) => return Err(Error::Task(error.to_string())),
        }
        Ok(())
    }

    pub fn discard(&self, file_path: String) -> Result<(), Error> {
        let path = PathBuf::from(file_path);
        let removed = self
            .completed
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&path);
        if !removed {
            return Err(Error::UnknownRecording);
        }

        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(Error::Recording(error.to_string())),
        }
    }

    fn take_active(&self) -> Option<ActiveRecording> {
        self.active
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
    }
}

async fn record_to_file(
    mut stream: anlg_audio::CaptureStream,
    cancellation: CancellationToken,
    path: &Path,
) -> Result<RecordedAudio, Error> {
    let specification = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, specification)
        .map_err(|error| Error::Recording(error.to_string()))?;
    let deadline = tokio::time::sleep(std::time::Duration::from_secs(MAX_RECORDING_SECONDS));
    tokio::pin!(deadline);
    let mut sample_count = 0_u64;

    loop {
        tokio::select! {
            _ = cancellation.cancelled() => break,
            _ = &mut deadline => break,
            next = stream.next() => {
                let Some(frame) = next else {
                    break;
                };
                let frame = frame.map_err(|error| Error::Capture(error.to_string()))?;
                let samples = frame.preferred_mic();
                for sample in samples.iter() {
                    writer
                        .write_sample(*sample)
                        .map_err(|error| Error::Recording(error.to_string()))?;
                }
                sample_count += samples.len() as u64;
            }
        }
    }

    writer
        .finalize()
        .map_err(|error| Error::Recording(error.to_string()))?;

    Ok(RecordedAudio {
        file_path: path.to_string_lossy().into_owned(),
        duration_ms: sample_count.saturating_mul(1_000) / u64::from(SAMPLE_RATE),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anlg_audio::{AudioProvider, CaptureConfig, CaptureFrame, CaptureStream};
    use futures_util::stream;

    use super::*;

    struct TestAudio;

    impl AudioProvider for TestAudio {
        fn open_capture(&self, _config: CaptureConfig) -> Result<CaptureStream, anlg_audio::Error> {
            unreachable!()
        }

        fn open_speaker_capture(
            &self,
            _sample_rate: u32,
            _chunk_size: usize,
        ) -> Result<CaptureStream, anlg_audio::Error> {
            unreachable!()
        }

        fn open_mic_capture(
            &self,
            _device: Option<String>,
            _sample_rate: u32,
            _chunk_size: usize,
        ) -> Result<CaptureStream, anlg_audio::Error> {
            let frame = CaptureFrame {
                raw_mic: Arc::from([0.25_f32; CHUNK_SIZE]),
                raw_speaker: Arc::from([]),
                aec_mic: None,
            };
            Ok(CaptureStream::new(stream::unfold(
                frame,
                |frame| async move {
                    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                    Some((Ok(frame.clone()), frame))
                },
            )))
        }

        fn default_device_name(&self) -> String {
            "test-mic".to_string()
        }

        fn list_mic_devices(&self) -> Vec<String> {
            vec![self.default_device_name()]
        }

        fn play_silence(&self) -> std::sync::mpsc::Sender<()> {
            std::sync::mpsc::channel().0
        }

        fn play_bytes(&self, _bytes: &'static [u8]) -> std::sync::mpsc::Sender<()> {
            std::sync::mpsc::channel().0
        }

        fn probe_mic(&self, _device: Option<String>) -> Result<(), anlg_audio::Error> {
            Ok(())
        }

        fn probe_speaker(&self) -> Result<(), anlg_audio::Error> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn records_and_discards_a_temporary_wav() {
        let recorder = Recorder::new();
        recorder.start(Arc::new(TestAudio), None).unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let recorded = recorder.stop().await.unwrap();
        let path = PathBuf::from(&recorded.file_path);
        let reader = hound::WavReader::open(&path).unwrap();

        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().sample_rate, SAMPLE_RATE);
        assert!(recorded.duration_ms > 0);
        assert!(path.exists());

        recorder.discard(recorded.file_path).unwrap();
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn rejects_parallel_recordings() {
        let recorder = Recorder::new();
        recorder.start(Arc::new(TestAudio), None).unwrap();

        assert!(matches!(
            recorder.start(Arc::new(TestAudio), None),
            Err(Error::AlreadyRecording)
        ));

        recorder.cancel().await.unwrap();
    }
}
