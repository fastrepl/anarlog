//! Anarlog's echo canceller for a raw native microphone, with the call track as its reference.
//!
//! Apple's voice processing cancels echo but ducks every other app's audio, the call included,
//! and macOS has no public setting that turns the ducking off. A raw microphone is not ducked;
//! this runs the existing engine's canceller (AEC model, reference alignment, linear residual
//! pass) on it. The helper placed both tracks on one host clock, so the reference only ever
//! leads the echo: it is held back only on a late speaker route, and the microphone is never
//! delayed to meet it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anlg_aec::AEC;

use crate::capture::{AecReferenceAligner, build_aec, process_aec};

pub(crate) struct EchoCanceller {
    sample_rate: u32,
    /// The listener's choice: off while every output is headphones.
    wanted: Arc<AtomicBool>,
    state: Option<State>,
}

struct State {
    aec: Option<AEC>,
    aligner: AecReferenceAligner,
    linear_echo_gain: Option<f32>,
}

impl EchoCanceller {
    pub(crate) fn new(sample_rate: u32, wanted: Arc<AtomicBool>) -> Self {
        Self {
            sample_rate,
            wanted,
            state: None,
        }
    }

    /// A rebuilt microphone or call tap is a new echo path.
    pub(crate) fn reset(&mut self) {
        if let Some(state) = &mut self.state {
            if let Some(aec) = &mut state.aec {
                aec.reset();
            }
            state.aligner = AecReferenceAligner::placed(self.sample_rate);
            state.linear_echo_gain = None;
        }
    }

