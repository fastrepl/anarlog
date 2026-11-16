//! The completion sounds shipping plays with `cuelume` (`shared/completion-sound.ts`).
//! cuelume synthesizes each recipe with Web Audio; this renders the same layers, envelopes,
//! shimmer echo and output limiter offline, then plays the buffer on the default output.

use std::f32::consts::TAU;

pub const COMPLETION_SOUNDS: [&str; 5] = ["ready", "success", "chime", "sparkle", "bloom"];
const COMPLETION_VOLUME: f32 = 0.7;
const RATE: u32 = 48_000;
const STOP_PADDING: f32 = 0.05;
const SILENT: f32 = 0.0001;

#[derive(Clone, Copy)]
enum Wave {
    Sine,
    Triangle,
}

#[derive(Clone, Copy)]
enum Filter {
    Lowpass,
    Bandpass,
}

#[derive(Clone, Copy)]
enum Kind {
    Tone {
        wave: Wave,
        frequency: f32,
        detune: f32,
        glide: Option<(f32, f32)>,
    },
    Noise {
        filter: Filter,
        frequency: f32,
        q: f32,
    },
}

#[derive(Clone, Copy)]
struct Layer {
    kind: Kind,
    offset: f32,
    attack: f32,
    decay: f32,
    peak: f32,
}

struct Shimmer {
    delay: f32,
    feedback: f32,
    wet: f32,
    lowpass: f32,
}

struct Recipe {
    master: f32,
    layers: Vec<Layer>,
    shimmer: Option<Shimmer>,
}

fn tone(frequency: f32, offset: f32, attack: f32, decay: f32, peak: f32) -> Layer {
    Layer {
        kind: Kind::Tone {
            wave: Wave::Sine,
            frequency,
            detune: 0.,
            glide: None,
        },
        offset,
        attack,
        decay,
        peak,
    }
}

fn shimmer(delay: f32, feedback: f32, wet: f32, lowpass: f32) -> Option<Shimmer> {
    Some(Shimmer {
        delay,
        feedback,
        wet,
        lowpass,
    })
}

/// cuelume 0.2.2 `RECIPES`, for the five completion sounds.
fn recipe(name: &str) -> Option<Recipe> {
    Some(match name {
        "chime" => Recipe {
            master: 0.5,
            layers: vec![
                tone(1046.5, 0., 0.006, 0.22, 0.09),
                tone(1568., 0.09, 0.006, 0.26, 0.08),
            ],
            shimmer: shimmer(0.12, 0.25, 0.18, 4000.),
        },
        "sparkle" => Recipe {
            master: 0.5,
            layers: vec![
                tone(1760., 0., 0.003, 0.09, 0.045),
                tone(2217., 0.045, 0.003, 0.09, 0.04),
                tone(2637., 0.09, 0.003, 0.1, 0.038),
                tone(3520., 0.135, 0.003, 0.12, 0.032),
            ],
            shimmer: shimmer(0.07, 0.35, 0.22, 6000.),
        },
        "bloom" => {
            let mut detuned = tone(528., 0., 0.06, 0.34, 0.05);
            if let Kind::Tone { detune, .. } = &mut detuned.kind {
                *detune = 12.;
            }
            Recipe {
                master: 0.5,
                layers: vec![tone(528., 0., 0.06, 0.32, 0.06), detuned],
                shimmer: shimmer(0.15, 0.2, 0.12, 2500.),
            }
        }
        "success" => Recipe {
            master: 0.5,
            layers: vec![
                tone(880., 0., 0.004, 0.09, 0.06),
                tone(1108.73, 0.06, 0.004, 0.1, 0.06),
                tone(1318.51, 0.12, 0.004, 0.18, 0.07),
            ],
            shimmer: shimmer(0.1, 0.22, 0.16, 4500.),
        },
        "ready" => Recipe {
            master: 0.48,
            layers: vec![
                Layer {
                    kind: Kind::Noise {
                        filter: Filter::Bandpass,
                        frequency: 3600.,
                        q: 1.8,
                    },
                    offset: 0.,
                    attack: 0.001,
                    decay: 0.02,
                    peak: 0.11,
                },
                Layer {
                    kind: Kind::Tone {
                        wave: Wave::Triangle,
                        frequency: 330.,
                        detune: 0.,
                        glide: Some((660., 0.12)),
                    },
                    offset: 0.012,
                    attack: 0.004,
                    decay: 0.16,
                    peak: 0.055,
                },
                tone(990., 0.13, 0.004, 0.22, 0.06),
            ],
            shimmer: shimmer(0.1, 0.16, 0.1, 4200.),
        },
        _ => return None,
    })
}

/// Web Audio's `exponentialRampToValueAtTime` from `from` to `to` over `duration`.
fn ramp(from: f32, to: f32, elapsed: f32, duration: f32) -> f32 {
    from * (to / from).powf((elapsed / duration).clamp(0., 1.))
}

fn envelope(layer: &Layer, t: f32) -> f32 {
    if t < layer.attack {
        ramp(SILENT, layer.peak, t, layer.attack)
    } else {
        ramp(layer.peak, SILENT, t - layer.attack, layer.decay)
    }
}

/// A Web Audio `BiquadFilterNode` (Audio EQ Cookbook; lowpass Q is in decibels).
struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    x: [f32; 2],
    y: [f32; 2],
}

