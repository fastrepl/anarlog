//! The session audio player, ported from `audio-player/{provider,timeline,
//! timeline-shell}.tsx`: wavesurfer's bar rendering over the decoded
//! session audio, the play / pause button, and the `mm:ss / mm:ss` meta.

use std::path::Path;
use std::time::Duration;

use rodio::Source;

/// wavesurfer options in `AudioPlayerProvider`.
pub const BAR_WIDTH: f32 = 3.0;
pub const BAR_GAP: f32 = 2.0;
pub const BAR_RADIUS: f32 = 2.0;
pub const WAVE_HEIGHT: f32 = 24.0;
pub const CURSOR_WIDTH: f32 = 2.0;
pub const CURSOR_COLOR: u32 = 0x737373;
/// `splitChannels` with `overlay: true`: the first two channels keep their
/// own colours and draw over one another.
pub const CHANNEL_COLORS: [(u32, u32); 2] = [(0xe8d5d5, 0xc9a3a3), (0xd5dde8, 0xa3b3c9)];

/// Decoded audio, normalised the way wavesurfer's `normalize: true` does:
/// per channel samples in `-1..1` scaled so the loudest peak is 1.
#[derive(Clone, Debug, PartialEq)]
pub struct Waveform {
    pub channels: Vec<Vec<f32>>,
    pub duration: Duration,
}

impl Waveform {
    pub fn decode(path: &Path) -> anyhow::Result<Self> {
        let file = std::fs::File::open(path)?;
        let decoder = rodio::Decoder::try_from(file)?;
        let channel_count: u16 = decoder.channels().into();
        let sample_rate: u32 = decoder.sample_rate().into();
        let channel_count = channel_count.max(1) as usize;
        let mut channels: Vec<Vec<f32>> = vec![Vec::new(); channel_count];
        for (index, sample) in decoder.enumerate() {
            channels[index % channel_count].push(sample);
        }
        let frames = channels[0].len();
        let duration = Duration::from_secs_f64(frames as f64 / sample_rate.max(1) as f64);
        Ok(Self::from_samples(channels, duration))
    }

    pub fn from_samples(mut channels: Vec<Vec<f32>>, duration: Duration) -> Self {
        let max = channels
            .iter()
            .flatten()
            .fold(0.0f32, |max, sample| max.max(sample.abs()));
        if max > 0.0 {
            for channel in &mut channels {
                for sample in channel {
                    *sample /= max;
                }
            }
        }
        Self { channels, duration }
    }

    /// wavesurfer's bar peaks for a lane `width` pixels wide: one bar per
    /// `BAR_WIDTH + BAR_GAP`, each the peak magnitude of its sample range.
    pub fn bars(&self, channel: usize, width: f32) -> Vec<f32> {
        let Some(samples) = self.channels.get(channel) else {
            return Vec::new();
        };
        let count = bar_count(width);
        if count == 0 || samples.is_empty() {
            return Vec::new();
        }
        let per_bar = samples.len() as f32 / count as f32;
        (0..count)
            .map(|bar| {
                let start = (bar as f32 * per_bar) as usize;
                let end = (((bar + 1) as f32 * per_bar) as usize)
                    .max(start + 1)
                    .min(samples.len());
                samples[start..end]
                    .iter()
                    .fold(0.0f32, |max, sample| max.max(sample.abs()))
            })
            .collect()
    }
}

pub fn bar_count(width: f32) -> usize {
    ((width + BAR_GAP) / (BAR_WIDTH + BAR_GAP)).floor().max(0.0) as usize
}

/// `formatTime(seconds)`: `mm:ss`, floored.
pub fn format_time(seconds: f64) -> String {
    let total = seconds.max(0.0).floor() as u64;
    format!("{:02}:{:02}", total / 60, total % 60)
}

/// `AudioPlayerState`
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayerState {
    Playing,
    Paused,
    Stopped,
}

