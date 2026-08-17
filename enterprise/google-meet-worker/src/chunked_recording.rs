use std::{collections::BTreeMap, error::Error, time::Duration};

use anlg_meeting_capture::RecordingChunk;
use async_trait::async_trait;
use sha2::{Digest, Sha256};

use crate::{AudioFrame, AudioFrameSink, AudioFrameSinkOutput};

const SAMPLE_RATE: u32 = 16_000;
const WAV_HEADER_BYTES: usize = 44;
const MAX_CHUNK_DURATION: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRecordingObject {
    pub uri: String,
}

#[async_trait]
pub trait RecordingChunkStore: Send {
    type Error: Error + Send + Sync + 'static;

    async fn put(
        &mut self,
        key: &str,
        content_type: &str,
        body: Vec<u8>,
    ) -> Result<StoredRecordingObject, Self::Error>;
}

#[derive(Debug, Clone)]
pub struct ChunkedRecordingConfig {
    pub object_prefix: String,
    pub chunk_duration: Duration,
    pub max_lateness: Duration,
}

pub struct ChunkedRecordingSink<S> {
    store: S,
    object_prefix: String,
    chunk_samples: u64,
    lateness_samples: u64,
    chunks: BTreeMap<u64, Vec<i32>>,
    next_chunk_to_flush: u64,
    max_seen_end_sample: u64,
    finished: bool,
}

impl<S> ChunkedRecordingSink<S> {
    pub fn new(
        config: ChunkedRecordingConfig,
        store: S,
    ) -> Result<Self, ChunkedRecordingConfigError> {
        validate_object_prefix(&config.object_prefix)?;
        if config.chunk_duration.is_zero()
            || config.chunk_duration > MAX_CHUNK_DURATION
            || config.max_lateness > config.chunk_duration
        {
            return Err(ChunkedRecordingConfigError::InvalidDurations);
        }
        let chunk_samples = duration_to_samples(config.chunk_duration)
            .map_err(|_| ChunkedRecordingConfigError::InvalidDurations)?;
        let lateness_samples = duration_to_samples(config.max_lateness)
            .map_err(|_| ChunkedRecordingConfigError::InvalidDurations)?;
        Ok(Self {
            store,
            object_prefix: config.object_prefix,
            chunk_samples,
            lateness_samples,
            chunks: BTreeMap::new(),
            next_chunk_to_flush: 0,
            max_seen_end_sample: 0,
            finished: false,
        })
    }
}

#[async_trait]
impl<S> AudioFrameSink for ChunkedRecordingSink<S>
where
    S: RecordingChunkStore,
{
    type Error = ChunkedRecordingError<S::Error>;

    async fn write_frame(
        &mut self,
        frame: AudioFrame,
    ) -> Result<Vec<AudioFrameSinkOutput>, Self::Error> {
        if self.finished {
            return Err(ChunkedRecordingError::AlreadyFinished);
        }
        if frame.sample_rate != SAMPLE_RATE {
            return Err(ChunkedRecordingError::UnsupportedSampleRate(
                frame.sample_rate,
            ));
        }
        let start_sample = milliseconds_to_samples(frame.start_ms)?;
        let end_sample = start_sample
            .checked_add(
                u64::try_from(frame.samples.len())
                    .map_err(|_| ChunkedRecordingError::TimelineOverflow)?,
            )
            .ok_or(ChunkedRecordingError::TimelineOverflow)?;
        let flushed_samples = self
            .next_chunk_to_flush
            .checked_mul(self.chunk_samples)
            .ok_or(ChunkedRecordingError::TimelineOverflow)?;
        if start_sample < flushed_samples {
            return Err(ChunkedRecordingError::LateFrame {
                start_ms: frame.start_ms,
            });
        }

        for (index, sample) in frame.samples.into_iter().enumerate() {
            let absolute_sample = start_sample
                .checked_add(
                    u64::try_from(index).map_err(|_| ChunkedRecordingError::TimelineOverflow)?,
                )
                .ok_or(ChunkedRecordingError::TimelineOverflow)?;
            let chunk_index = absolute_sample / self.chunk_samples;
            let offset = usize::try_from(absolute_sample % self.chunk_samples)
                .map_err(|_| ChunkedRecordingError::TimelineOverflow)?;
            let chunk = self.chunks.entry(chunk_index).or_insert_with(|| {
                vec![0; usize::try_from(self.chunk_samples).expect("validated chunk size")]
            });
            chunk[offset] = chunk[offset].saturating_add(i32::from(sample));
        }
        self.max_seen_end_sample = self.max_seen_end_sample.max(end_sample);

        let flush_through = self
            .max_seen_end_sample
            .saturating_sub(self.lateness_samples);
        let mut output = Vec::new();
        while self.next_chunk_end_sample()? <= flush_through {
            output.push(self.flush_next(self.chunk_samples).await?);
        }
        Ok(output)
    }

    async fn finish(
        &mut self,
        capture_duration: Duration,
    ) -> Result<Vec<AudioFrameSinkOutput>, Self::Error> {
        if self.finished {
            return Ok(Vec::new());
        }
        let total_samples = duration_to_samples(capture_duration)?.max(self.max_seen_end_sample);
        let total_chunks = total_samples.div_ceil(self.chunk_samples);
        let mut output = Vec::new();
        while self.next_chunk_to_flush < total_chunks {
            let chunk_start = self
                .next_chunk_to_flush
                .checked_mul(self.chunk_samples)
                .ok_or(ChunkedRecordingError::TimelineOverflow)?;
            let sample_count = (total_samples - chunk_start).min(self.chunk_samples);
            output.push(self.flush_next(sample_count).await?);
        }
        self.finished = true;
        Ok(output)
    }
}

