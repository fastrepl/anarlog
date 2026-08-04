use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use anlg_model_manager::ModelManager;
use anlg_transcribe_core::{
    BatchEventSender, ProgressTracker, batch_event_channel, batch_sse_response,
    chunk_channel_audio, json_error_response,
};
use axum::{
    Json,
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use futures_util::StreamExt;
use owhisper_interface::ListenParams;
use owhisper_interface::batch;
use owhisper_interface::batch_sse::BatchSseMessage;
use rodio::Source;
use tokio::io::AsyncWriteExt;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use super::response::{TranscriptKind, build_batch_words, build_transcript_response};
use super::{TARGET_SAMPLE_RATE, build_metadata, build_model, transcribe_chunk};

const MAX_BATCH_AUDIO_BODY_BYTES: usize = 100 * 1024 * 1024;
const MAX_BATCH_CHANNELS: usize = 8;
const MAX_CONCURRENT_HTTP_BATCH_JOBS: usize = 1;
const MAX_REJECTED_BATCH_DRAIN_BYTES: usize = 64 * 1024;
const MAX_REJECTED_BATCH_DRAIN_CHUNKS: usize = 64;
const REJECTED_BATCH_DRAIN_TIMEOUT: Duration = Duration::from_millis(100);
const CHANNEL_WINDOW_SAMPLES: usize = TARGET_SAMPLE_RATE as usize * 2 * 60;

#[derive(Clone, Default)]
struct BatchCancellation {
    cancelled: Arc<AtomicBool>,
}

impl BatchCancellation {
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

struct CancelBatchOnDrop(BatchCancellation);

impl Drop for CancelBatchOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

pub(super) fn http_batch_admission() -> Arc<Semaphore> {
    Arc::new(Semaphore::new(MAX_CONCURRENT_HTTP_BATCH_JOBS))
}

pub(super) fn try_acquire_http_batch_permit(
    admission: Arc<Semaphore>,
) -> Option<OwnedSemaphorePermit> {
    admission.try_acquire_owned().ok()
}

pub(super) fn batch_busy_response() -> Response {
    json_error_response(
        StatusCode::TOO_MANY_REQUESTS,
        "batch_busy",
        "another local batch transcription is already running",
    )
}

fn spawn_batch_job<T>(
    permit: OwnedSemaphorePermit,
    job: impl FnOnce() -> T + Send + 'static,
) -> tokio::task::JoinHandle<T>
where
    T: Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        job()
    })
}

#[derive(Debug)]
pub(super) struct BatchAudioFile {
    file: tempfile::NamedTempFile,
    len: u64,
}

impl BatchAudioFile {
    fn path(&self) -> &Path {
        self.file.path()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[derive(Debug)]
enum BatchAudioWriteError {
    Body(axum::Error),
    Io(std::io::Error),
    TooLarge,
}

impl From<std::io::Error> for BatchAudioWriteError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

pub(super) async fn spool_batch_audio(
    body: Body,
    content_type: &str,
) -> Result<BatchAudioFile, Response> {
    spool_batch_audio_with_limit(body, content_type, MAX_BATCH_AUDIO_BODY_BYTES)
        .await
        .map_err(|error| match error {
            BatchAudioWriteError::TooLarge => json_error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "payload_too_large",
                format!("request body exceeds {MAX_BATCH_AUDIO_BODY_BYTES} bytes"),
            ),
            BatchAudioWriteError::Body(error) => json_error_response(
                StatusCode::BAD_REQUEST,
                "invalid_request_body",
                error.to_string(),
            ),
            BatchAudioWriteError::Io(error) => json_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed_to_store_audio",
                error.to_string(),
            ),
        })
}

pub(super) async fn drain_rejected_batch_audio(body: Body) -> Result<(), Response> {
    let drain = async move {
        let mut stream = body.into_data_stream();
        let mut drained = 0usize;

        for _ in 0..MAX_REJECTED_BATCH_DRAIN_CHUNKS {
            if drained >= MAX_REJECTED_BATCH_DRAIN_BYTES {
                break;
            }

            let Some(chunk) = stream.next().await else {
                break;
            };
            let chunk = chunk.map_err(|error| {
                json_error_response(
                    StatusCode::BAD_REQUEST,
                    "invalid_request_body",
                    error.to_string(),
                )
            })?;
            drained = drained.saturating_add(chunk.len());
        }

        Ok(())
    };

    match tokio::time::timeout(REJECTED_BATCH_DRAIN_TIMEOUT, drain).await {
        Ok(result) => result,
        Err(_) => Ok(()),
    }
}