impl Biquad {
    fn new(filter: Filter, frequency: f32, q: f32) -> Self {
        let w0 = TAU * frequency / RATE as f32;
        let (sin, cos) = w0.sin_cos();
        let (b, alpha) = match filter {
            Filter::Lowpass => {
                let alpha = sin / (2. * 10f32.powf(q / 20.));
                ([(1. - cos) / 2., 1. - cos, (1. - cos) / 2.], alpha)
            }
            Filter::Bandpass => {
                let alpha = sin / (2. * q);
                ([alpha, 0., -alpha], alpha)
            }
        };
        let a0 = 1. + alpha;
        Self {
            b: b.map(|value| value / a0),
            a: [-2. * cos / a0, (1. - alpha) / a0],
            x: [0.; 2],
            y: [0.; 2],
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let output = self.b[0] * input + self.b[1] * self.x[0] + self.b[2] * self.x[1]
            - self.a[0] * self.y[0]
            - self.a[1] * self.y[1];
        self.x = [input, self.x[0]];
        self.y = [output, self.y[0]];
        output
    }
}

fn duration(recipe: &Recipe) -> f32 {
    let sources = recipe
        .layers
        .iter()
        .map(|layer| layer.offset + layer.attack + layer.decay + STOP_PADDING)
        .fold(0., f32::max);
    let tail = recipe.shimmer.as_ref().map_or(0., |shimmer| {
        shimmer.delay * (1. + (0.001f32.ln() / shimmer.feedback.ln()).ceil())
    });
    sources + tail + STOP_PADDING
}

/// Renders `name` as mono samples at 48 kHz, or `None` for an unknown sound.
pub fn render(name: &str, volume: f32) -> Option<Vec<f32>> {
    let recipe = recipe(name)?;
    let length = (duration(&recipe) * RATE as f32).ceil() as usize;
    let mut master = vec![0f32; length];
    let mut seed = 0x9e37_79b9u32;
    for layer in &recipe.layers {
        let start = (layer.offset * RATE as f32) as usize;
        let end = (start + ((layer.attack + layer.decay + STOP_PADDING) * RATE as f32) as usize)
            .min(length);
        let mut phase = 0f32;
        let mut filter = match layer.kind {
            Kind::Noise {
                filter,
                frequency,
                q,
            } => Some(Biquad::new(filter, frequency, q)),
            Kind::Tone { .. } => None,
        };
        for (index, sample) in master[start..end].iter_mut().enumerate() {
            let t = index as f32 / RATE as f32;
            let value = match layer.kind {
                Kind::Tone {
                    wave,
                    frequency,
                    detune,
                    glide,
                } => {
                    let base = match glide {
                        Some((to, time)) => ramp(frequency, to, t, time),
                        None => frequency,
                    };
                    phase = (phase + base * 2f32.powf(detune / 1200.) / RATE as f32).fract();
                    match wave {
                        Wave::Sine => (TAU * phase).sin(),
                        Wave::Triangle => 1. - 4. * ((phase - 0.25).round() - (phase - 0.25)).abs(),
                    }
                }
                Kind::Noise { .. } => {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    let noise = seed as f32 / u32::MAX as f32 * 2. - 1.;
                    filter
                        .as_mut()
                        .map_or(noise, |filter| filter.process(noise))
                }
            };
            *sample += value * envelope(layer, t);
        }
    }
    let gain = recipe.master * volume;
    let mut mixed: Vec<f32> = master.iter().map(|sample| sample * gain).collect();
    if let Some(shimmer) = &recipe.shimmer {
        let delay = (shimmer.delay * RATE as f32) as usize;
        let mut line = vec![0f32; length];
        let mut lowpass = Biquad::new(Filter::Lowpass, shimmer.lowpass, 1.);
        for index in 0..length {
            let delayed = if index >= delay {
                line[index - delay]
            } else {
                0.
            };
            let filtered = lowpass.process(delayed);
            line[index] = master[index] * gain + filtered * shimmer.feedback;
            mixed[index] += filtered * shimmer.wet;
        }
    }
    Some(limit(mixed))
}

/// cuelume's shared output: gain 4 into a compressor (threshold -8 dB, ratio 12).
fn limit(samples: Vec<f32>) -> Vec<f32> {
    let threshold = 10f32.powf(-8. / 20.);
    let attack = (-1. / (0.002 * RATE as f32)).exp();
    let release = (-1. / (0.08 * RATE as f32)).exp();
    let mut level = 0f32;
    samples
        .into_iter()
        .map(|sample| {
            let sample = sample * 4.;
            let magnitude = sample.abs();
            let coefficient = if magnitude > level { attack } else { release };
            level = magnitude + coefficient * (level - magnitude);
            let gain = if level > threshold {
                threshold * (level / threshold).powf(1. / 12.) / level
            } else {
                1.
            };
            (sample * gain).clamp(-1., 1.)
        })
        .collect()
}

/// Plays a completion sound at shipping's preview volume, off the UI thread.
pub fn play_completion(name: &'static str) {
    std::thread::spawn(move || {
        let Some(samples) = render(name, COMPLETION_VOLUME) else {
            return;
        };
        let Ok(stream) = anlg_audio_utils::open_default_playback_sink() else {
            return;
        };
        let (Some(channels), Some(rate)) =
            (std::num::NonZero::new(1), std::num::NonZero::new(RATE))
        else {
            return;
        };
        let player = rodio::Player::connect_new(stream.mixer());
        player.append(rodio::buffer::SamplesBuffer::new(channels, rate, samples));
        player.sleep_until_end();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_sounds_render_bounded_audio() {
        for name in COMPLETION_SOUNDS {
            let samples = render(name, COMPLETION_VOLUME).unwrap();
            assert!(samples.len() > RATE as usize / 5, "{name} is too short");
            assert!(samples.iter().all(|sample| sample.abs() <= 1.));
            let peak = samples
                .iter()
                .fold(0f32, |peak, sample| peak.max(sample.abs()));
            assert!(peak > 0.05, "{name} is silent");
        }
        assert!(render("droplet", 1.).is_none());
    }
}
