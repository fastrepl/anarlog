use std::num::{NonZeroU16, NonZeroU32};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use anlg_audio_utils::{Source, open_default_playback_sink, source_from_path};
use desktop_runtime::{Result, ServiceError};
use rodio::Player;

use super::model::failure;

pub const RATES: [f32; 7] = [0.5, 0.75, 1., 1.25, 1.5, 1.75, 2.];
const MAX_PEAKS: usize = 4096;

pub(crate) struct Centered<S>(pub S);

impl<S: Source> Iterator for Centered<S> {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let channels = usize::from(self.0.channels().get());
        anlg_audio_utils::mono_frames(&mut self.0, channels).next()
    }
}

impl<S: Source> Source for Centered<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.0
            .current_span_len()
            .map(|samples| samples / usize::from(self.0.channels().get()))
    }
    fn channels(&self) -> NonZeroU16 {
        NonZeroU16::MIN
    }
    fn sample_rate(&self) -> NonZeroU32 {
        self.0.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.0.total_duration()
    }
    fn try_seek(
        &mut self,
        position: Duration,
    ) -> std::result::Result<(), rodio::source::SeekError> {
        self.0.try_seek(position)
    }
}

pub enum Command {
    Open(PathBuf),
    Play,
    Pause,
    Seek(Duration),
    Rate(f32),
}

#[derive(Clone, Default)]
pub struct Snapshot {
    pub path: Option<PathBuf>,
    pub playing: bool,
    pub position: Duration,
    pub duration: Duration,
    pub rate: f32,
    pub waveform: Arc<[[f32; 2]]>,
    pub error: Option<ServiceError>,
    pub revision: u64,
}

#[derive(Clone)]
pub struct Playback {
    commands: mpsc::SyncSender<Command>,
    state: Arc<Mutex<Snapshot>>,
}

impl Playback {
    pub fn spawn() -> Result<Self> {
        let (commands, receiver) = mpsc::sync_channel(16);
        let state = Arc::new(Mutex::new(Snapshot {
            rate: 1.,
            ..Snapshot::default()
        }));
        let shared = state.clone();
        std::thread::Builder::new()
            .name("meeting-playback".into())
            .spawn(move || {
                let mut player: Option<Player> = None;
                let mut device = None;
                loop {
                    let command = receiver.recv_timeout(Duration::from_millis(100));
                    // Opening only decodes; the output device connects on first play, so
                    // viewing a note never holds an audio device.
                    let mut connect = |player: &mut Option<Player>| -> Result<()> {
                        if player.is_some() {
                            return Ok(());
                        }
                        let (path, position, rate) = {
                            let state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
                            (state.path.clone(), state.position, state.rate)
                        };
                        let path = path.ok_or_else(|| failure("No playable audio is loaded."))?;
                        let stream = open_default_playback_sink().map_err(failure)?;
                        let sink = Player::connect_new(stream.mixer());
                        sink.pause();
                        sink.append(Centered(source_from_path(&path).map_err(failure)?));
                        sink.set_speed(rate);
                        if !position.is_zero() {
                            sink.try_seek(position).map_err(failure)?;
                        }
                        *player = Some(sink);
                        device = Some(stream);
                        Ok(())
                    };
                    let result = match command {
                        Ok(Command::Open(path)) => {
                            if let Some(player) = player.take() {
                                player.stop();
                            }
                            device = None;
                            {
                                let mut state =
                                    shared.lock().unwrap_or_else(|poison| poison.into_inner());
                                *state = Snapshot {
                                    rate: 1.,
                                    revision: state.revision + 1,
                                    ..Snapshot::default()
                                };
                            }
                            decode_waveform(&path).map(|(waveform, duration)| {
                                let mut state =
                                    shared.lock().unwrap_or_else(|poison| poison.into_inner());
                                state.path = Some(path);
                                state.duration = duration;
                                state.waveform = waveform;
                                state.error = None;
                                state.revision += 1;
                            })
                        }
                        Ok(Command::Play) => connect(&mut player).and_then(|()| {
                            let Some(sink) = &player else {
                                return Ok(());
                            };
                            if sink.empty() {
                                let path = shared
                                    .lock()
                                    .unwrap_or_else(|poison| poison.into_inner())
                                    .path
                                    .clone()
                                    .ok_or_else(|| failure("No audio loaded."))?;
                                sink.append(Centered(source_from_path(path).map_err(failure)?));
                            }
                            sink.play();
                            Ok(())
                        }),
                        Ok(Command::Pause) => {
                            if let Some(sink) = &player {
                                sink.pause();
                            }
                            Ok(())
                        }
                        Ok(Command::Seek(position)) => match &player {
                            Some(sink) => sink.try_seek(position).map_err(failure),
                            None => {
                                let mut state =
                                    shared.lock().unwrap_or_else(|poison| poison.into_inner());
                                if state.path.is_some() {
                                    state.position = position.min(state.duration);
                                    state.revision += 1;
                                    Ok(())
                                } else {
                                    Err(failure("No playable audio is loaded."))
                                }
                            }
                        },
                        Ok(Command::Rate(rate)) if RATES.contains(&rate) => {
                            if let Some(sink) = &player {
                                sink.set_speed(rate);
                            }
                            let mut state =
                                shared.lock().unwrap_or_else(|poison| poison.into_inner());
                            state.rate = rate;
                            state.revision += 1;
                            Ok(())
                        }
                        Ok(Command::Rate(_)) => Err(failure("Unsupported playback rate.")),
                        Err(mpsc::RecvTimeoutError::Timeout) => Ok(()),
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    let mut state = shared.lock().unwrap_or_else(|poison| poison.into_inner());
                    if let Some(sink) = &player {
                        let position = if sink.empty() {
                            state.duration
                        } else {
                            sink.get_pos()
                        };
                        let playing = !sink.is_paused() && !sink.empty();
                        if state.position != position || state.playing != playing {
                            state.position = position.min(state.duration);
                            state.playing = playing;
                            state.revision += 1;
                        }
                    }
                    if let Err(error) = result {
                        state.error = Some(error);
                        state.revision += 1;
                    }
                }
                drop(device);
            })
            .map_err(failure)?;
        Ok(Self { commands, state })
    }

    pub fn send(&self, command: Command) -> Result<()> {
        self.commands
            .try_send(command)
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => ServiceError::Busy,
                mpsc::TrySendError::Disconnected(_) => ServiceError::Closed,
            })
    }

    pub fn snapshot(&self) -> Snapshot {
        self.state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
    }
}