async fn spool_batch_audio_with_limit(
    body: Body,
    content_type: &str,
    max_bytes: usize,
) -> Result<BatchAudioFile, BatchAudioWriteError> {
    let extension = anlg_audio_utils::content_type_to_extension(content_type);
    let file = tempfile::Builder::new()
        .prefix("whisper_local_batch_")
        .suffix(&format!(".{extension}"))
        .tempfile()?;
    let mut writer = tokio::fs::File::from_std(file.reopen()?);
    let mut stream = body.into_data_stream();
    let mut len = 0u64;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(BatchAudioWriteError::Body)?;
        len = len.saturating_add(chunk.len() as u64);
        if len > max_bytes as u64 {
            return Err(BatchAudioWriteError::TooLarge);
        }
        writer.write_all(&chunk).await?;
    }
    writer.flush().await?;

    Ok(BatchAudioFile { file, len })
}

pub(super) async fn handle_batch(
    audio_file: BatchAudioFile,
    params: &ListenParams,
    manager: &ModelManager<anlg_whisper_local::LoadedWhisper>,
    model_path: &Path,
    permit: OwnedSemaphorePermit,
) -> Response {
    let cancellation = BatchCancellation::default();
    let _cancel_on_drop = CancelBatchOnDrop(cancellation.clone());
    let model = match manager.get(None).await {
        Ok(model) => model,
        Err(error) => {
            tracing::error!(error = %error, "failed_to_load_model");
            return json_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "model_load_failed",
                error.to_string(),
            );
        }
    };

    let model = model.clone();
    let model_path = model_path.to_path_buf();
    let params = params.clone();

    match spawn_batch_job(permit, move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            transcribe_batch(
                audio_file.path(),
                &params,
                model.as_ref(),
                &model_path,
                None,
                &cancellation,
            )
        }))
    })
    .await
    {
        Ok(Ok(Ok(response))) => Json(response).into_response(),
        Ok(Ok(Err(error))) => {
            tracing::error!(error = %error, "batch_transcription_failed");
            json_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "transcription_failed",
                error.to_string(),
            )
        }
        Ok(Err(_)) | Err(_) => json_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "transcription_failed",
            "task panicked",
        ),
    }
}

pub(super) async fn handle_batch_sse(
    audio_file: BatchAudioFile,
    params: &ListenParams,
    manager: &ModelManager<anlg_whisper_local::LoadedWhisper>,
    model_path: &Path,
    permit: OwnedSemaphorePermit,
) -> Response {
    let model = match manager.get(None).await {
        Ok(model) => model,
        Err(error) => {
            tracing::error!(error = %error, "failed_to_load_model");
            return json_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "model_load_failed",
                error.to_string(),
            );
        }
    };

    let model = model.clone();
    let model_path = model_path.to_path_buf();
    let params = params.clone();
    let (event_tx, event_rx) = batch_event_channel();
    let cancellation = BatchCancellation::default();

    spawn_batch_job(permit, move || {
        let message = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            transcribe_batch(
                audio_file.path(),
                &params,
                model.as_ref(),
                &model_path,
                Some(event_tx.clone()),
                &cancellation,
            )
        })) {
            Ok(Ok(response)) => BatchSseMessage::Result { response },
            Ok(Err(error)) => BatchSseMessage::Error {
                error: "transcription_failed".to_string(),
                detail: error.to_string(),
            },
            Err(_) => BatchSseMessage::Error {
                error: "transcription_failed".to_string(),
                detail: "task panicked".to_string(),
            },
        };

        event_tx.send_terminal(message);
    });

    batch_sse_response(event_rx)
}

fn transcribe_batch(
    audio_path: &Path,
    params: &ListenParams,
    loaded_model: &anlg_whisper_local::LoadedWhisper,
    model_path: &Path,
    event_tx: Option<BatchEventSender>,
    cancellation: &BatchCancellation,
) -> Result<batch::Response, crate::Error> {
    let source = anlg_audio_utils::source_from_path(audio_path)?;
    transcribe_source(
        source,
        params,
        loaded_model,
        model_path,
        event_tx,
        cancellation,
    )
}

