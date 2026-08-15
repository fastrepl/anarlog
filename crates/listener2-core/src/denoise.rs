use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anlg_audio_utils::Source;

use crate::DenoiseEvent;
use crate::runtime::DenoiseRuntime;

const DENOISE_SAMPLE_RATE: u32 = 16000;
const CHUNK_SIZE: usize = 16000;
const DENOISE_CANCELLED: &str = "Denoise was cancelled.";

#[derive(Debug, Clone, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct DenoiseParams {
    pub session_id: String,
    pub input_path: PathBuf,
    pub output_path: PathBuf,
}

pub async fn run_denoise(
    runtime: Arc<dyn DenoiseRuntime>,
    params: DenoiseParams,
) -> crate::Result<()> {
    let rt = runtime.clone();
    let session_id = params.session_id.clone();

    let result = tokio::task::spawn_blocking(move || run_denoise_blocking(&runtime, &params))
        .await
        .map_err(|e| crate::Error::DenoiseError(e.to_string()))?;

    if let Err(e) = &result {
        rt.emit(DenoiseEvent::DenoiseFailed {
            session_id,
            error: e.to_string(),
        });
    }

    result
}

fn run_denoise_blocking(
    runtime: &Arc<dyn DenoiseRuntime>,
    params: &DenoiseParams,
) -> crate::Result<()> {
    run_denoise_blocking_with_stats(runtime, params).map(|_| ())
}

#[derive(Debug, Default, PartialEq, Eq)]
struct DenoiseProcessingStats {
    max_buffered_samples: usize,
    processed_frames: usize,
}

