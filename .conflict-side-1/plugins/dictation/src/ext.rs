use crate::{error::Error, events::Phase, handler::Handler};

pub struct Dictation<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Dictation<'a, R, M> {
    pub fn show(&self) -> Result<(), Error> {
        self.manager.state::<Handler>().show()
    }

    pub fn hide(&self) -> Result<(), Error> {
        self.manager.state::<Handler>().hide()
    }

    pub fn set_phase(&self, phase: Phase) -> Result<(), Error> {
        self.manager.state::<Handler>().set_phase(phase)
    }

    pub fn update_amplitude(&self, amplitude: f32) -> Result<(), Error> {
        self.manager.state::<Handler>().update_amplitude(amplitude)
    }
}

pub trait DictationPluginExt<R: tauri::Runtime> {
    fn dictation(&self) -> Dictation<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> DictationPluginExt<R> for T {
    fn dictation(&self) -> Dictation<'_, R, Self>
    where
        Self: Sized,
    {
        Dictation {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
