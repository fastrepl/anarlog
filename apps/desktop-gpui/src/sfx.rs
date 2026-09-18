//! `plugins/sfx`'s `AppSounds::BGM`: the onboarding's looping background
//! music, decoded and mixed on a thread like the plugin's `to_speaker`.

use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::time::Duration;

/// The plugin's own asset, so both shells play the same track.
static BGM: &[u8] = include_bytes!("../../../plugins/sfx/sounds/bgm.mp3");

/// `BGM_VOLUME`
pub const BGM_VOLUME: f32 = 0.2;

enum SoundControl {
    Stop,
    SetVolume(f32),
}

/// A playing sound; dropping it stops playback (`sfxCommands.stop`).
pub struct Sound {
    control: Sender<SoundControl>,
}

impl Sound {
    /// `sfxCommands.play("BGM")`
    pub fn play_bgm() -> Self {
        Self::play(BGM, true, BGM_VOLUME)
    }

    /// `cuelume`'s `play(sound, { volume })`: render the recipe and play it
    /// once; the thread ends with the sound.
    pub fn play_cue(name: &str, volume: f32) -> Option<Self> {
        let recipe = crate::cuelume::recipe(name)?;
        let samples = crate::cuelume::render(&recipe, volume);
        let (control, rx) = channel();
        std::thread::Builder::new()
            .name("sfx-cue".into())
            .spawn(move || {
                let Ok(sink) = rodio::DeviceSinkBuilder::open_default_sink() else {
                    tracing::debug!("no playback device for sfx");
                    return;
                };
                let source = rodio::buffer::SamplesBuffer::new(
                    std::num::NonZero::new(1u16).expect("one channel"),
                    std::num::NonZero::new(crate::cuelume::SAMPLE_RATE).expect("sample rate"),
                    samples,
                );
                let player = rodio::Player::connect_new(sink.mixer());
                player.append(source);
                loop {
                    match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(SoundControl::Stop) | Err(RecvTimeoutError::Disconnected) => {
                            player.stop();
                            break;
                        }
                        Ok(SoundControl::SetVolume(volume)) => player.set_volume(volume),
                        Err(RecvTimeoutError::Timeout) => {
                            if player.empty() {
                                break;
                            }
                        }
                    }
                }
                drop(sink);
            })
            .ok();
        Some(Self { control })
    }

    fn play(bytes: &'static [u8], looping: bool, volume: f32) -> Self {
        use rodio::Source as _;
        let (control, rx) = channel();
        std::thread::Builder::new()
            .name("sfx".into())
            .spawn(move || {
                let Ok(sink) = rodio::DeviceSinkBuilder::open_default_sink() else {
                    tracing::debug!("no playback device for sfx");
                    return;
                };
                let Ok(source) = rodio::Decoder::try_from(std::io::Cursor::new(bytes)) else {
                    return;
                };
                let player = rodio::Player::connect_new(sink.mixer());
                player.set_volume(volume);
                if looping {
                    player.append(source.repeat_infinite());
                } else {
                    player.append(source);
                }
                loop {
                    match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(SoundControl::Stop) | Err(RecvTimeoutError::Disconnected) => {
                            player.stop();
                            break;
                        }
                        Ok(SoundControl::SetVolume(volume)) => player.set_volume(volume),
                        Err(RecvTimeoutError::Timeout) => {
                            if !looping && player.empty() {
                                break;
                            }
                        }
                    }
                }
                drop(sink);
            })
            .ok();
        Self { control }
    }

    /// `sfxCommands.setVolume`
    pub fn set_volume(&self, volume: f32) {
        let _ = self.control.send(SoundControl::SetVolume(volume));
    }
}

impl Drop for Sound {
    fn drop(&mut self) {
        let _ = self.control.send(SoundControl::Stop);
    }
}