pub(super) fn transcribe_recorded_file(
    loaded_model: &anlg_whisper_local::LoadedWhisper,
    model_path: &Path,
    audio_path: &Path,
) -> Result<Vec<owhisper_interface::Word2>, crate::Error> {
    let source = anlg_audio_utils::source_from_path(audio_path)?;
    let cancellation = BatchCancellation::default();
    let response = transcribe_source(
        source,
        &ListenParams::default(),
        loaded_model,
        model_path,
        None,
        &cancellation,
    )?;
    let words = response
        .results
        .channels
        .into_iter()
        .flat_map(|channel| channel.alternatives.into_iter())
        .flat_map(|alt| alt.words.into_iter())
        .map(|word| owhisper_interface::Word2 {
            text: word.punctuated_word.unwrap_or(word.word),
            speaker: word
                .speaker
                .map(|speaker| owhisper_interface::SpeakerIdentity::Unassigned {
                    index: speaker as u8,
                }),
            confidence: Some(word.confidence as f32),
            start_ms: Some((word.start * 1000.0) as u64),
            end_ms: Some((word.end * 1000.0) as u64),
        })
        .collect();
    Ok(words)
}

fn transcribe_source<S>(
    source: S,
    params: &ListenParams,
    loaded_model: &anlg_whisper_local::LoadedWhisper,
    model_path: &Path,
    event_tx: Option<BatchEventSender>,
    cancellation: &BatchCancellation,
) -> Result<batch::Response, crate::Error>
where
    S: Source<Item = f32>,
{
    let channel_files = resample_to_channel_files(source, event_tx.as_ref(), cancellation)?;
    let channel_count = channel_files.len();
    let channel_durations = channel_files
        .iter()
        .map(|channel| channel.sample_count as f64 / TARGET_SAMPLE_RATE as f64)
        .collect::<Vec<_>>();
    let total_duration = channel_durations.iter().copied().fold(0.0_f64, f64::max);

    let metadata = build_metadata(model_path);
    let mut model = build_model(loaded_model, params)?;
    let mut response_channels = Vec::with_capacity(channel_count);
    let mut progress = ProgressTracker::new(vec![0.0; channel_count], total_duration, event_tx);
    ensure_batch_active(&progress, cancellation)?;
    progress.emit(None);

    for (channel_idx, channel) in channel_files.into_iter().enumerate() {
        ensure_batch_active(&progress, cancellation)?;
        let channel_index = [channel_idx as i32, channel_count as i32];
        let channel_duration = channel_durations[channel_idx];
        let chunks = ChannelChunkIterator::new(channel)?;
        let mut work = BatchWorkContext {
            progress: &mut progress,
            cancellation,
        };

        let (words, transcript, avg_confidence) = transcribe_channel_chunks(
            channel_idx,
            chunks,
            channel_duration,
            &mut model,
            &mut work,
            &metadata,
            &channel_index,
        )?;

        response_channels.push(batch::Channel {
            alternatives: vec![batch::Alternatives {
                transcript,
                confidence: avg_confidence,
                words,
            }],
        });
    }

    let mut metadata_json = serde_json::to_value(&metadata).unwrap_or_default();
    if let Some(obj) = metadata_json.as_object_mut() {
        obj.insert("duration".to_string(), serde_json::json!(total_duration));
        obj.insert(
            "channels".to_string(),
            serde_json::json!(response_channels.len()),
        );
    }

    Ok(batch::Response {
        metadata: metadata_json,
        results: batch::Results {
            channels: response_channels,
        },
    })
}

#[derive(Debug)]
struct ResampledChannelFile {
    file: tempfile::NamedTempFile,
    sample_count: usize,
}