impl<S> ChunkedRecordingSink<S>
where
    S: RecordingChunkStore,
{
    fn next_chunk_end_sample(&self) -> Result<u64, ChunkedRecordingError<S::Error>> {
        self.next_chunk_to_flush
            .checked_add(1)
            .and_then(|index| index.checked_mul(self.chunk_samples))
            .ok_or(ChunkedRecordingError::TimelineOverflow)
    }

    async fn flush_next(
        &mut self,
        sample_count: u64,
    ) -> Result<AudioFrameSinkOutput, ChunkedRecordingError<S::Error>> {
        let chunk_index = self.next_chunk_to_flush;
        let full_sample_count = usize::try_from(self.chunk_samples)
            .map_err(|_| ChunkedRecordingError::TimelineOverflow)?;
        let sample_count =
            usize::try_from(sample_count).map_err(|_| ChunkedRecordingError::TimelineOverflow)?;
        let mixed = self
            .chunks
            .remove(&chunk_index)
            .unwrap_or_else(|| vec![0; full_sample_count]);
        let body = encode_wav(&mixed[..sample_count])?;
        let size_bytes =
            u64::try_from(body.len()).map_err(|_| ChunkedRecordingError::TimelineOverflow)?;
        let sha256 = hex_digest(Sha256::digest(&body));
        let key = format!("{}/{chunk_index:08}.wav", self.object_prefix);
        let stored = match self.store.put(&key, "audio/wav", body).await {
            Ok(stored) => stored,
            Err(error) => {
                self.chunks.insert(chunk_index, mixed);
                return Err(ChunkedRecordingError::Store(error));
            }
        };
        if let Err(error) = validate_uri(&stored.uri) {
            self.chunks.insert(chunk_index, mixed);
            return Err(error);
        }
        self.next_chunk_to_flush = self
            .next_chunk_to_flush
            .checked_add(1)
            .ok_or(ChunkedRecordingError::TimelineOverflow)?;
        let start_sample = chunk_index
            .checked_mul(self.chunk_samples)
            .ok_or(ChunkedRecordingError::TimelineOverflow)?;
        let chunk_id = hex_digest(Sha256::digest(key.as_bytes()));
        Ok(AudioFrameSinkOutput::RecordingChunkReady(RecordingChunk {
            id: format!("recording-chunk-{chunk_id}"),
            sequence: chunk_index,
            start_ms: samples_to_milliseconds(start_sample),
            duration_ms: samples_to_milliseconds_ceil(
                u64::try_from(sample_count).map_err(|_| ChunkedRecordingError::TimelineOverflow)?,
            ),
            content_type: "audio/wav".into(),
            uri: stored.uri,
            size_bytes: Some(size_bytes),
            sha256: Some(sha256),
        }))
    }
}

