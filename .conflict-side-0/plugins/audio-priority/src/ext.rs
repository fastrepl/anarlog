use std::collections::HashSet;
use std::path::PathBuf;

use tauri_plugin_settings::SettingsPluginExt;

use crate::{PriorityState, StoredDevice};
use hypr_audio_device::{AudioDevice, AudioDeviceBackend, AudioDirection, DeviceId, backend};

pub const FILENAME: &str = "audio.json";

pub fn audio_priority_path<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<PathBuf, crate::Error> {
    let base = app.settings().global_base()?;
    Ok(base.join(FILENAME).into_std_path_buf())
}

pub struct AudioPriority<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> AudioPriority<'a, R, M> {
    pub fn path(&self) -> Result<PathBuf, crate::Error> {
        audio_priority_path(self.manager.app_handle())
    }

    pub fn list_devices(&self) -> Result<Vec<AudioDevice>, crate::Error> {
        let backend = backend();
        backend.list_devices().map_err(Into::into)
    }

    pub fn list_input_devices(&self) -> Result<Vec<AudioDevice>, crate::Error> {
        let backend = backend();
        backend.list_input_devices().map_err(Into::into)
    }

    pub fn list_output_devices(&self) -> Result<Vec<AudioDevice>, crate::Error> {
        let backend = backend();
        backend.list_output_devices().map_err(Into::into)
    }

    pub fn get_default_input_device(&self) -> Result<Option<AudioDevice>, crate::Error> {
        let backend = backend();
        backend.get_default_input_device().map_err(Into::into)
    }

    pub fn get_default_output_device(&self) -> Result<Option<AudioDevice>, crate::Error> {
        let backend = backend();
        backend.get_default_output_device().map_err(Into::into)
    }

    pub fn set_default_input_device(&self, device_id: &str) -> Result<(), crate::Error> {
        let backend = backend();
        backend
            .set_default_input_device(&DeviceId::new(device_id))
            .map_err(Into::into)
    }

    pub fn set_default_output_device(&self, device_id: &str) -> Result<(), crate::Error> {
        let backend = backend();
        backend
            .set_default_output_device(&DeviceId::new(device_id))
            .map_err(Into::into)
    }

    pub fn is_headphone(&self, device: &AudioDevice) -> bool {
        let backend = backend();
        backend.is_headphone(device)
    }

    pub async fn load_state(&self) -> crate::Result<PriorityState> {
        let state = self.manager.state::<crate::state::AudioPriorityState>();
        state.load().await
    }

    pub async fn save_state(&self, priority_state: PriorityState) -> crate::Result<()> {
        let state = self.manager.state::<crate::state::AudioPriorityState>();
        state.save(priority_state).await
    }

    pub async fn get_input_priorities(&self) -> crate::Result<Vec<String>> {
        let state = self.load_state().await?;
        Ok(state.input_priorities)
    }

    pub async fn get_output_priorities(&self) -> crate::Result<Vec<String>> {
        let state = self.load_state().await?;
        Ok(state.output_priorities)
    }

    pub async fn save_input_priorities(&self, priorities: Vec<String>) -> crate::Result<()> {
        let mut state = self.load_state().await?;
        state.input_priorities =
            merge_priorities_preserving_disconnected(&state.input_priorities, &priorities);
        self.save_state(state).await
    }

    pub async fn save_output_priorities(&self, priorities: Vec<String>) -> crate::Result<()> {
        let mut state = self.load_state().await?;
        state.output_priorities =
            merge_priorities_preserving_disconnected(&state.output_priorities, &priorities);
        self.save_state(state).await
    }

    pub async fn move_device_to_top(
        &self,
        device_id: &str,
        direction: AudioDirection,
    ) -> crate::Result<()> {
        let mut state = self.load_state().await?;
        let uid = device_id.to_string();

        let priorities = match direction {
            AudioDirection::Input => &mut state.input_priorities,
            AudioDirection::Output => &mut state.output_priorities,
        };
        priorities.retain(|u| u != &uid);
        priorities.insert(0, uid);

        self.save_state(state).await
    }

    pub async fn get_known_devices(&self) -> crate::Result<Vec<StoredDevice>> {
        let state = self.load_state().await?;
        Ok(state.known_devices)
    }

    pub async fn remember_device(
        &self,
        uid: &str,
        name: &str,
        is_input: bool,
    ) -> crate::Result<()> {
        let mut state = self.load_state().await?;

        if let Some(device) = state.known_devices.iter_mut().find(|d| d.uid == uid) {
            device.name = name.to_string();
            device.update_last_seen();
        } else {
            state
                .known_devices
                .push(StoredDevice::new(uid, name, is_input));
        }

        self.save_state(state).await
    }

    pub async fn forget_device(&self, uid: &str) -> crate::Result<()> {
        let mut state = self.load_state().await?;
        state.known_devices.retain(|d| d.uid != uid);
        state.input_priorities.retain(|u| u != uid);
        state.output_priorities.retain(|u| u != uid);
        state.hidden_inputs.retain(|u| u != uid);
        state.hidden_outputs.retain(|u| u != uid);
        self.save_state(state).await
    }

    pub async fn is_device_hidden(
        &self,
        device_id: &str,
        direction: AudioDirection,
    ) -> crate::Result<bool> {
        let state = self.load_state().await?;
        let uid = device_id.to_string();

        match direction {
            AudioDirection::Input => Ok(state.hidden_inputs.contains(&uid)),
            AudioDirection::Output => Ok(state.hidden_outputs.contains(&uid)),
        }
    }

    pub async fn hide_device(
        &self,
        device_id: &str,
        direction: AudioDirection,
    ) -> crate::Result<()> {
        let mut state = self.load_state().await?;
        let uid = device_id.to_string();

        let hidden_list = match direction {
            AudioDirection::Input => &mut state.hidden_inputs,
            AudioDirection::Output => &mut state.hidden_outputs,
        };
        if !hidden_list.contains(&uid) {
            hidden_list.push(uid);
        }

        self.save_state(state).await
    }

    pub async fn unhide_device(
        &self,
        device_id: &str,
        direction: AudioDirection,
    ) -> crate::Result<()> {
        let mut state = self.load_state().await?;

        let hidden_list = match direction {
            AudioDirection::Input => &mut state.hidden_inputs,
            AudioDirection::Output => &mut state.hidden_outputs,
        };
        hidden_list.retain(|u| u != device_id);

        self.save_state(state).await
    }
}