    /// The microphone without the call's echo. It trails the raw microphone only by the model's
    /// block latency (24 ms at 16 kHz), as on the existing engine. None while cancellation is off
    /// or failed: the listener then keeps the raw microphone.
    pub(crate) fn cancel(&mut self, mic: &[f32], speaker: &[f32]) -> Option<Arc<[f32]>> {
        if !self.wanted.load(Ordering::Acquire) {
            self.state = None;
            return None;
        }
        let sample_rate = self.sample_rate;
        let state = self.state.get_or_insert_with(|| State {
            aec: build_aec(),
            aligner: AecReferenceAligner::placed(sample_rate),
            linear_echo_gain: None,
        });
        let aligned = state.aligner.align(speaker, mic);
        if aligned.alignment_changed {
            if let Some(aec) = &mut state.aec {
                aec.reset();
            }
            state.linear_echo_gain = None;
        }
        process_aec(
            &mut state.aec,
            &mut state.linear_echo_gain,
            &aligned.mic,
            &aligned.speaker,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use anlg_audio::CaptureEvent;
    use anlg_audio::meeting::CaptureIdentity;

    use super::EchoCanceller;
    use crate::native_meeting::convert::Converter;
    use crate::native_meeting::dispatch::{Dispatcher, Output};
    use crate::native_meeting::wire::{Message, Track, encode_event, read_message};

    const IN_RATE: usize = 48_000;
    const OUT_RATE: usize = 16_000;
    /// The AEC model's block latency: its output trails the raw microphone, as on the existing
    /// engine. Nothing else may move the microphone off its clock.
    const AEC_LATENCY: usize = anlg_aec::BLOCK_SIZE - anlg_aec::BLOCK_SHIFT;

    /// Speech at the helper's rate. 16 kHz sources are interpolated; the converter's low-pass
    /// removes the images, and every reference below takes the same path.
    fn speech(path: &str, offset_seconds: usize, seconds: usize, rms_db: f32) -> Vec<f32> {
        let mut reader = hound::WavReader::open(path).unwrap();
        let rate = reader.spec().sample_rate as usize;
        let factor = IN_RATE / rate;
        let samples: Vec<f32> = reader
            .samples::<i16>()
            .skip(offset_seconds * rate)
            .take(seconds * rate + 1)
            .map(|s| s.unwrap() as f32 / 32_768.0)
            .collect();
        let upsampled: Vec<f32> = (0..seconds * IN_RATE)
            .map(|n| {
                let (i, k) = (n / factor, (n % factor) as f32 / factor as f32);
                samples[i] + (samples[i + 1] - samples[i]) * k
            })
            .collect();
        scaled(&upsampled, rms_db)
    }

    fn scaled(signal: &[f32], rms_db: f32) -> Vec<f32> {
        let gain = 10f32.powf(rms_db / 20.0) / power(signal).sqrt();
        signal.iter().map(|s| s * gain).collect()
    }

    fn silence(signal: &mut [f32], from_seconds: f32, to_seconds: f32) {
        let at = |seconds: f32| (seconds * IN_RATE as f32) as usize;
        signal[at(from_seconds)..at(to_seconds)].fill(0.0);
    }

    /// A small speaker in a room, `delay_ms` after the call track and 12.8 dB lower: what the
    /// helper's `plain` mode measured on a MacBook Pro's built-in speakers at 48 ms. No bass,
    /// mild distortion, and early reflections, so a delay line alone cannot cancel it.
    fn room_echo(far: &[f32], delay_ms: usize) -> Vec<f32> {
        let alpha = 1.0 / (1.0 + 2.0 * std::f32::consts::PI * 250.0 / IN_RATE as f32);
        let (mut previous_in, mut previous_out) = (0.0, 0.0);
        let speaker: Vec<f32> = far
            .iter()
            .map(|&x| {
                previous_out = alpha * (previous_out + x - previous_in);
                previous_in = x;
                (1.5 * previous_out).tanh() / 1.5
            })
            .collect();
        let reflections = [
            (0.0, 1.0),
            (3.1, 0.55),
            (7.3, -0.4),
            (11.9, 0.3),
            (17.2, -0.25),
            (23.5, 0.2),
            (31.0, 0.15),
            (40.7, -0.12),
            (52.9, 0.09),
            (68.0, -0.07),
            (86.4, 0.05),
            (110.0, 0.035),
            (140.0, -0.025),
        ];
        let mut echo = vec![0.0; far.len()];
        for (ms, gain) in reflections {
            let delay = ((delay_ms as f32 + ms) * IN_RATE as f32 / 1_000.0) as usize;
            for n in delay..far.len() {
                echo[n] += gain * speaker[n - delay];
            }
        }
        let gain = (power(far) / power(&echo)).sqrt() * 10f32.powf(-12.8 / 20.0);
        echo.iter().map(|s| s * gain).collect()
    }

    fn noise(len: usize, rms_db: f32) -> Vec<f32> {
        let mut state = 0x2545_f491_u32;
        let noise: Vec<f32> = (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as f32 / u32::MAX as f32 * 2.0 - 1.0
            })
            .collect();
        scaled(&noise, rms_db)
    }

    fn event(name: &str, fields: &[(&str, &str)]) -> Message {
        read_message(&mut encode_event(name, fields).as_slice())
            .unwrap()
            .unwrap()
    }

    #[derive(Default)]
    struct Run {
        raw: Vec<f32>,
        mic: Vec<f32>,
        frames: usize,
        cancelled: usize,
        voice_processed: usize,
    }

    /// The helper's wire messages for one capture, through the adapter as Anarlog runs it.
    fn capture(mic: &[f32], remote: &[f32], processing: &str, wanted: Arc<AtomicBool>) -> Run {
        let mut dispatcher = Dispatcher::new(
            CaptureIdentity {
                account_id: "account".into(),
                session_id: "session".into(),
                capture_id: "capture".into(),
            },
            Converter::new(OUT_RATE as u32, 1_920).unwrap(),
            EchoCanceller::new(OUT_RATE as u32, wanted),
        );
        let voice = (processing == "voice_processed").to_string();
        let mut messages = vec![
            event("microphone-configured", &[("voiceProcessing", &voice)]),
            event(
                "started",
                &[("scope", "system"), ("micProcessing", processing)],
            ),
            event("call-audio-configured", &[("scope", "system")]),
        ];
        for start in (0..mic.len()).step_by(480) {
            let end = (start + 480).min(mic.len());
            for (track, samples) in [(Track::Microphone, mic), (Track::Remote, remote)] {
                messages.push(Message::Pcm {
                    track,
                    frame: start as u64,
                    samples: samples[start..end].to_vec(),
                });
            }
            messages.push(event("tick", &[("frame", &end.to_string())]));
        }
        let frames = mic.len().to_string();
        messages.push(event(
            "ended",
            &[
                ("reason", "user_stopped"),
                ("frames", &frames),
                ("frame", &frames),
            ],
        ));
        let mut run = Run::default();
        for message in messages {
            for output in dispatcher.handle(message) {
                let Output::Stream(Ok(CaptureEvent::Frame(frame) | CaptureEvent::Gap(frame))) =
                    output
                else {
                    continue;
                };
                run.frames += 1;
                match &frame.aec_mic {
                    Some(aec) if Arc::ptr_eq(aec, &frame.raw_mic) => run.voice_processed += 1,
                    Some(_) => run.cancelled += 1,
                    None => {}
                }
                run.mic.extend_from_slice(&frame.preferred_mic());
                run.raw.extend_from_slice(&frame.raw_mic);
            }
        }
        run
    }

    /// One track at the live rate, through the same converter.
    fn converted(signal: &[f32]) -> Vec<f32> {
        let mut converter = Converter::new(OUT_RATE as u32, 1_920).unwrap();
        converter.push(Track::Microphone, 0, signal);
        converter.push(Track::Remote, 0, &vec![0.0; signal.len()]);
        converter
            .finish(signal.len() as u64)
            .into_iter()
            .flat_map(|chunk| chunk.frame.raw_mic.to_vec())
            .collect()
    }

    fn seconds(signal: &[f32], from: f32, to: f32, lag: usize) -> &[f32] {
        let at = |seconds: f32| (seconds * OUT_RATE as f32) as usize + lag;
        &signal[at(from)..at(to)]
    }

    fn power(signal: &[f32]) -> f32 {
        signal.iter().map(|s| s * s).sum::<f32>() / signal.len() as f32
    }

    fn db(power_ratio: f32) -> f32 {
        10.0 * power_ratio.max(1e-12).log10()
    }

    fn correlation(a: &[f32], b: &[f32]) -> f32 {
        let cross = a.iter().zip(b).map(|(a, b)| a * b).sum::<f32>() / a.len() as f32;
        cross / (power(a) * power(b)).sqrt()
    }

    /// The call talks alone for 6 s, the person alone from 6.5 s, then both from 10 s, through
    /// the built-in speakers' measured echo. Synthetic: this proves the adapter cancels a raw
    /// microphone's echo on its clock, not what a real room or speaker does.
    #[test]
    fn a_raw_microphone_on_speakers_has_the_calls_echo_cancelled_on_its_own_clock() {
        let mut far = speech(anlg_data::english_1::AUDIO_PATH, 15, 14, -20.0);
        silence(&mut far, 6.0, 10.0);
        let mut near = speech(anlg_data::english_2::AUDIO_PATH, 7, 14, -26.0);
        silence(&mut near, 0.0, 6.5);
        let echo = room_echo(&far, 48);
        let floor = noise(far.len(), -70.0);
        let mic: Vec<f32> = (0..far.len())
            .map(|n| echo[n] + near[n] + floor[n])
            .collect();

        let run = capture(&mic, &far, "raw", Arc::new(AtomicBool::new(true)));
        assert_eq!(run.cancelled, run.frames);
        let (near, echo) = (converted(&near), converted(&echo));

        let far_only = |s: &[f32], lag| power(seconds(s, 1.0, 6.0, lag));
        let reduction = db(far_only(&run.raw, 0) / far_only(&run.mic, AEC_LATENCY));
        assert!(reduction >= 30.0, "far-end echo reduced {reduction:.1} dB");

        // The person's speech is untouched and stays on the microphone's clock: the processed
        // track matches it exactly the model's block latency later, and at no other lag.
        let near_only = |s: &[f32], lag| seconds(s, 7.0, 10.0, lag).to_vec();
        let kept = db(power(&near_only(&run.mic, AEC_LATENCY)) / power(&near_only(&near, 0)));
        assert!(kept.abs() <= 1.5, "near-end speech changed {kept:.1} dB");
        for lag in [
            AEC_LATENCY - 16,
            AEC_LATENCY,
            AEC_LATENCY + 16,
            AEC_LATENCY + 768,
        ] {
            let matched = correlation(&near_only(&run.mic, lag), &near_only(&near, 0));
            assert_eq!(
                matched > 0.95,
                lag == AEC_LATENCY,
                "lag {lag}: {matched:.2}"
            );
        }

        // Both talking: the person stays, most of the echo goes.
        let both = |s: &[f32], lag| seconds(s, 11.0, 13.9, lag).to_vec();
        let residual: Vec<f32> = both(&run.mic, AEC_LATENCY)
            .iter()
            .zip(both(&near, 0))
            .map(|(mic, near)| mic - near)
            .collect();
        let double_talk = db(power(&both(&echo, 0)) / power(&residual));
        assert!(
            double_talk >= 5.0,
            "double-talk echo reduced {double_talk:.1} dB"
        );
    }

    /// A Bluetooth or display speaker can play a quarter second late. The reference is held
    /// back to a short lead once the lag locks, so the echo stays cancelled.
    #[test]
    fn a_late_speaker_route_is_cancelled_once_its_lag_locks() {
        let far = speech(anlg_data::english_1::AUDIO_PATH, 15, 8, -20.0);
        let echo = room_echo(&far, 250);
        let floor = noise(far.len(), -70.0);
        let mic: Vec<f32> = (0..far.len()).map(|n| echo[n] + floor[n]).collect();

        let run = capture(&mic, &far, "raw", Arc::new(AtomicBool::new(true)));
        let locked = |s: &[f32], lag| power(seconds(s, 5.0, 8.0 - 0.1, lag));
        let reduction = db(locked(&run.raw, 0) / locked(&run.mic, AEC_LATENCY));
        assert!(reduction >= 25.0, "late echo reduced {reduction:.1} dB");
    }

    /// Apple's voice processing already cancelled the echo; cancelling again would eat speech.
    /// Headphones leave no echo, so the listener turns the canceller off until speakers return.
    #[test]
    fn only_a_raw_microphone_the_listener_wants_cancelled_is_cancelled() {
        let far = speech(anlg_data::english_1::AUDIO_PATH, 15, 2, -20.0);
        let mic = room_echo(&far, 48);

        let wanted = Arc::new(AtomicBool::new(true));
        let run = capture(&mic, &far, "voice_processed", wanted.clone());
        assert_eq!(run.voice_processed, run.frames);
        assert_eq!(run.raw, run.mic);

        wanted.store(false, Ordering::Release);
        let run = capture(&mic, &far, "raw", wanted);
        assert_eq!((run.cancelled, run.voice_processed), (0, 0));
        assert_eq!(run.raw, run.mic);
    }
}