fn validate_object_prefix(value: &str) -> Result<(), ChunkedRecordingConfigError> {
    if value.is_empty()
        || value.len() > 512
        || value.starts_with('/')
        || value.ends_with('/')
        || value.split('/').any(|component| {
            component.is_empty()
                || component == "."
                || component == ".."
                || !component
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
        })
    {
        return Err(ChunkedRecordingConfigError::InvalidObjectPrefix);
    }
    Ok(())
}

fn validate_uri<E>(value: &str) -> Result<(), ChunkedRecordingError<E>>
where
    E: Error + Send + Sync + 'static,
{
    if value.is_empty() || value.len() > 2048 || value.chars().any(char::is_control) {
        return Err(ChunkedRecordingError::InvalidStoredUri);
    }
    Ok(())
}

fn duration_to_samples(duration: Duration) -> Result<u64, ChunkedRecordingArithmeticError> {
    let samples = duration
        .as_nanos()
        .checked_mul(u128::from(SAMPLE_RATE))
        .ok_or(ChunkedRecordingArithmeticError)?
        .div_ceil(1_000_000_000);
    u64::try_from(samples).map_err(|_| ChunkedRecordingArithmeticError)
}

fn milliseconds_to_samples(milliseconds: u64) -> Result<u64, ChunkedRecordingArithmeticError> {
    milliseconds
        .checked_mul(u64::from(SAMPLE_RATE))
        .ok_or(ChunkedRecordingArithmeticError)
        .map(|samples| samples / 1000)
}

fn samples_to_milliseconds(samples: u64) -> u64 {
    samples.saturating_mul(1000) / u64::from(SAMPLE_RATE)
}

fn samples_to_milliseconds_ceil(samples: u64) -> u64 {
    samples
        .saturating_mul(1000)
        .div_ceil(u64::from(SAMPLE_RATE))
}

fn encode_wav(mixed: &[i32]) -> Result<Vec<u8>, ChunkedRecordingArithmeticError> {
    let data_bytes = mixed
        .len()
        .checked_mul(2)
        .and_then(|length| u32::try_from(length).ok())
        .ok_or(ChunkedRecordingArithmeticError)?;
    let riff_size = data_bytes
        .checked_add(36)
        .ok_or(ChunkedRecordingArithmeticError)?;
    let capacity = WAV_HEADER_BYTES
        .checked_add(usize::try_from(data_bytes).map_err(|_| ChunkedRecordingArithmeticError)?)
        .ok_or(ChunkedRecordingArithmeticError)?;
    let mut output = Vec::with_capacity(capacity);
    output.extend_from_slice(b"RIFF");
    output.extend_from_slice(&riff_size.to_le_bytes());
    output.extend_from_slice(b"WAVEfmt ");
    output.extend_from_slice(&16_u32.to_le_bytes());
    output.extend_from_slice(&1_u16.to_le_bytes());
    output.extend_from_slice(&1_u16.to_le_bytes());
    output.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    output.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    output.extend_from_slice(&2_u16.to_le_bytes());
    output.extend_from_slice(&16_u16.to_le_bytes());
    output.extend_from_slice(b"data");
    output.extend_from_slice(&data_bytes.to_le_bytes());
    for sample in mixed {
        let sample = (*sample).clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
        output.extend_from_slice(&sample.to_le_bytes());
    }
    Ok(output)
}

fn hex_digest(digest: impl AsRef<[u8]>) -> String {
    use std::fmt::Write;

    digest
        .as_ref()
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to a string cannot fail");
            output
        })
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("recording timeline exceeds supported bounds")]
struct ChunkedRecordingArithmeticError;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ChunkedRecordingConfigError {
    #[error("recording object prefix must be a safe relative path")]
    InvalidObjectPrefix,
    #[error("recording chunk duration must be 1ns-10m and lateness cannot exceed it")]
    InvalidDurations,
}

