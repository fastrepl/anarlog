//! The `cuelume` sound palette the frontend plays for completions, rendered
//! offline: each recipe is a few sine / triangle tones and filtered noise
//! bursts with exponential envelopes, an optional feedback "shimmer" delay,
//! and the shared output stage (`OUTPUT_GAIN` into a limiter), as in
//! `cuelume/dist/audio/engine.js`. No audio files, like the original.

pub const SAMPLE_RATE: u32 = 48_000;
const SOURCE_STOP_PADDING: f32 = 0.05;
const CLEANUP_MARGIN: f32 = 0.05;
const INAUDIBLE_GAIN: f32 = 0.001;
const OUTPUT_GAIN: f32 = 4.0;
/// `COMPLETION_SOUND_VOLUME` in `shared/completion-sound.ts`.
pub const COMPLETION_SOUND_VOLUME: f32 = 0.7;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Waveform {
    Sine,
    Triangle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Filter {
    Lowpass,
    Bandpass,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Layer {
    Tone {
        waveform: Waveform,
        frequency: f32,
        detune: f32,
        glide_to: Option<f32>,
        glide_time: Option<f32>,
        offset: f32,
        attack: f32,
        decay: f32,
        peak: f32,
    },
    Noise {
        filter: Filter,
        filter_frequency: f32,
        filter_q: f32,
        offset: f32,
        attack: f32,
        decay: f32,
        peak: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shimmer {
    pub delay: f32,
    pub feedback: f32,
    pub wet: f32,
    pub lowpass: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Recipe {
    pub master_gain: f32,
    pub layers: Vec<Layer>,
    pub shimmer: Option<Shimmer>,
}

fn tone(
    waveform: Waveform,
    frequency: f32,
    offset: f32,
    attack: f32,
    decay: f32,
    peak: f32,
) -> Layer {
    Layer::Tone {
        waveform,
        frequency,
        detune: 0.0,
        glide_to: None,
        glide_time: None,
        offset,
        attack,
        decay,
        peak,
    }
}

fn noise(
    filter: Filter,
    filter_frequency: f32,
    filter_q: f32,
    offset: f32,
    attack: f32,
    decay: f32,
    peak: f32,
) -> Layer {
    Layer::Noise {
        filter,
        filter_frequency,
        filter_q,
        offset,
        attack,
        decay,
        peak,
    }
}

/// `COMPLETION_SOUND_NAMES` of `RECIPES`.
pub fn recipe(name: &str) -> Option<Recipe> {
    use Filter::*;
    use Waveform::*;
    Some(match name {
        "chime" => Recipe {
            master_gain: 0.5,
            layers: vec![
                tone(Sine, 1046.5, 0.0, 0.006, 0.22, 0.09),
                tone(Sine, 1568.0, 0.09, 0.006, 0.26, 0.08),
            ],
            shimmer: Some(Shimmer {
                delay: 0.12,
                feedback: 0.25,
                wet: 0.18,
                lowpass: 4000.0,
            }),
        },
        "sparkle" => Recipe {
            master_gain: 0.5,
            layers: vec![
                tone(Sine, 1760.0, 0.0, 0.003, 0.09, 0.045),
                tone(Sine, 2217.0, 0.045, 0.003, 0.09, 0.04),
                tone(Sine, 2637.0, 0.09, 0.003, 0.1, 0.038),
                tone(Sine, 3520.0, 0.135, 0.003, 0.12, 0.032),
            ],
            shimmer: Some(Shimmer {
                delay: 0.07,
                feedback: 0.35,
                wet: 0.22,
                lowpass: 6000.0,
            }),
        },
        "bloom" => Recipe {
            master_gain: 0.5,
            layers: vec![
                tone(Sine, 528.0, 0.0, 0.06, 0.32, 0.06),
                Layer::Tone {
                    waveform: Sine,
                    frequency: 528.0,
                    detune: 12.0,
                    glide_to: None,
                    glide_time: None,
                    offset: 0.0,
                    attack: 0.06,
                    decay: 0.34,
                    peak: 0.05,
                },
            ],
            shimmer: Some(Shimmer {
                delay: 0.15,
                feedback: 0.2,
                wet: 0.12,
                lowpass: 2500.0,
            }),
        },
        "success" => Recipe {
            master_gain: 0.5,
            layers: vec![
                tone(Sine, 880.0, 0.0, 0.004, 0.09, 0.06),
                tone(Sine, 1108.73, 0.06, 0.004, 0.1, 0.06),
                tone(Sine, 1318.51, 0.12, 0.004, 0.18, 0.07),
            ],
            shimmer: Some(Shimmer {
                delay: 0.1,
                feedback: 0.22,
                wet: 0.16,
                lowpass: 4500.0,
            }),
        },
        "ready" => Recipe {
            master_gain: 0.48,
            layers: vec![
                noise(Bandpass, 3600.0, 1.8, 0.0, 0.001, 0.02, 0.11),
                Layer::Tone {
                    waveform: Triangle,
                    frequency: 330.0,
                    detune: 0.0,
                    glide_to: Some(660.0),
                    glide_time: Some(0.12),
                    offset: 0.012,
                    attack: 0.004,
                    decay: 0.16,
                    peak: 0.055,
                },
                tone(Sine, 990.0, 0.13, 0.004, 0.22, 0.06),
            ],
            shimmer: Some(Shimmer {
                delay: 0.1,
                feedback: 0.16,
                wet: 0.1,
                lowpass: 4200.0,
            }),
        },
        _ => return None,
    })
}

/// `normalizeCompletionSoundName`: unknown names play `ready`.
pub fn normalize_completion_sound_name(value: Option<&str>) -> &'static str {
    match value {
        Some("success") => "success",
        Some("chime") => "chime",
        Some("sparkle") => "sparkle",
        Some("bloom") => "bloom",
        _ => "ready",
    }
}

/// `sourceEnd`
fn source_end(recipe: &Recipe) -> f32 {
    recipe
        .layers
        .iter()
        .map(|layer| match layer {
            Layer::Tone {
                offset,
                attack,
                decay,
                ..
            }
            | Layer::Noise {
                offset,
                attack,
                decay,
                ..
            } => offset + attack + decay + SOURCE_STOP_PADDING,
        })
        .fold(0.0, f32::max)
}

/// `shimmerTail`
fn shimmer_tail(shimmer: Option<&Shimmer>) -> f32 {
    let Some(shimmer) = shimmer else {
        return 0.0;
    };
    if shimmer.feedback <= 0.0 {
        return 0.0;
    }
    if shimmer.feedback >= 1.0 {
        return shimmer.delay;
    }
    shimmer.delay * (1.0 + (INAUDIBLE_GAIN.ln() / shimmer.feedback.ln()).ceil())
}

/// `exponentialRampToValueAtTime` between two automation points.
fn exp_ramp(from: f32, to: f32, t0: f32, t1: f32, t: f32) -> f32 {
    if t <= t0 || t1 <= t0 {
        return from;
    }
    if t >= t1 {
        return to;
    }
    from * (to / from).powf((t - t0) / (t1 - t0))
}

/// The layer's gain envelope at `t` seconds after its start.
fn envelope(t: f32, attack: f32, decay: f32, peak: f32) -> f32 {
    if t < 0.0 {
        return 0.0;
    }
    if t < attack {
        exp_ramp(0.0001, peak, 0.0, attack, t)
    } else if t < attack + decay {
        exp_ramp(peak, 0.0001, attack, attack + decay, t)
    } else {
        0.0
    }
}

/// RBJ biquad, as WebAudio's `BiquadFilterNode` (lowpass `Q` is in dB).
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    fn new(filter: Filter, frequency: f32, q: f32, sample_rate: f32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * (frequency / sample_rate).clamp(0.0, 0.5);
        let (sin, cos) = w0.sin_cos();
        let (b0, b1, b2, a0, a1, a2) = match filter {
            Filter::Lowpass => {
                let q = 10f32.powf(q / 20.0);
                let alpha = sin / (2.0 * q);
                (
                    (1.0 - cos) / 2.0,
                    1.0 - cos,
                    (1.0 - cos) / 2.0,
                    1.0 + alpha,
                    -2.0 * cos,
                    1.0 - alpha,
                )
            }
            Filter::Bandpass => {
                let alpha = sin / (2.0 * q.max(0.0001));
                (alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * cos, 1.0 - alpha)
            }
        };
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// A small deterministic noise source (xorshift), in place of `Math.random`.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// `DynamicsCompressorNode` (threshold −8 dB, knee 6, ratio 12, attack 2ms,
/// release 80ms), after Blink's `DynamicsCompressorKernel`: the soft-knee
/// static curve (`kneeCurve` / `saturate` with `k` solved by `kAtSlope`), the
/// perceptual full-range makeup gain (`pow(1 / saturate(1), 0.6)`), the 6ms
/// look-ahead pre-delay, and a one-pole gain envelope for attack / release.
struct Limiter {
    linear_threshold: f32,
    knee_threshold: f32,
    knee_threshold_db: f32,
    y_knee_threshold_db: f32,
    slope: f32,
    k: f32,
    makeup: f32,
    gain: f32,
    attack: f32,
    release: f32,
    pre_delay: Vec<f32>,
    pre_index: usize,
}

fn db_to_linear(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

fn linear_to_db(x: f32) -> f32 {
    20.0 * x.max(1e-9).log10()
}

impl Limiter {
    const THRESHOLD_DB: f32 = -8.0;
    const KNEE_DB: f32 = 6.0;
    const RATIO: f32 = 12.0;

    fn knee_curve(&self, x: f32, k: f32) -> f32 {
        if x < self.linear_threshold {
            return x;
        }
        self.linear_threshold + (1.0 - (-k * (x - self.linear_threshold)).exp()) / k
    }

    fn slope_at(&self, x: f32, k: f32) -> f32 {
        if x < self.linear_threshold {
            return 1.0;
        }
        let x2 = x * 1.001;
        let (x_db, x2_db) = (linear_to_db(x), linear_to_db(x2));
        let (y_db, y2_db) = (
            linear_to_db(self.knee_curve(x, k)),
            linear_to_db(self.knee_curve(x2, k)),
        );
        (y2_db - y_db) / (x2_db - x_db)
    }

    fn k_at_slope(&self, desired: f32) -> f32 {
        let x = db_to_linear(Self::THRESHOLD_DB + Self::KNEE_DB);
        let (mut min_k, mut max_k, mut k) = (0.1f32, 10_000.0f32, 5.0f32);
        for _ in 0..15 {
            if self.slope_at(x, k) < desired {
                max_k = k;
            } else {
                min_k = k;
            }
            k = (min_k * max_k).sqrt();
        }
        k
    }

    fn saturate(&self, x: f32) -> f32 {
        if x < self.knee_threshold {
            self.knee_curve(x, self.k)
        } else {
            let y_db =
                self.y_knee_threshold_db + self.slope * (linear_to_db(x) - self.knee_threshold_db);
            db_to_linear(y_db)
        }
    }

    fn new(sample_rate: f32) -> Self {
        let mut limiter = Self {
            linear_threshold: db_to_linear(Self::THRESHOLD_DB),
            knee_threshold: db_to_linear(Self::THRESHOLD_DB + Self::KNEE_DB),
            knee_threshold_db: Self::THRESHOLD_DB + Self::KNEE_DB,
            y_knee_threshold_db: 0.0,
            slope: 1.0 / Self::RATIO,
            k: 5.0,
            makeup: 1.0,
            gain: 1.0,
            attack: (-1.0 / (0.002 * sample_rate)).exp(),
            release: (-1.0 / (0.08 * sample_rate)).exp(),
            pre_delay: vec![0.0; ((0.006 * sample_rate) as usize).max(1)],
            pre_index: 0,
        };
        limiter.k = limiter.k_at_slope(limiter.slope);
        limiter.y_knee_threshold_db =
            linear_to_db(limiter.knee_curve(limiter.knee_threshold, limiter.k));
        let full_range_gain = limiter.saturate(1.0);
        limiter.makeup = (1.0 / full_range_gain).powf(0.6);
        limiter
    }

    fn process(&mut self, x: f32) -> f32 {
        let level = x.abs();
        let desired = if level <= 0.0001 {
            1.0
        } else {
            self.saturate(level) / level
        };
        let coefficient = if desired < self.gain {
            self.attack
        } else {
            self.release
        };
        self.gain = coefficient * self.gain + (1.0 - coefficient) * desired;
        let delayed = self.pre_delay[self.pre_index];
        self.pre_delay[self.pre_index] = x;
        self.pre_index = (self.pre_index + 1) % self.pre_delay.len();
        delayed * self.gain * self.makeup
    }
}

/// `renderRecipe`: mono PCM at `SAMPLE_RATE`, already through the output
/// stage, for the given volume multiplier.
pub fn render(recipe: &Recipe, volume: f32) -> Vec<f32> {
    let sample_rate = SAMPLE_RATE as f32;
    let duration = source_end(recipe) + shimmer_tail(recipe.shimmer.as_ref()) + CLEANUP_MARGIN;
    let length = (duration * sample_rate).ceil().max(1.0) as usize;
    let mut dry = vec![0.0f32; length];

    let mut noise = Noise(0x9e37_79b9);
    for layer in &recipe.layers {
        match *layer {
            Layer::Tone {
                waveform,
                frequency,
                detune,
                glide_to,
                glide_time,
                offset,
                attack,
                decay,
                peak,
            } => {
                let start = (offset * sample_rate) as usize;
                let stop = ((offset + attack + decay + SOURCE_STOP_PADDING) * sample_rate) as usize;
                let base = frequency * 2f32.powf(detune / 1200.0);
                let glide_time = glide_time.unwrap_or(attack + decay);
                let mut phase = 0.0f32;
                for (i, sample) in dry
                    .iter_mut()
                    .enumerate()
                    .take(stop.min(length))
                    .skip(start)
                {
                    let t = (i - start) as f32 / sample_rate;
                    let hz = match glide_to {
                        Some(target) => exp_ramp(
                            base,
                            target * 2f32.powf(detune / 1200.0),
                            0.0,
                            glide_time,
                            t,
                        ),
                        None => base,
                    };
                    let value = match waveform {
                        Waveform::Sine => (phase * std::f32::consts::TAU).sin(),
                        // A triangle from the phase, peaking at ±1.
                        Waveform::Triangle => {
                            let saw = (phase + 0.25).rem_euclid(1.0);
                            4.0 * (saw - 0.5).abs() - 1.0
                        }
                    };
                    *sample += value * envelope(t, attack, decay, peak);
                    phase = (phase + hz / sample_rate).rem_euclid(1.0);
                }
            }
            Layer::Noise {
                filter,
                filter_frequency,
                filter_q,
                offset,
                attack,
                decay,
                peak,
            } => {
                let start = (offset * sample_rate) as usize;
                let stop = ((offset + attack + decay + SOURCE_STOP_PADDING) * sample_rate) as usize;
                let mut biquad = Biquad::new(filter, filter_frequency, filter_q, sample_rate);
                for (i, sample) in dry
                    .iter_mut()
                    .enumerate()
                    .take(stop.min(length))
                    .skip(start)
                {
                    let t = (i - start) as f32 / sample_rate;
                    let filtered = biquad.process(noise.next());
                    *sample += filtered * envelope(t, attack, decay, peak);
                }
            }
        }
    }

    // `master` carries `masterGain * volume`; the shimmer is a feedback delay
    // with a lowpass in the loop, mixed back at `wet`.
    let master_gain = recipe.master_gain * volume;
    let mut mixed = vec![0.0f32; length];
    if let Some(shimmer) = &recipe.shimmer {
        let delay_samples = ((shimmer.delay * sample_rate) as usize).max(1);
        let mut line = vec![0.0f32; length + delay_samples];
        let mut lowpass = Biquad::new(Filter::Lowpass, shimmer.lowpass, 1.0, sample_rate);
        for i in 0..length {
            let dry_sample = dry[i] * master_gain;
            // `delay` feeds `feedbackFilter`, which returns into the delay at
            // `feedback` and reaches the output at `wet`.
            let filtered = lowpass.process(line[i]);
            line[i + delay_samples] += dry_sample + filtered * shimmer.feedback;
            mixed[i] = dry_sample + filtered * shimmer.wet;
        }
    } else {
        for i in 0..length {
            mixed[i] = dry[i] * master_gain;
        }
    }

    let mut limiter = Limiter::new(sample_rate);
    mixed
        .into_iter()
        .map(|sample| limiter.process(sample * OUTPUT_GAIN).clamp(-1.0, 1.0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_completion_sound_renders_audible_samples_that_decay() {
        for name in ["ready", "success", "chime", "sparkle", "bloom"] {
            let recipe = recipe(name).unwrap();
            let samples = render(&recipe, COMPLETION_SOUND_VOLUME);
            assert!(
                samples.len() > SAMPLE_RATE as usize / 10,
                "{name} is too short"
            );
            let peak = samples.iter().fold(0.0f32, |acc, s| acc.max(s.abs()));
            assert!(peak > 0.05 && peak <= 1.0, "{name} peak {peak}");
            let tail = &samples[samples.len() - 200..];
            let tail_peak = tail.iter().fold(0.0f32, |acc, s| acc.max(s.abs()));
            assert!(tail_peak < 0.02, "{name} tail {tail_peak}");
        }
        assert!(recipe("nope").is_none());
    }

    #[test]
    fn sound_names_normalise_like_the_frontend() {
        assert_eq!(normalize_completion_sound_name(None), "ready");
        assert_eq!(normalize_completion_sound_name(Some("x")), "ready");
        assert_eq!(normalize_completion_sound_name(Some("bloom")), "bloom");
    }

    #[test]
    fn tails_and_envelopes_follow_the_engine() {
        assert_eq!(shimmer_tail(None), 0.0);
        let shimmer = Shimmer {
            delay: 0.1,
            feedback: 0.16,
            wet: 0.1,
            lowpass: 4200.0,
        };
        // 0.1 * (1 + ceil(ln(0.001) / ln(0.16))) = 0.1 * (1 + 4)
        assert!((shimmer_tail(Some(&shimmer)) - 0.5).abs() < 1e-5);
        assert!((envelope(0.004, 0.004, 0.1, 0.06) - 0.06).abs() < 1e-6);
        assert_eq!(envelope(0.2, 0.004, 0.1, 0.06), 0.0);
    }
}
