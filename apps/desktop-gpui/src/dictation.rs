//! `plugins/dictation`'s `Recorder` and the pure parts of
//! `chat/components/input/use-dictation.ts`: a temporary 16 kHz mono WAV of
//! the microphone, capped at five minutes, and the transcript text the batch
//! words become.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anlg_audio::{AudioProvider, CaptureStream};
use futures_util::StreamExt as _;

pub const SAMPLE_RATE: u32 = 16_000;
pub const CHUNK_SIZE: usize = 1_600;
/// `MAX_DICTATION_SECONDS` / `MAX_RECORDING_SECONDS`
pub const MAX_SECONDS: u64 = 5 * 60;
/// `WAVEFORM_HEIGHTS`: the bar heights of the recording indicator.
pub const WAVEFORM_HEIGHTS: [f32; 14] = [
    3.0, 7.0, 5.0, 10.0, 6.0, 12.0, 8.0, 4.0, 9.0, 6.0, 11.0, 5.0, 8.0, 3.0,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    Starting,
    Recording,
    Transcribing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedAudio {
    pub file_path: PathBuf,
    pub duration_ms: u64,
}

/// One microphone recording in progress.
pub struct Recording {
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<anyhow::Result<RecordedAudio>>,
}

impl Recording {
    /// `Recorder::start`: open the microphone and stream it into a fresh
    /// temporary file until `stop`, `cancel`, or the five-minute cap.
    pub fn start(
        runtime: &tokio::runtime::Handle,
        audio: Arc<dyn AudioProvider>,
        microphone_device: Option<String>,
    ) -> anyhow::Result<Self> {
        // Providers spawn their capture loop on the ambient runtime (the
        // plugin opens the stream inside a Tauri command).
        let stream = {
            let _runtime = runtime.enter();
            audio.open_mic_capture(microphone_device, SAMPLE_RATE, CHUNK_SIZE)?
        };
        let (_, path) = tempfile::Builder::new()
            .prefix("anarlog-dictation-")
            .suffix(".wav")
            .tempfile()?
            .keep()
            .map_err(|error| error.error)?;
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let task = runtime.spawn(async move {
            let result = record_to_file(stream, stopped, &path).await;
            if result.is_err() {
                let _ = std::fs::remove_file(&path);
            }
            result
        });
        Ok(Self {
            stop: Some(stop),
            task,
        })
    }

    /// `Recorder::stop`: finish the file and hand it over.
    pub async fn stop(mut self) -> anyhow::Result<RecordedAudio> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.task.await?
    }

    /// `Recorder::cancel`: stop and delete whatever was written.
    pub async fn cancel(self) {
        if let Ok(recorded) = self.stop().await {
            let _ = std::fs::remove_file(recorded.file_path);
        }
    }
}

async fn record_to_file(
    mut stream: CaptureStream,
    mut stopped: tokio::sync::oneshot::Receiver<()>,
    path: &Path,
) -> anyhow::Result<RecordedAudio> {
    let specification = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, specification)?;
    let deadline = tokio::time::sleep(std::time::Duration::from_secs(MAX_SECONDS));
    tokio::pin!(deadline);
    let mut sample_count = 0_u64;
    loop {
        tokio::select! {
            _ = &mut stopped => break,
            _ = &mut deadline => break,
            next = stream.next() => {
                let Some(frame) = next else {
                    break;
                };
                let samples = frame?.preferred_mic();
                for sample in samples.iter() {
                    writer.write_sample(*sample)?;
                }
                sample_count += samples.len() as u64;
            }
        }
    }
    writer.finalize()?;
    Ok(RecordedAudio {
        file_path: path.to_path_buf(),
        duration_ms: sample_count.saturating_mul(1_000) / u64::from(SAMPLE_RATE),
    })
}

/// `discardTemporaryRecording`
pub fn discard(recorded: &RecordedAudio) {
    if let Err(error) = std::fs::remove_file(&recorded.file_path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(%error, "[chat-dictation] failed to discard temporary recording");
    }
}

/// `handlePersist`: the words in time order, joined, blanks collapsed.
pub fn transcript_text(words: &[crate::batch::BatchWord]) -> String {
    let mut ordered: Vec<&crate::batch::BatchWord> = words.iter().collect();
    ordered.sort_by_key(|word| word.start_ms);
    ordered
        .iter()
        .map(|word| word.text.as_str())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// `formatElapsedTime`: `m:ss`.
pub fn format_elapsed(seconds: u64) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// `/no speech|empty transcript/i`: the failures shown as the softer
/// `No speech detected` warning.
pub fn is_no_speech(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("no speech") || lower.contains("empty transcript")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, start_ms: i64) -> crate::batch::BatchWord {
        crate::batch::BatchWord {
            text: text.to_string(),
            start_ms,
            end_ms: start_ms + 200,
            channel: 0,
            metadata: serde_json::Value::Null,
            speaker_index: None,
        }
    }

    #[test]
    fn transcript_text_orders_and_collapses() {
        let words = [word(" world", 500), word("Hello", 0), word("  again ", 900)];
        assert_eq!(transcript_text(&words), "Hello world again");
        assert_eq!(transcript_text(&[]), "");
        assert_eq!(transcript_text(&[word("  ", 0)]), "");
    }

    #[test]
    fn elapsed_and_error_classification() {
        assert_eq!(format_elapsed(0), "0:00");
        assert_eq!(format_elapsed(65), "1:05");
        assert_eq!(format_elapsed(MAX_SECONDS), "5:00");
        assert!(is_no_speech("No speech was detected in the audio."));
        assert!(is_no_speech("Empty transcript"));
        assert!(!is_no_speech("connection refused"));
    }

    struct TestAudio;

    impl AudioProvider for TestAudio {
        fn open_capture(
            &self,
            _config: anlg_audio::CaptureConfig,
        ) -> Result<CaptureStream, anlg_audio::Error> {
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
            let frame = anlg_audio::CaptureFrame {
                raw_mic: Arc::from([0.25_f32; CHUNK_SIZE]),
                raw_speaker: Arc::from([]),
                aec_mic: None,
            };
            Ok(CaptureStream::new(futures_util::stream::unfold(
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
        let recording = Recording::start(
            &tokio::runtime::Handle::current(),
            Arc::new(TestAudio),
            None,
        )
        .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let recorded = recording.stop().await.unwrap();
        let reader = hound::WavReader::open(&recorded.file_path).unwrap();
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().sample_rate, SAMPLE_RATE);
        assert!(recorded.duration_ms > 0);
        assert!(recorded.file_path.exists());
        discard(&recorded);
        assert!(!recorded.file_path.exists());

        let cancelled = Recording::start(
            &tokio::runtime::Handle::current(),
            Arc::new(TestAudio),
            None,
        )
        .unwrap();
        cancelled.cancel().await;
    }
}
