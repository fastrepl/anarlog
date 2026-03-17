use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::theme::Theme;

pub struct Waveform<'a> {
    mic: &'a [u64],
    speaker: &'a [u64],
    muted: bool,
    theme: &'a Theme,
}

impl<'a> Waveform<'a> {
    pub fn new(mic: &'a [u64], speaker: &'a [u64], theme: &'a Theme) -> Self {
        Self {
            mic,
            speaker,
            muted: false,
            theme,
        }
    }

    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }
}

impl Widget for Waveform<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let width = area.width as usize;
        if width == 0 || area.height == 0 {
            return;
        }

        let sample_count = self.mic.len().max(self.speaker.len());
        if sample_count == 0 {
            return;
        }

        let mut combined = Vec::with_capacity(sample_count);
        for i in 0..sample_count {
            let mic_value = aligned_sample(self.mic, sample_count, i);
            let speaker_value = aligned_sample(self.speaker, sample_count, i);
            combined.push(mic_value.max(speaker_value).min(1000));
        }

        for x in 0..width {
            let raw_value = column_energy(&combined, x, width) as f64;
            let envelope = edge_envelope(x, width);
            let value = (raw_value * envelope).round() as u64;

            let normalized = (value as f64 / 1000.0).clamp(0.0, 1.0);
            let level = if value == 0 {
                0
            } else {
                (normalized.powf(0.55) * 8.0).round().clamp(1.0, 8.0) as u8
            };

            let style = if self.muted || level == 0 || envelope < 0.22 {
                self.theme.waveform_silent
            } else if level >= 6 && envelope > 0.7 {
                self.theme.waveform_hot
            } else {
                self.theme.waveform_normal
            };

            let ch = level_char(level);
            buf[(area.x + x as u16, area.y)]
                .set_char(ch)
                .set_style(style);
        }
    }
}

const ENVELOPE_FLAT_RADIUS: f64 = 0.62;

fn edge_envelope(x: usize, width: usize) -> f64 {
    if width <= 1 {
        return 1.0;
    }

    let center = (width - 1) as f64 / 2.0;
    let distance = ((x as f64) - center).abs() / center.max(1.0);
    if distance <= ENVELOPE_FLAT_RADIUS {
        return 1.0;
    }

    let t = ((distance - ENVELOPE_FLAT_RADIUS) / (1.0 - ENVELOPE_FLAT_RADIUS)).clamp(0.0, 1.0);
    let smooth = t * t * (3.0 - 2.0 * t);
    1.0 - smooth
}

fn column_energy(values: &[u64], x: usize, width: usize) -> u64 {
    if values.is_empty() || width == 0 {
        return 0;
    }

    let sample_count = values.len();
    let start = x * sample_count / width;
    let mut end = (x + 1) * sample_count / width;
    if end <= start {
        end = (start + 1).min(sample_count);
    }

    let mut max_value = 0u64;
    let mut sum = 0u64;
    let mut count = 0u64;

    for value in &values[start..end] {
        max_value = max_value.max(*value);
        sum += *value;
        count += 1;
    }

    let avg = if count == 0 { 0 } else { sum / count };
    let raw = (max_value * 7 + avg * 3) / 10;

    let left = if start > 0 {
        values[start - 1]
    } else {
        values[start]
    };
    let right = if end < sample_count {
        values[end]
    } else {
        values[sample_count - 1]
    };

    ((raw * 6) + left + right) / 8
}

fn aligned_sample(values: &[u64], sample_count: usize, index: usize) -> u64 {
    if values.is_empty() {
        return 0;
    }

    let offset = sample_count.saturating_sub(values.len());
    if index < offset {
        0
    } else {
        values[index - offset]
    }
}

fn level_char(level: u8) -> char {
    match level {
        0 => ' ',
        1 => '\u{2581}',
        2 => '\u{2582}',
        3 => '\u{2583}',
        4 => '\u{2584}',
        5 => '\u{2585}',
        6 => '\u{2586}',
        7 => '\u{2587}',
        _ => '\u{2588}',
    }
}