fn resample_to_channel_files<S>(
    source: S,
    event_tx: Option<&BatchEventSender>,
    cancellation: &BatchCancellation,
) -> Result<Vec<ResampledChannelFile>, crate::Error>
where
    S: Source<Item = f32>,
{
    let channel_count = u16::from(source.channels()).max(1) as usize;
    if channel_count > MAX_BATCH_CHANNELS {
        return Err(crate::Error::protocol(format!(
            "whisper-local batch transcription supports at most {MAX_BATCH_CHANNELS} audio channels; the recording declares {channel_count}"
        )));
    }

    let files = (0..channel_count)
        .map(|_| {
            tempfile::Builder::new()
                .prefix("whisper_local_channel_")
                .suffix(".wav")
                .tempfile()
        })
        .collect::<std::io::Result<Vec<_>>>()?;
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writers = files
        .iter()
        .map(|file| {
            let writer = BufWriter::new(file.reopen()?);
            hound::WavWriter::new(writer, spec).map_err(crate::Error::from)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let info = anlg_audio_utils::for_each_resampled_channel_block::<_, crate::Error>(
        source,
        TARGET_SAMPLE_RATE,
        |channels| {
            if cancellation.is_cancelled() {
                return Err(batch_request_cancelled());
            }
            if event_tx.is_some_and(BatchEventSender::is_closed) {
                return Err(batch_receiver_unavailable());
            }
            for (writer, channel) in writers.iter_mut().zip(channels) {
                for sample in *channel {
                    writer.write_sample(*sample)?;
                }
            }
            Ok(())
        },
    )?;

    for writer in writers {
        writer.finalize()?;
    }

    Ok(files
        .into_iter()
        .map(|file| ResampledChannelFile {
            file,
            sample_count: info.frame_count,
        })
        .collect())
}

struct ChannelChunkIterator {
    reader: hound::WavReader<BufReader<File>>,
    _file: tempfile::NamedTempFile,
    pending: std::vec::IntoIter<anlg_audio_chunking::AudioChunk>,
    next_window_start: usize,
    max_window_samples: usize,
    finished: bool,
}

impl ChannelChunkIterator {
    fn new(channel: ResampledChannelFile) -> Result<Self, crate::Error> {
        Self::new_with_window_samples(channel, CHANNEL_WINDOW_SAMPLES)
    }

    fn new_with_window_samples(
        channel: ResampledChannelFile,
        max_window_samples: usize,
    ) -> Result<Self, crate::Error> {
        let reader = hound::WavReader::open(channel.file.path())?;
        Ok(Self {
            reader,
            _file: channel.file,
            pending: Vec::new().into_iter(),
            next_window_start: 0,
            max_window_samples: max_window_samples.max(1),
            finished: false,
        })
    }

    fn read_next_window(&mut self) -> Result<Option<Vec<f32>>, crate::Error> {
        let samples = self
            .reader
            .samples::<f32>()
            .take(self.max_window_samples)
            .collect::<Result<Vec<_>, _>>()?;
        Ok((!samples.is_empty()).then_some(samples))
    }
}

impl Iterator for ChannelChunkIterator {
    type Item = Result<anlg_audio_chunking::AudioChunk, crate::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(chunk) = self.pending.next() {
                return Some(Ok(chunk));
            }
            if self.finished {
                return None;
            }

            let samples = match self.read_next_window() {
                Ok(Some(samples)) => samples,
                Ok(None) => {
                    self.finished = true;
                    return None;
                }
                Err(error) => {
                    self.finished = true;
                    return Some(Err(error));
                }
            };

            let window_start = self.next_window_start;
            self.next_window_start += samples.len();
            let mut chunks = match chunk_channel_audio::<crate::Error>(&samples) {
                Ok(chunks) => chunks,
                Err(error) => {
                    self.finished = true;
                    return Some(Err(error));
                }
            };
            for chunk in &mut chunks {
                chunk.sample_start += window_start;
                chunk.sample_end += window_start;
            }
            self.pending = chunks.into_iter();
        }
    }
}

struct BatchWorkContext<'a> {
    progress: &'a mut ProgressTracker,
    cancellation: &'a BatchCancellation,
}

