use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::UNIX_EPOCH;

use anlg_audio_utils::Source;

pub const PEAKS_PER_CHANNEL: usize = 8000;
const MAX_CHANNELS: usize = 2;
const PRECISION: f32 = 10_000.0;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AudioPeaks {
    pub duration: f64,
    pub channels: Vec<Vec<f32>>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceStamp {
    filename: String,
    size_bytes: u64,
    modified_ns: u128,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachedPeaks {
    source: SourceStamp,
    peaks: AudioPeaks,
}

/// Returns the waveform peaks for `audio_path`, reusing `cache_path` while the
/// audio file is unchanged.
pub fn cached_peaks(audio_path: &Path, cache_path: &Path) -> io::Result<AudioPeaks> {
    let lock = cache_lock(cache_path);
    let _guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    let source = source_stamp(audio_path)?;
    if let Some(peaks) = read_cache(cache_path, &source) {
        return Ok(peaks);
    }

    let peaks = compute_peaks(audio_path, PEAKS_PER_CHANNEL)?;
    // The recording may have been deleted or replaced while decoding.
    if source_stamp(audio_path).ok().as_ref() != Some(&source) {
        return Ok(peaks);
    }
    if let Err(error) = write_cache(cache_path, source, &peaks) {
        tracing::warn!(?error, "audio_peaks_cache_write_failed");
    }
    Ok(peaks)
}

// Concurrent requests for the same recording wait for the first computation
// and then read its cache instead of decoding the audio again.
fn cache_lock(cache_path: &Path) -> Arc<Mutex<()>> {
    static LOCKS: LazyLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> =
        LazyLock::new(Default::default);
    let mut locks = LOCKS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    locks.retain(|_, lock| Arc::strong_count(lock) > 1);
    locks.entry(cache_path.to_path_buf()).or_default().clone()
}

pub fn compute_peaks(audio_path: &Path, peaks_per_channel: usize) -> io::Result<AudioPeaks> {
    let source = anlg_audio_utils::source_from_path(audio_path).map_err(io::Error::other)?;
    source_peaks(source, peaks_per_channel)
}

fn source_peaks(source: impl Source, peaks_per_channel: usize) -> io::Result<AudioPeaks> {
    let peaks_per_channel = peaks_per_channel.max(1);
    let channels = usize::from(source.channels().get());
    let sample_rate = f64::from(source.sample_rate().get());
    let kept_channels = channels.min(MAX_CHANNELS);

    // Per-block max magnitudes; adjacent blocks are merged whenever the
    // buffer doubles so memory stays bounded for arbitrarily long files.
    let mut blocks = vec![Vec::with_capacity(peaks_per_channel * 2); kept_channels];
    let mut block_frames = 1_usize;
    let mut current = vec![0_f32; kept_channels];
    let mut frames_in_block = 0_usize;
    let mut total_frames = 0_u64;
    let mut channel = 0_usize;

    for sample in source {
        if channel < kept_channels {
            let magnitude = if sample.is_finite() {
                sample.abs()
            } else {
                0.0
            };
            if magnitude > current[channel] {
                current[channel] = magnitude;
            }
        }
        channel += 1;
        if channel < channels {
            continue;
        }
        channel = 0;
        total_frames += 1;
        frames_in_block += 1;
        if frames_in_block < block_frames {
            continue;
        }
        push_block(&mut blocks, &mut current);
        frames_in_block = 0;
        if blocks[0].len() >= peaks_per_channel * 2 {
            for channel_blocks in &mut blocks {
                merge_pairs(channel_blocks);
            }
            block_frames *= 2;
        }
    }
    if frames_in_block > 0 {
        push_block(&mut blocks, &mut current);
    }

    if total_frames == 0 {
        return Err(io::Error::other("audio_empty"));
    }

    Ok(AudioPeaks {
        duration: total_frames as f64 / sample_rate,
        channels: blocks
            .iter()
            .map(|channel_blocks| resample_max(channel_blocks, peaks_per_channel))
            .collect(),
    })
}

fn push_block(blocks: &mut [Vec<f32>], current: &mut [f32]) {
    for (channel_blocks, value) in blocks.iter_mut().zip(current.iter_mut()) {
        channel_blocks.push(*value);
        *value = 0.0;
    }
}

fn merge_pairs(values: &mut Vec<f32>) {
    let merged = values
        .chunks(2)
        .map(|pair| pair.iter().copied().fold(0.0, f32::max))
        .collect();
    *values = merged;
}

fn resample_max(values: &[f32], target_len: usize) -> Vec<f32> {
    if values.len() <= target_len {
        return values.iter().map(|value| round(*value)).collect();
    }
    let step = values.len() as f64 / target_len as f64;
    (0..target_len)
        .map(|index| {
            let start = (index as f64 * step).floor() as usize;
            let end = (((index + 1) as f64 * step).ceil() as usize).min(values.len());
            round(values[start..end].iter().copied().fold(0.0, f32::max))
        })
        .collect()
}

fn round(value: f32) -> f32 {
    (value * PRECISION).round() / PRECISION
}

fn source_stamp(audio_path: &Path) -> io::Result<SourceStamp> {
    let metadata = std::fs::metadata(audio_path)?;
    Ok(SourceStamp {
        filename: audio_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string(),
        size_bytes: metadata.len(),
        modified_ns: metadata
            .modified()?
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default(),
    })
}

fn read_cache(cache_path: &Path, source: &SourceStamp) -> Option<AudioPeaks> {
    let bytes = std::fs::read(cache_path).ok()?;
    let cached: CachedPeaks = serde_json::from_slice(&bytes).ok()?;
    (cached.source == *source).then_some(cached.peaks)
}

fn write_cache(cache_path: &Path, source: SourceStamp, peaks: &AudioPeaks) -> io::Result<()> {
    let Some(parent) = cache_path.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(parent)?;
    let bytes = serde_json::to_vec(&CachedPeaks {
        source,
        peaks: peaks.clone(),
    })?;
    let tmp_path = cache_path.with_extension(format!("json.{}.tmp", std::process::id()));
    std::fs::write(&tmp_path, bytes)?;
    std::fs::rename(&tmp_path, cache_path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp_path);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZero;

    fn buffer(channels: u16, sample_rate: u32, samples: Vec<f32>) -> rodio::buffer::SamplesBuffer {
        rodio::buffer::SamplesBuffer::new(
            NonZero::new(channels).unwrap(),
            NonZero::new(sample_rate).unwrap(),
            samples,
        )
    }

    #[test]
    fn keeps_per_channel_peaks_and_exact_duration() {
        let samples = (0..8)
            .flat_map(|frame| [frame as f32 / 10.0, -(frame as f32) / 20.0])
            .collect();
        let peaks = source_peaks(buffer(2, 4, samples), 4).unwrap();

        assert_eq!(peaks.duration, 2.0);
        assert_eq!(
            peaks.channels,
            vec![vec![0.1, 0.3, 0.5, 0.7], vec![0.05, 0.15, 0.25, 0.35],]
        );
    }

    #[test]
    fn long_sources_are_reduced_to_the_requested_length() {
        let samples: Vec<f32> = (0..10_001)
            .map(|index| if index == 7_777 { -0.9 } else { 0.1 })
            .collect();
        let peaks = source_peaks(buffer(1, 1_000, samples), 100).unwrap();

        assert_eq!(peaks.channels.len(), 1);
        assert_eq!(peaks.channels[0].len(), 100);
        assert!((peaks.duration - 10.001).abs() < 1e-9);
        assert_eq!(peaks.channels[0].iter().copied().fold(0.0, f32::max), 0.9);
        assert!(peaks.channels[0].iter().all(|value| *value >= 0.1));
    }

    #[test]
    fn extra_channels_are_ignored() {
        let peaks = source_peaks(buffer(3, 1, vec![0.1, 0.2, 0.9, 0.3, 0.4, 0.9]), 8).unwrap();
        assert_eq!(peaks.channels, vec![vec![0.1, 0.3], vec![0.2, 0.4]]);
    }

    #[test]
    fn empty_sources_are_rejected() {
        assert!(source_peaks(buffer(1, 16_000, Vec::new()), 8).is_err());
    }

    #[test]
    fn cache_is_reused_until_the_audio_changes() {
        let dir = tempfile::tempdir().unwrap();
        let audio_path = dir.path().join("audio.wav");
        let cache_path = dir.path().join("cache").join("peaks.json");
        std::fs::copy(anlg_data::english_1::AUDIO_PATH, &audio_path).unwrap();

        let first = cached_peaks(&audio_path, &cache_path).unwrap();
        assert!(cache_path.exists());
        assert_eq!(first.channels[0].len(), PEAKS_PER_CHANNEL);

        let stale = CachedPeaks {
            source: source_stamp(&audio_path).unwrap(),
            peaks: AudioPeaks {
                duration: 1.0,
                channels: vec![vec![0.5]],
            },
        };
        std::fs::write(&cache_path, serde_json::to_vec(&stale).unwrap()).unwrap();
        assert_eq!(cached_peaks(&audio_path, &cache_path).unwrap(), stale.peaks);

        std::fs::copy(anlg_data::english_1::AUDIO_MP3_PATH, &audio_path).unwrap();
        let recomputed = cached_peaks(&audio_path, &cache_path).unwrap();
        assert_ne!(recomputed, stale.peaks);
    }
}
