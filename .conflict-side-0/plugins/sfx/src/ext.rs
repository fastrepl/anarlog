use std::sync::{LazyLock, Mutex};

pub(crate) enum SoundControl {
    Stop,
    SetVolume(f32),
}

struct SoundHandle {
    control_tx: std::sync::mpsc::Sender<SoundControl>,
}

static PLAYING_SOUNDS: LazyLock<Mutex<std::collections::HashMap<AppSounds, SoundHandle>>> =
    LazyLock::new(|| Mutex::new(std::collections::HashMap::new()));

#[derive(Debug, serde::Serialize, serde::Deserialize, specta::Type, Clone, PartialEq, Eq, Hash)]
pub enum AppSounds {
    StartRecording,
    StopRecording,
    BGM,
}

pub(crate) fn to_speaker(
    bytes: &'static [u8],
    looping: bool,
) -> std::sync::mpsc::Sender<SoundControl> {
    use rodio::source::Source;
    use rodio::{Decoder, Player, stream::DeviceSinkBuilder};
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let Ok(stream) = DeviceSinkBuilder::open_default_sink() else {
            return;
        };

        let file = std::io::Cursor::new(bytes);
        let Ok(source) = Decoder::try_from(file) else {
            return;
        };

        let player = Player::connect_new(stream.mixer());

        if looping {
            player.append(source.repeat_infinite());
        } else {
            player.append(source);
        }

        loop {
            match rx.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(SoundControl::Stop) => {
                    player.stop();
                    break;
                }
                Ok(SoundControl::SetVolume(volume)) => {
                    player.set_volume(volume);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if !looping && player.empty() {
                        break;
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    break;
                }
            }
        }
        drop(stream);
    });

    tx
}

impl AppSounds {
    pub fn play(&self) {
        self.stop();

        let bytes = self.get_sound_bytes();
        let looping = matches!(self, AppSounds::BGM);
        let control_tx = to_speaker(bytes, looping);

        {
            let mut sounds = PLAYING_SOUNDS.lock().unwrap();
            sounds.insert(self.clone(), SoundHandle { control_tx });
        }
    }

    pub fn stop(&self) {
        let mut sounds = PLAYING_SOUNDS.lock().unwrap();
        if let Some(handle) = sounds.remove(self) {
            let _ = handle.control_tx.send(SoundControl::Stop);
        }
    }

    pub fn set_volume(&self, volume: f32) {
        let sounds = PLAYING_SOUNDS.lock().unwrap();
        if let Some(handle) = sounds.get(self) {
            let _ = handle.control_tx.send(SoundControl::SetVolume(volume));
        }
    }

    fn get_sound_bytes(&self) -> &'static [u8] {
        match self {
            AppSounds::StartRecording => include_bytes!("../sounds/start_recording.ogg"),
            AppSounds::StopRecording => include_bytes!("../sounds/stop_recording.ogg"),
            AppSounds::BGM => include_bytes!("../sounds/bgm.mp3"),
        }
    }
}

pub struct Sfx<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Sfx<'a, R, M> {
    pub fn play(&self, sfx: AppSounds) {
        let _ = self.manager;
        sfx.play();
    }

    pub fn stop(&self, sfx: AppSounds) {
        let _ = self.manager;
        sfx.stop();
    }

    pub fn set_volume(&self, sfx: AppSounds, volume: f32) {
        let _ = self.manager;
        sfx.set_volume(volume);
    }
}

pub trait SfxPluginExt<R: tauri::Runtime> {
    fn sfx(&self) -> Sfx<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> SfxPluginExt<R> for T {
    fn sfx(&self) -> Sfx<'_, R, Self>
    where
        Self: Sized,
    {
        Sfx {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