fn merge_priorities_preserving_disconnected(
    old_priorities: &[String],
    new_priorities: &[String],
) -> Vec<String> {
    let new_set: HashSet<&str> = new_priorities.iter().map(|s| s.as_str()).collect();
    let mut new_iter = new_priorities.iter();
    let mut result: Vec<String> = Vec::new();

    for old_id in old_priorities {
        if new_set.contains(old_id.as_str()) {
            if let Some(new_id) = new_iter.next() {
                result.push(new_id.clone());
            }
        } else {
            result.push(old_id.clone());
        }
    }

    for remaining in new_iter {
        result.push(remaining.clone());
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> String {
        v.to_string()
    }

    #[test]
    fn merge_preserves_disconnected_devices_at_their_positions() {
        let old = vec![s("USB_A"), s("USB_B"), s("Built_In"), s("BT")];
        let new = vec![s("BT"), s("Built_In")];
        let result = merge_priorities_preserving_disconnected(&old, &new);
        assert_eq!(result, vec![s("USB_A"), s("USB_B"), s("BT"), s("Built_In")]);
    }

    #[test]
    fn merge_no_disconnected_devices() {
        let old = vec![s("A"), s("B"), s("C")];
        let new = vec![s("C"), s("A"), s("B")];
        let result = merge_priorities_preserving_disconnected(&old, &new);
        assert_eq!(result, vec![s("C"), s("A"), s("B")]);
    }

    #[test]
    fn merge_empty_old() {
        let old: Vec<String> = vec![];
        let new = vec![s("A"), s("B")];
        let result = merge_priorities_preserving_disconnected(&old, &new);
        assert_eq!(result, vec![s("A"), s("B")]);
    }

    #[test]
    fn merge_empty_new() {
        let old = vec![s("A"), s("B")];
        let new: Vec<String> = vec![];
        let result = merge_priorities_preserving_disconnected(&old, &new);
        assert_eq!(result, vec![s("A"), s("B")]);
    }

    #[test]
    fn merge_new_device_not_in_old() {
        let old = vec![s("A"), s("B")];
        let new = vec![s("B"), s("A"), s("C")];
        let result = merge_priorities_preserving_disconnected(&old, &new);
        assert_eq!(result, vec![s("B"), s("A"), s("C")]);
    }

    #[test]
    fn merge_swap_with_disconnected_in_middle() {
        let old = vec![s("Built_In"), s("USB_A"), s("USB_B"), s("BT")];
        let new = vec![s("USB_A"), s("BT")];
        let result = merge_priorities_preserving_disconnected(&old, &new);
        assert_eq!(result, vec![s("Built_In"), s("USB_A"), s("USB_B"), s("BT")]);
    }
}

pub trait AudioPriorityPluginExt<R: tauri::Runtime> {
    fn audio_priority(&self) -> AudioPriority<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> AudioPriorityPluginExt<R> for T {
    fn audio_priority(&self) -> AudioPriority<'_, R, Self>
    where
        Self: Sized,
    {
        AudioPriority {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
