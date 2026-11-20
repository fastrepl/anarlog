use std::collections::VecDeque;

use futures_util::StreamExt;
use tauri::ipc::Channel;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::phrases::Phrases;
use super::stream::{FINISH_DEADLINE, LiveStream, StreamReport};
use super::wav::{self, WavWriter};
use super::{CaptureEndReason, DictationUpdate, Limits};
use crate::lease::MicGuard;
use crate::preview::PreviewConfig;

// Char's LIVE_AFTER_FRAMES (dictation-audio.ts, d4608723): a microphone that just opened is near
// silent for its first 150-200 ms, so speech is heard from the second 100 ms frame.
const READY_AFTER_FRAMES: u32 = 2;

pub struct Captured {
    pub samples: u64,
    pub phrases: Phrases,
    pub stream: Option<StreamReport>,
}

pub enum CaptureError {
    Cancelled,
    Recording(String),
}

pub struct Capture {
    pub microphone: anlg_audio::CaptureStream,
    pub guard: MicGuard,
    pub duck: Option<Box<dyn Send>>,
    pub writer: WavWriter,
    pub stream: Option<PreviewConfig>,
    pub updates: Channel<DictationUpdate>,
    pub limits: Limits,
}

/// Captures until stopped, cancelled, capped or the microphone ends, then releases the microphone
/// and output ducking before waiting on the stream, so another dictation can start at once.
pub async fn run(
    capture: Capture,
    stop: CancellationToken,
    cancel: CancellationToken,
    mut attach: mpsc::UnboundedReceiver<PreviewConfig>,
) -> Result<Captured, CaptureError> {
    let Capture {
        mut microphone,
        guard,
        duck,
        mut writer,
        stream,
        updates,
        limits,
    } = capture;
    let capacity = limits.early_frames + 64;
    let mut live = stream.map(|config| LiveStream::start(config, capacity, updates.clone()));
    let mut early: VecDeque<Vec<i16>> = VecDeque::new();
    let mut early_overflowed = false;
    let mut phrases = Phrases::default();
    let mut samples = 0_u64;
    let mut frames = 0_u32;
    let mut ended = None;
    let mut cancelled = false;

    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                cancelled = true;
                break;
            }
            _ = stop.cancelled() => break,
            Some(config) = attach.recv() => {
                if live.is_some() {
                    continue;
                }
                if early_overflowed {
                    let _ = updates.send(DictationUpdate::PreviewUnavailable);
                    continue;
                }
                let mut stream = LiveStream::start(config, capacity, updates.clone());
                for pcm in early.drain(..) {
                    if !stream.send(&pcm) {
                        break;
                    }
                }
                early.clear();
                live = Some(stream);
            }
            next = microphone.next() => {
                let Some(Ok(frame)) = next else {
                    ended = Some(CaptureEndReason::Failed);
                    break;
                };
                let frame = frame.preferred_mic();
                let pcm = wav::to_pcm16(&frame);
                if let Err(error) = writer.write(&pcm) {
                    return Err(CaptureError::Recording(error.to_string()));
                }
                samples += pcm.len() as u64;
                frames += 1;
                if frames == READY_AFTER_FRAMES {
                    let _ = updates.send(DictationUpdate::Ready);
                }
                let rms = phrases.push(&frame, &pcm);
                let _ = updates.send(DictationUpdate::Amplitude {
                    amplitude: (rms * 8.0).clamp(0.0, 1.0),
                });
                match &mut live {
                    Some(stream) => {
                        stream.send(&pcm);
                    }
                    None if !early_overflowed => {
                        if early.len() < limits.early_frames {
                            early.push_back(pcm);
                        } else {
                            early_overflowed = true;
                            early.clear();
                        }
                    }
                    None => {}
                }
                if samples >= limits.max_samples {
                    ended = Some(CaptureEndReason::Limit);
                    break;
                }
            }
        }
    }

    drop(microphone);
    drop(duck);
    drop(guard);
    phrases.flush();
    let samples_written = writer
        .finish()
        .map_err(|error| CaptureError::Recording(error.to_string()))?;
    if cancelled {
        return Err(CaptureError::Cancelled);
    }
    if let Some(reason) = ended {
        let _ = updates.send(DictationUpdate::CaptureEnded { reason });
    }
    let stream = match live {
        Some(stream) => tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(CaptureError::Cancelled),
            report = stream.finish(FINISH_DEADLINE) => Some(report),
        },
        None => None,
    };
    Ok(Captured {
        samples: samples_written,
        phrases,
        stream,
    })
}