fn run_denoise_blocking_with_stats(
    runtime: &Arc<dyn DenoiseRuntime>,
    params: &DenoiseParams,
) -> crate::Result<DenoiseProcessingStats> {
    runtime.emit(DenoiseEvent::DenoiseStarted {
        session_id: params.session_id.clone(),
    });
    ensure_running(runtime)?;

    let metadata = anlg_audio_utils::audio_file_metadata(&params.input_path)
        .map_err(|e| crate::Error::DenoiseError(e.to_string()))?;
    let channels = metadata.channels.max(1) as usize;

    let source = anlg_audio_utils::source_from_path(&params.input_path)
        .map_err(|e| crate::Error::DenoiseError(e.to_string()))?;
    let estimated_frames = source.total_duration().map(|duration| {
        (duration.as_secs_f64() * f64::from(DENOISE_SAMPLE_RATE))
            .ceil()
            .max(1.0) as usize
    });

    let mut denoisers: Vec<anlg_denoise::onnx::Denoiser> = (0..channels)
        .map(|_| anlg_denoise::onnx::Denoiser::new())
        .collect::<Result<_, _>>()
        .map_err(|e| crate::Error::DenoiseError(e.to_string()))?;
    let spec = hound::WavSpec {
        channels: channels as u16,
        sample_rate: DENOISE_SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let output_parent = params
        .output_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let output_temp = tempfile::Builder::new()
        .prefix(".anarlog-denoise-")
        .suffix(".wav")
        .tempfile_in(output_parent)
        .map_err(|e| crate::Error::DenoiseError(e.to_string()))?;
    let output_file = output_temp
        .reopen()
        .map_err(|e| crate::Error::DenoiseError(e.to_string()))?;
    let mut writer = hound::WavWriter::new(BufWriter::new(output_file), spec)
        .map_err(|e| crate::Error::DenoiseError(e.to_string()))?;
    let mut pending = (0..channels)
        .map(|_| Vec::with_capacity(CHUNK_SIZE + 1024))
        .collect::<Vec<_>>();
    let mut stats = DenoiseProcessingStats::default();

    let info = anlg_audio_utils::for_each_resampled_channel_block::<_, crate::Error>(
        source,
        DENOISE_SAMPLE_RATE,
        |channel_block| {
            ensure_running(runtime)?;
            for (pending, samples) in pending.iter_mut().zip(channel_block) {
                pending.extend_from_slice(samples);
            }
            stats.max_buffered_samples = stats.max_buffered_samples.max(buffered_samples(&pending));

            while pending
                .first()
                .is_some_and(|channel| channel.len() >= CHUNK_SIZE)
            {
                process_pending_frames(
                    runtime,
                    params,
                    &mut denoisers,
                    &mut pending,
                    CHUNK_SIZE,
                    estimated_frames,
                    &mut stats,
                    &mut writer,
                )?;
            }
            Ok(())
        },
    )?;

    let remaining_frames = pending.first().map_or(0, Vec::len);
    if remaining_frames > 0 {
        process_pending_frames(
            runtime,
            params,
            &mut denoisers,
            &mut pending,
            remaining_frames,
            Some(info.frame_count.max(1)),
            &mut stats,
            &mut writer,
        )?;
    }
    ensure_running(runtime)?;

    writer
        .finalize()
        .map_err(|e| crate::Error::DenoiseError(e.to_string()))?;
    output_temp
        .persist(&params.output_path)
        .map_err(|e| crate::Error::DenoiseError(e.error.to_string()))?;

    runtime.emit(DenoiseEvent::DenoiseProgress {
        session_id: params.session_id.clone(),
        percentage: 1.0,
    });
    runtime.emit(DenoiseEvent::DenoiseCompleted {
        session_id: params.session_id.clone(),
    });

    Ok(stats)
}

#[allow(clippy::too_many_arguments)]
fn process_pending_frames(
    runtime: &Arc<dyn DenoiseRuntime>,
    params: &DenoiseParams,
    denoisers: &mut [anlg_denoise::onnx::Denoiser],
    pending: &mut [Vec<f32>],
    frame_count: usize,
    estimated_frames: Option<usize>,
    stats: &mut DenoiseProcessingStats,
    writer: &mut hound::WavWriter<BufWriter<std::fs::File>>,
) -> crate::Result<()> {
    ensure_running(runtime)?;
    let mut output_channels = Vec::with_capacity(denoisers.len());
    for (denoiser, channel) in denoisers.iter_mut().zip(pending.iter()) {
        output_channels.push(
            denoiser
                .process_streaming(&channel[..frame_count])
                .map_err(|e| crate::Error::DenoiseError(e.to_string()))?,
        );
    }
    stats.max_buffered_samples = stats
        .max_buffered_samples
        .max(buffered_samples(pending) + output_channels.iter().map(Vec::len).sum::<usize>());

    for frame in 0..frame_count {
        for channel in &output_channels {
            writer
                .write_sample(channel[frame])
                .map_err(|e| crate::Error::DenoiseError(e.to_string()))?;
        }
    }
    for channel in pending {
        channel.drain(..frame_count);
    }

    stats.processed_frames += frame_count;

    if let Some(estimated_frames) = estimated_frames {
        runtime.emit(DenoiseEvent::DenoiseProgress {
            session_id: params.session_id.clone(),
            percentage: (stats.processed_frames as f64 / estimated_frames as f64).clamp(0.0, 0.99),
        });
    }
    ensure_running(runtime)
}

fn buffered_samples(channels: &[Vec<f32>]) -> usize {
    channels.iter().map(Vec::len).sum()
}

fn ensure_running(runtime: &Arc<dyn DenoiseRuntime>) -> crate::Result<()> {
    if runtime.is_cancelled() {
        Err(crate::Error::DenoiseError(DENOISE_CANCELLED.to_string()))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum RecordedEvent {
        Started,
        Progress(f64),
        Completed,
        Failed,
    }

    #[derive(Default)]
    struct TestRuntime {
        events: Mutex<Vec<RecordedEvent>>,
        cancel_after_progress: bool,
        cancelled: AtomicBool,
    }

    impl DenoiseRuntime for TestRuntime {
        fn emit(&self, event: DenoiseEvent) {
            let recorded = match event {
                DenoiseEvent::DenoiseStarted { .. } => RecordedEvent::Started,
                DenoiseEvent::DenoiseProgress { percentage, .. } => {
                    if self.cancel_after_progress {
                        self.cancelled.store(true, Ordering::Release);
                    }
                    RecordedEvent::Progress(percentage)
                }
                DenoiseEvent::DenoiseCompleted { .. } => RecordedEvent::Completed,
                DenoiseEvent::DenoiseFailed { .. } => RecordedEvent::Failed,
            };
            self.events.lock().unwrap().push(recorded);
        }

        fn is_cancelled(&self) -> bool {
            self.cancelled.load(Ordering::Acquire)
        }
    }

    fn write_synthetic_wav(path: &Path, seconds: usize, channels: u16, sample_rate: u32) {
        let mut writer = hound::WavWriter::create(
            path,
            hound::WavSpec {
                channels,
                sample_rate,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .unwrap();
        for frame in 0..seconds * sample_rate as usize {
            for channel in 0..channels {
                let sample = ((frame + channel as usize * 17) as f32 * 0.001).sin() * 0.1;
                writer.write_sample(sample).unwrap();
            }
        }
        writer.finalize().unwrap();
    }

    #[test]
    fn streams_denoised_audio_with_bounded_buffers_and_monotonic_progress() {
        let directory = tempfile::tempdir().unwrap();
        let input_path = directory.path().join("input.wav");
        let output_path = directory.path().join("output.wav");
        write_synthetic_wav(&input_path, 3, 2, 44_100);

        let runtime = Arc::new(TestRuntime::default());
        let stats = run_denoise_blocking_with_stats(
            &(runtime.clone() as Arc<dyn DenoiseRuntime>),
            &DenoiseParams {
                session_id: "session".to_string(),
                input_path,
                output_path: output_path.clone(),
            },
        )
        .unwrap();

        let reader = hound::WavReader::open(output_path).unwrap();
        assert_eq!(reader.spec().channels, 2);
        assert_eq!(reader.spec().sample_rate, DENOISE_SAMPLE_RATE);
        assert_eq!(reader.duration() as usize, stats.processed_frames);
        assert!(stats.max_buffered_samples <= 2 * CHUNK_SIZE * 2 + 2 * 1024);

        let events = runtime.events.lock().unwrap();
        let progress = events
            .iter()
            .filter_map(|event| match event {
                RecordedEvent::Progress(value) => Some(*value),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(progress.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(progress.last(), Some(&1.0));
        assert_eq!(events.last(), Some(&RecordedEvent::Completed));
    }

    #[tokio::test]
    async fn cancellation_keeps_the_existing_destination_and_reports_failure() {
        let directory = tempfile::tempdir().unwrap();
        let input_path = directory.path().join("input.wav");
        let output_path = directory.path().join("output.wav");
        write_synthetic_wav(&input_path, 3, 1, DENOISE_SAMPLE_RATE);
        std::fs::write(&output_path, b"existing").unwrap();

        let runtime = Arc::new(TestRuntime {
            cancel_after_progress: true,
            ..TestRuntime::default()
        });
        let result = run_denoise(
            runtime.clone(),
            DenoiseParams {
                session_id: "session".to_string(),
                input_path,
                output_path: output_path.clone(),
            },
        )
        .await;

        assert!(result.unwrap_err().to_string().contains(DENOISE_CANCELLED));
        assert_eq!(std::fs::read(output_path).unwrap(), b"existing");
        let events = runtime.events.lock().unwrap();
        assert!(events.contains(&RecordedEvent::Failed));
        assert!(!events.contains(&RecordedEvent::Completed));
    }

    #[test]
    #[ignore = "long synthetic-audio memory benchmark"]
    fn long_audio_memory_benchmark_stays_duration_independent() {
        let directory = tempfile::tempdir().unwrap();
        let input_path = directory.path().join("long-input.wav");
        let output_path = directory.path().join("long-output.wav");
        write_synthetic_wav(&input_path, 30 * 60, 2, 44_100);

        let runtime: Arc<dyn DenoiseRuntime> = Arc::new(TestRuntime::default());
        let stats = run_denoise_blocking_with_stats(
            &runtime,
            &DenoiseParams {
                session_id: "benchmark".to_string(),
                input_path,
                output_path,
            },
        )
        .unwrap();

        assert!(stats.max_buffered_samples <= 2 * CHUNK_SIZE * 2 + 2 * 1024);
    }
}