pub fn decode_waveform(path: &std::path::Path) -> Result<(Arc<[[f32; 2]]>, Duration)> {
    let mut decoder = source_from_path(path).map_err(failure)?;
    let channels = u16::from(decoder.channels()) as usize;
    let rate = u32::from(decoder.sample_rate()) as u64;
    if channels == 0 || rate == 0 {
        return Err(failure("Invalid audio metadata."));
    }
    let mut peaks: Vec<[f32; 2]> = Vec::with_capacity(MAX_PEAKS);
    let mut frames = 0_u64;
    let mut bucket_width = (rate / 100).max(1);
    let mut in_bucket = 0;
    let mut peak = [0_f32; 2];
    while let Some(first) = decoder.next() {
        peak[0] = peak[0].max(first.abs());
        if channels == 1 {
            peak[1] = peak[0];
        }
        for channel in 1..channels {
            let value = decoder
                .next()
                .ok_or_else(|| failure("Truncated audio frame."))?;
            peak[channel.min(1)] = peak[channel.min(1)].max(value.abs());
        }
        frames += 1;
        in_bucket += 1;
        if in_bucket == bucket_width {
            peaks.push(peak);
            peak = [0., 0.];
            in_bucket = 0;
            if peaks.len() == MAX_PEAKS {
                for index in 0..MAX_PEAKS / 2 {
                    peaks[index] = [
                        peaks[index * 2][0].max(peaks[index * 2 + 1][0]),
                        peaks[index * 2][1].max(peaks[index * 2 + 1][1]),
                    ];
                }
                peaks.truncate(MAX_PEAKS / 2);
                bucket_width *= 2;
            }
        }
    }
    if in_bucket != 0 {
        peaks.push(peak);
    }
    let maximum = peaks.iter().flatten().copied().fold(0_f32, f32::max);
    if maximum > 0. {
        for peak in &mut peaks {
            for value in peak {
                *value = (*value / maximum).clamp(0., 1.);
            }
        }
    }
    Ok((
        peaks.into(),
        Duration::from_secs_f64(frames as f64 / rate as f64),
    ))
}