/// The WebAudio element behind wavesurfer: rodio's default output device
/// with one queued decode of the file. Without an output device the player
/// stays stopped, like a media element that fails to play.
pub struct Playback {
    _sink: rodio::MixerDeviceSink,
    player: rodio::Player,
    duration: Duration,
}

impl Playback {
    pub fn start(path: &Path, duration: Duration, rate: f32) -> anyhow::Result<Self> {
        let sink = rodio::DeviceSinkBuilder::open_default_sink()?;
        let player = rodio::Player::connect_new(sink.mixer());
        let file = std::fs::File::open(path)?;
        let decoder = rodio::Decoder::try_from(file)?;
        player.append(decoder);
        player.set_speed(rate);
        player.play();
        Ok(Self {
            _sink: sink,
            player,
            duration,
        })
    }

    pub fn position(&self) -> Duration {
        self.player.get_pos().min(self.duration)
    }

    pub fn finished(&self) -> bool {
        self.player.empty()
    }

    pub fn pause(&self) {
        self.player.pause();
    }

    pub fn resume(&self) {
        self.player.play();
    }

    pub fn seek(&self, position: Duration) {
        let _ = self.player.try_seek(position.min(self.duration));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_is_floored_minutes_and_seconds() {
        assert_eq!(format_time(0.0), "00:00");
        assert_eq!(format_time(2.9), "00:02");
        assert_eq!(format_time(61.0), "01:01");
        assert_eq!(format_time(3600.0), "60:00");
        assert_eq!(format_time(-3.0), "00:00");
    }

    #[test]
    fn normalisation_scales_the_loudest_peak_to_one() {
        let waveform = Waveform::from_samples(
            vec![vec![0.1, -0.5, 0.25], vec![0.0, 0.0, 0.0]],
            Duration::from_secs(1),
        );
        assert_eq!(waveform.channels[0], vec![0.2, -1.0, 0.5]);
        assert_eq!(waveform.channels[1], vec![0.0, 0.0, 0.0]);
        // Silence stays silent rather than dividing by zero.
        let silent = Waveform::from_samples(vec![vec![0.0, 0.0]], Duration::from_secs(1));
        assert_eq!(silent.channels[0], vec![0.0, 0.0]);
    }

    #[test]
    fn bars_take_the_peak_of_each_pixel_range() {
        // 5px pitch: 24px fits (24 + 2) / 5 = 5 bars.
        assert_eq!(bar_count(24.0), 5);
        assert_eq!(bar_count(0.0), 0);
        let waveform = Waveform::from_samples(
            vec![vec![0.0, 0.5, -1.0, 0.0, 0.25, 0.25, 0.0, 0.0, 0.75, 0.1]],
            Duration::from_secs(1),
        );
        assert_eq!(waveform.bars(0, 24.0), vec![0.5, 1.0, 0.25, 0.0, 0.75]);
        assert!(waveform.bars(1, 24.0).is_empty());
    }

    #[test]
    fn decodes_a_wav_into_normalised_channels() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tone.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 8000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&path, spec).unwrap();
        for i in 0..8000 {
            let left = ((i as f32 / 8000.0 * 440.0 * std::f32::consts::TAU).sin()
                * 0.5
                * i16::MAX as f32) as i16;
            writer.write_sample(left).unwrap();
            writer.write_sample(0i16).unwrap();
        }
        writer.finalize().unwrap();
        let waveform = Waveform::decode(&path).unwrap();
        assert_eq!(waveform.channels.len(), 2);
        assert_eq!(waveform.channels[0].len(), 8000);
        assert!((waveform.duration.as_secs_f64() - 1.0).abs() < 0.01);
        let peak = waveform.channels[0]
            .iter()
            .fold(0.0f32, |m, s| m.max(s.abs()));
        assert!((peak - 1.0).abs() < 1e-3);
        assert!(waveform.channels[1].iter().all(|s| *s == 0.0));
    }
}