#[derive(Debug, thiserror::Error)]
pub enum ChunkedRecordingError<E>
where
    E: Error + Send + Sync + 'static,
{
    #[error("recording sink is already finalized")]
    AlreadyFinished,
    #[error("recording sink requires 16000 Hz PCM, got {0} Hz")]
    UnsupportedSampleRate(u32),
    #[error("audio frame at {start_ms}ms arrived after its recording chunk was stored")]
    LateFrame { start_ms: u64 },
    #[error("recording timeline exceeds supported bounds")]
    TimelineOverflow,
    #[error("recording store returned an invalid object URI")]
    InvalidStoredUri,
    #[error("recording object store failed")]
    Store(#[source] E),
}

impl<E> From<ChunkedRecordingArithmeticError> for ChunkedRecordingError<E>
where
    E: Error + Send + Sync + 'static,
{
    fn from(_: ChunkedRecordingArithmeticError) -> Self {
        Self::TimelineOverflow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MemoryStore {
        objects: Vec<(String, String, Vec<u8>)>,
    }

    #[derive(Default)]
    struct CountingStore {
        object_count: usize,
        max_object_bytes: usize,
    }

    #[derive(Debug, thiserror::Error)]
    #[error("memory store failed")]
    struct MemoryStoreError;

    #[async_trait]
    impl RecordingChunkStore for MemoryStore {
        type Error = MemoryStoreError;

        async fn put(
            &mut self,
            key: &str,
            content_type: &str,
            body: Vec<u8>,
        ) -> Result<StoredRecordingObject, Self::Error> {
            self.objects.push((key.into(), content_type.into(), body));
            Ok(StoredRecordingObject { uri: key.into() })
        }
    }

    #[async_trait]
    impl RecordingChunkStore for CountingStore {
        type Error = MemoryStoreError;

        async fn put(
            &mut self,
            key: &str,
            _content_type: &str,
            body: Vec<u8>,
        ) -> Result<StoredRecordingObject, Self::Error> {
            self.object_count += 1;
            self.max_object_bytes = self.max_object_bytes.max(body.len());
            Ok(StoredRecordingObject { uri: key.into() })
        }
    }

    fn config(chunk_duration: Duration) -> ChunkedRecordingConfig {
        ChunkedRecordingConfig {
            object_prefix: "recordings/job-a".into(),
            chunk_duration,
            max_lateness: Duration::ZERO,
        }
    }

    fn frame(sequence: u64, track_index: u32, start_ms: u64, samples: Vec<i16>) -> AudioFrame {
        AudioFrame {
            sequence,
            track_index,
            sample_rate: SAMPLE_RATE,
            start_ms,
            samples,
            speaker: None,
        }
    }

    #[tokio::test]
    async fn mixes_overlapping_tracks_and_emits_playable_wav_metadata() {
        let mut sink =
            ChunkedRecordingSink::new(config(Duration::from_secs(1)), MemoryStore::default())
                .unwrap();
        sink.write_frame(frame(1, 0, 0, vec![20_000]))
            .await
            .unwrap();
        sink.write_frame(frame(2, 1, 0, vec![-10_000]))
            .await
            .unwrap();

        let output = sink.finish(Duration::from_millis(1)).await.unwrap();

        assert_eq!(output.len(), 1);
        let AudioFrameSinkOutput::RecordingChunkReady(chunk) = &output[0] else {
            panic!("expected recording chunk")
        };
        assert_eq!(chunk.duration_ms, 1);
        assert_eq!(chunk.content_type, "audio/wav");
        assert_eq!(chunk.size_bytes, Some(76));
        let body = &sink.store.objects[0].2;
        assert_eq!(&body[..4], b"RIFF");
        assert_eq!(i16::from_le_bytes([body[44], body[45]]), 10_000);
    }

    #[tokio::test]
    async fn writes_explicit_silence_for_gaps() {
        let mut sink =
            ChunkedRecordingSink::new(config(Duration::from_secs(1)), MemoryStore::default())
                .unwrap();
        let output = sink.write_frame(frame(1, 0, 1_000, vec![7])).await.unwrap();
        assert_eq!(output.len(), 1);

        let output = sink.finish(Duration::from_millis(1_001)).await.unwrap();
        assert_eq!(output.len(), 1);
        assert!(
            sink.store.objects[0].2[WAV_HEADER_BYTES..]
                .iter()
                .all(|byte| *byte == 0)
        );
        assert_eq!(
            i16::from_le_bytes([sink.store.objects[1].2[44], sink.store.objects[1].2[45]]),
            7
        );
    }

    #[tokio::test]
    async fn recording_chunk_ids_are_stable_and_job_scoped() {
        let mut first = ChunkedRecordingSink::new(
            ChunkedRecordingConfig {
                object_prefix: "recordings/job-a".into(),
                ..config(Duration::from_secs(1))
            },
            MemoryStore::default(),
        )
        .unwrap();
        let mut second = ChunkedRecordingSink::new(
            ChunkedRecordingConfig {
                object_prefix: "recordings/job-b".into(),
                ..config(Duration::from_secs(1))
            },
            MemoryStore::default(),
        )
        .unwrap();

        let AudioFrameSinkOutput::RecordingChunkReady(first_chunk) = first
            .finish(Duration::from_millis(1))
            .await
            .unwrap()
            .remove(0)
        else {
            panic!("expected recording chunk")
        };
        let AudioFrameSinkOutput::RecordingChunkReady(second_chunk) = second
            .finish(Duration::from_millis(1))
            .await
            .unwrap()
            .remove(0)
        else {
            panic!("expected recording chunk")
        };

        assert_ne!(first_chunk.id, second_chunk.id);
        assert_eq!(first_chunk.id.len(), "recording-chunk-".len() + 64);
    }

    #[tokio::test]
    async fn rejects_frames_for_chunks_that_are_already_durable() {
        let mut sink =
            ChunkedRecordingSink::new(config(Duration::from_secs(1)), MemoryStore::default())
                .unwrap();
        sink.write_frame(frame(1, 0, 1_000, vec![1])).await.unwrap();

        assert!(matches!(
            sink.write_frame(frame(2, 1, 0, vec![1])).await,
            Err(ChunkedRecordingError::LateFrame { start_ms: 0 })
        ));
    }

    #[tokio::test]
    async fn two_hour_recording_keeps_only_two_minute_chunks_buffered() {
        let mut sink = ChunkedRecordingSink::new(
            ChunkedRecordingConfig {
                object_prefix: "recordings/job-long".into(),
                chunk_duration: Duration::from_secs(60),
                max_lateness: Duration::from_secs(60),
            },
            CountingStore::default(),
        )
        .unwrap();
        let mut output_count = 0;
        for minute in 0..120_u64 {
            output_count += sink
                .write_frame(frame(minute + 1, 0, minute * 60_000, vec![1]))
                .await
                .unwrap()
                .len();
            assert!(sink.chunks.len() <= 2);
        }

        output_count += sink
            .finish(Duration::from_secs(2 * 60 * 60))
            .await
            .unwrap()
            .len();

        assert_eq!(output_count, 120);
        assert_eq!(sink.store.object_count, 120);
        assert_eq!(sink.chunks.len(), 0);
        assert_eq!(
            sink.store.max_object_bytes,
            WAV_HEADER_BYTES + 60 * 16_000 * 2
        );
    }

    #[test]
    fn rejects_unsafe_object_prefixes_and_unbounded_chunks() {
        for prefix in ["", "/root", "recordings/../secret", "recordings//job"] {
            assert!(matches!(
                ChunkedRecordingSink::new(
                    ChunkedRecordingConfig {
                        object_prefix: prefix.into(),
                        chunk_duration: Duration::from_secs(1),
                        max_lateness: Duration::ZERO,
                    },
                    MemoryStore::default(),
                ),
                Err(ChunkedRecordingConfigError::InvalidObjectPrefix)
            ));
        }
        assert!(matches!(
            ChunkedRecordingSink::new(
                config(Duration::from_secs(10 * 60 + 1)),
                MemoryStore::default(),
            ),
            Err(ChunkedRecordingConfigError::InvalidDurations)
        ));
    }
}