fn transcribe_channel_chunks(
    channel_idx: usize,
    chunks: impl Iterator<Item = Result<anlg_audio_chunking::AudioChunk, crate::Error>>,
    channel_duration: f64,
    model: &mut anlg_whisper_local::Whisper,
    work: &mut BatchWorkContext<'_>,
    metadata: &owhisper_interface::stream::Metadata,
    channel_index: &[i32],
) -> Result<(Vec<batch::Word>, String, f64), crate::Error> {
    let mut all_words = Vec::new();
    let mut transcript = String::new();
    let mut cumulative_confidence = 0.0;
    let mut segment_count = 0usize;

    for chunk in chunks {
        ensure_batch_active(work.progress, work.cancellation)?;
        let chunk = chunk?;
        let chunk_start_sec = chunk.sample_start as f64 / TARGET_SAMPLE_RATE as f64;
        work.progress.update_channel(channel_idx, chunk_start_sec);

        let segments = transcribe_chunk(model, &chunk.samples, chunk_start_sec)?;
        ensure_batch_active(work.progress, work.cancellation)?;
        for segment in segments {
            cumulative_confidence += segment.confidence;
            segment_count += 1;
            all_words.extend(build_batch_words(&segment, channel_idx as i32));

            if let Some(tx) = work.progress.event_tx()
                && !tx.send_blocking(BatchSseMessage::Segment {
                    response: build_transcript_response(
                        &segment,
                        TranscriptKind::Confirmed,
                        metadata,
                        channel_index,
                    ),
                })
            {
                return Err(batch_receiver_unavailable());
            }

            append_transcript(&mut transcript, &segment.text);
        }

        work.progress.update_channel(
            channel_idx,
            chunk.sample_end as f64 / TARGET_SAMPLE_RATE as f64,
        );
        work.progress.emit(Some(transcript.clone()));
        ensure_batch_active(work.progress, work.cancellation)?;
    }
    work.progress.update_channel(channel_idx, channel_duration);
    work.progress.emit(Some(transcript.clone()));

    let avg_confidence = if segment_count == 0 {
        0.0
    } else {
        cumulative_confidence / segment_count as f64
    };

    Ok((all_words, transcript, avg_confidence))
}

fn append_transcript(transcript: &mut String, text: &str) {
    if text.is_empty() {
        return;
    }
    if !transcript.is_empty() {
        transcript.push(' ');
    }
    transcript.push_str(text);
}

fn ensure_batch_active(
    progress: &ProgressTracker,
    cancellation: &BatchCancellation,
) -> Result<(), crate::Error> {
    if cancellation.is_cancelled() {
        return Err(batch_request_cancelled());
    }
    if progress.is_cancelled() {
        return Err(batch_receiver_unavailable());
    }
    Ok(())
}

fn batch_receiver_unavailable() -> crate::Error {
    crate::Error::protocol("batch event receiver unavailable")
}

