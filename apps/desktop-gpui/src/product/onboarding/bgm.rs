//! The onboarding background music, as the sfx plugin plays `AppSounds::BGM`: looped at 0.2
//! volume until onboarding ends; muting sets the volume to 0.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};

const VOLUME: f32 = 0.2;
const SOUND: &[u8] = include_bytes!("../../../../../plugins/sfx/sounds/bgm.mp3");

enum Control {
    Volume(f32),
    Stop,
}

pub struct Bgm(Sender<Control>);

impl Bgm {
    pub fn play() -> Self {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            use rodio::{Decoder, Player, Source};
            let Ok(stream) = anlg_audio_utils::open_default_playback_sink() else {
                return;
            };
            let Ok(source) = Decoder::try_from(std::io::Cursor::new(SOUND)) else {
                return;
            };
            let player = Player::connect_new(stream.mixer());
            player.set_volume(VOLUME);
            player.append(source.repeat_infinite());
            loop {
                match receiver.recv_timeout(std::time::Duration::from_millis(100)) {
                    Ok(Control::Volume(volume)) => player.set_volume(volume),
                    Ok(Control::Stop) | Err(RecvTimeoutError::Disconnected) => break,
                    Err(RecvTimeoutError::Timeout) => {}
                }
            }
            player.stop();
        });
        Self(sender)
    }

    pub fn set_muted(&self, muted: bool) {
        let _ = self
            .0
            .send(Control::Volume(if muted { 0. } else { VOLUME }));
    }
}

impl Drop for Bgm {
    fn drop(&mut self) {
        let _ = self.0.send(Control::Stop);
    }
}