fn batch_request_cancelled() -> crate::Error {
    crate::Error::protocol("batch request cancelled")
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::{Duration, Instant};

    use anlg_transcribe_core::{ProgressTracker, batch_event_channel};
    use axum::body::{Body, Bytes};
    use axum::http::StatusCode;
    use tokio::sync::Semaphore;

    use super::{
        BatchAudioWriteError, BatchCancellation, CancelBatchOnDrop, ChannelChunkIterator,
        MAX_REJECTED_BATCH_DRAIN_BYTES, ResampledChannelFile, batch_busy_response,
        drain_rejected_batch_audio, ensure_batch_active, resample_to_channel_files,
        spawn_batch_job, spool_batch_audio_with_limit, try_acquire_http_batch_permit,
    };

    #[test]
    fn disconnected_sse_receiver_cancels_batch_work() {
        let (tx, rx) = batch_event_channel();
        drop(rx);
        let progress = ProgressTracker::new(vec![0.0], 1.0, Some(tx));
        let cancellation = BatchCancellation::default();

        assert!(ensure_batch_active(&progress, &cancellation).is_err());
    }

    #[test]
    fn dropping_request_guard_cancels_batch_work() {
        let progress = ProgressTracker::new(vec![0.0], 1.0, None);
        let cancellation = BatchCancellation::default();
        let guard = CancelBatchOnDrop(cancellation.clone());

        assert!(ensure_batch_active(&progress, &cancellation).is_ok());
        drop(guard);
        let error = ensure_batch_active(&progress, &cancellation).unwrap_err();
        assert!(error.to_string().contains("batch request cancelled"));
    }

    #[test]
    fn batch_admission_rejects_work_instead_of_queueing_it() {
        let admission = Arc::new(Semaphore::new(1));
        let permit = try_acquire_http_batch_permit(Arc::clone(&admission)).unwrap();

        assert!(try_acquire_http_batch_permit(Arc::clone(&admission)).is_none());
        assert_eq!(
            batch_busy_response().status(),
            StatusCode::TOO_MANY_REQUESTS
        );

        drop(permit);
        assert!(try_acquire_http_batch_permit(admission).is_some());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn detached_blocking_job_keeps_its_admission_permit() {
        let admission = Arc::new(Semaphore::new(1));
        let permit = try_acquire_http_batch_permit(Arc::clone(&admission)).unwrap();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();

        let job = spawn_batch_job(permit, move || {
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        });
        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        drop(job);

        assert!(try_acquire_http_batch_permit(Arc::clone(&admission)).is_none());

        release_tx.send(()).unwrap();
        let permit = tokio::time::timeout(Duration::from_secs(1), admission.acquire_owned())
            .await
            .unwrap()
            .unwrap();
        drop(permit);
    }

    #[tokio::test]
    async fn batch_request_body_is_spooled_to_disk() {
        let audio = spool_batch_audio_with_limit(Body::from("audio"), "audio/wav", 16)
            .await
            .unwrap();

        assert_eq!(audio.len, 5);
        assert_eq!(std::fs::read(audio.path()).unwrap(), b"audio");
    }

    #[tokio::test]
    async fn small_rejected_batch_body_is_drained() {
        drain_rejected_batch_audio(Body::from("audio"))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn rejected_batch_body_drain_stops_at_its_byte_bound() {
        const CHUNK_BYTES: usize = 8 * 1_024;

        let chunks_read = Arc::new(AtomicUsize::new(0));
        let observed_chunks = Arc::clone(&chunks_read);
        let stream = futures_util::stream::iter((0..1_000).map(move |_| {
            observed_chunks.fetch_add(1, Ordering::SeqCst);
            Ok::<_, Infallible>(Bytes::from(vec![0; CHUNK_BYTES]))
        }));

        drain_rejected_batch_audio(Body::from_stream(stream))
            .await
            .unwrap();

        assert_eq!(
            chunks_read.load(Ordering::SeqCst),
            MAX_REJECTED_BATCH_DRAIN_BYTES.div_ceil(CHUNK_BYTES)
        );
    }

    #[tokio::test]
    async fn rejected_batch_body_drain_times_out_on_a_slow_client() {
        let body =
            Body::from_stream(futures_util::stream::pending::<Result<Bytes, std::io::Error>>());
        let started = Instant::now();

        drain_rejected_batch_audio(body).await.unwrap();

        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn oversized_batch_request_is_rejected_while_spooling() {
        let error = spool_batch_audio_with_limit(Body::from("12345"), "audio/wav", 4)
            .await
            .unwrap_err();

        assert!(matches!(error, BatchAudioWriteError::TooLarge));
    }

    #[test]
    fn unsupported_channel_count_is_rejected_before_resampling() {
        let source = rodio::buffer::SamplesBuffer::new(
            std::num::NonZeroU16::new(9).unwrap(),
            std::num::NonZeroU32::new(16_000).unwrap(),
            Vec::new(),
        );

        let cancellation = BatchCancellation::default();
        let error = resample_to_channel_files(source, None, &cancellation).unwrap_err();

        assert!(error.to_string().contains("at most 8 audio channels"));
        assert!(error.to_string().contains("declares 9"));
    }

    #[test]
    fn channel_audio_is_read_in_bounded_windows() {
        let file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(file.path(), spec).unwrap();
        for sample in 0..10 {
            writer.write_sample(sample as f32).unwrap();
        }
        writer.finalize().unwrap();
        let channel = ResampledChannelFile {
            file,
            sample_count: 10,
        };
        let mut reader = ChannelChunkIterator::new_with_window_samples(channel, 4).unwrap();

        assert_eq!(reader.read_next_window().unwrap().unwrap().len(), 4);
        assert_eq!(reader.read_next_window().unwrap().unwrap().len(), 4);
        assert_eq!(reader.read_next_window().unwrap().unwrap().len(), 2);
        assert!(reader.read_next_window().unwrap().is_none());
    }
}
