use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use hypr_audio_device::{AudioDevice, AudioDirection};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct StoredDevice {
    pub uid: String,
    pub name: String,
    pub is_input: bool,
    pub last_seen: u64,
}

impl StoredDevice {
    pub fn new(uid: impl Into<String>, name: impl Into<String>, is_input: bool) -> Self {
        Self {
            uid: uid.into(),
            name: name.into(),
            is_input,
            last_seen: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    pub fn last_seen_relative(&self) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let interval = now.saturating_sub(self.last_seen);

        if interval < 60 {
            "now".to_string()
        } else if interval < 3600 {
            let mins = interval / 60;
            format!("{}m ago", mins)
        } else if interval < 86400 {
            let hours = interval / 3600;
            format!("{}h ago", hours)
        } else if interval < 604800 {
            let days = interval / 86400;
            format!("{}d ago", days)
        } else if interval < 2592000 {
            let weeks = interval / 604800;
            format!("{}w ago", weeks)
        } else {
            let months = interval / 2592000;
            format!("{}mo ago", months)
        }
    }

    pub fn update_last_seen(&mut self) {
        self.last_seen = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
pub struct PriorityState {
    #[serde(default)]
    pub input_priorities: Vec<String>,
    #[serde(default, alias = "speaker_priorities")]
    pub output_priorities: Vec<String>,
    #[serde(default, alias = "hidden_mics")]
    pub hidden_inputs: Vec<String>,
    #[serde(default, alias = "hidden_speakers")]
    pub hidden_outputs: Vec<String>,
    #[serde(default)]
    pub known_devices: Vec<StoredDevice>,
}

#[derive(Debug, Clone)]
pub struct PriorityManager {
    state: PriorityState,
}

impl Default for PriorityManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PriorityManager {
    pub fn new() -> Self {
        Self {
            state: PriorityState::default(),
        }
    }

    pub fn from_state(state: PriorityState) -> Self {
        Self { state }
    }

    pub fn state(&self) -> &PriorityState {
        &self.state
    }

    pub fn into_state(self) -> PriorityState {
        self.state
    }

    pub fn get_known_devices(&self) -> &[StoredDevice] {
        &self.state.known_devices
    }

    pub fn remember_device(&mut self, uid: &str, name: &str, is_input: bool) {
        if let Some(device) = self.state.known_devices.iter_mut().find(|d| d.uid == uid) {
            device.name = name.to_string();
            device.update_last_seen();
        } else {
            self.state
                .known_devices
                .push(StoredDevice::new(uid, name, is_input));
        }
    }

    pub fn get_stored_device(&self, uid: &str) -> Option<&StoredDevice> {
        self.state.known_devices.iter().find(|d| d.uid == uid)
    }

    pub fn forget_device(&mut self, uid: &str) {
        self.state.known_devices.retain(|d| d.uid != uid);
        self.state.input_priorities.retain(|u| u != uid);
        self.state.output_priorities.retain(|u| u != uid);
        self.state.hidden_inputs.retain(|u| u != uid);
        self.state.hidden_outputs.retain(|u| u != uid);
    }

    pub fn is_hidden(&self, device: &AudioDevice) -> bool {
        let uid = device.id.as_str();
        match device.direction {
            AudioDirection::Input => self.state.hidden_inputs.contains(&uid.to_string()),
            AudioDirection::Output => self.state.hidden_outputs.contains(&uid.to_string()),
        }
    }

    pub fn hide_device(&mut self, device: &AudioDevice) {
        let uid = device.id.as_str().to_string();
        let hidden_list = match device.direction {
            AudioDirection::Input => &mut self.state.hidden_inputs,
            AudioDirection::Output => &mut self.state.hidden_outputs,
        };
        if !hidden_list.contains(&uid) {
            hidden_list.push(uid);
        }
    }

    pub fn unhide_device(&mut self, device: &AudioDevice) {
        let uid = device.id.as_str();
        let hidden_list = match device.direction {
            AudioDirection::Input => &mut self.state.hidden_inputs,
            AudioDirection::Output => &mut self.state.hidden_outputs,
        };
        hidden_list.retain(|u| u != uid);
    }

    pub fn sort_by_priority(
        &self,
        devices: &[AudioDevice],
        direction: AudioDirection,
    ) -> Vec<AudioDevice> {
        let priorities = match direction {
            AudioDirection::Input => &self.state.input_priorities,
            AudioDirection::Output => &self.state.output_priorities,
        };
        self.sort_devices_by_priorities(devices, priorities)
    }

    fn sort_devices_by_priorities(
        &self,
        devices: &[AudioDevice],
        priorities: &[String],
    ) -> Vec<AudioDevice> {
        let mut sorted = devices.to_vec();
        sorted.sort_by(|a, b| {
            let index_a = priorities
                .iter()
                .position(|u| u == a.id.as_str())
                .unwrap_or(usize::MAX);
            let index_b = priorities
                .iter()
                .position(|u| u == b.id.as_str())
                .unwrap_or(usize::MAX);
            index_a.cmp(&index_b)
        });
        sorted
    }

    pub fn save_priorities(&mut self, devices: &[AudioDevice], direction: AudioDirection) {
        let uids: Vec<String> = devices.iter().map(|d| d.id.as_str().to_string()).collect();
        match direction {
            AudioDirection::Input => self.state.input_priorities = uids,
            AudioDirection::Output => self.state.output_priorities = uids,
        }
    }

    pub fn move_device_to_top(&mut self, device: &AudioDevice) {
        let uid = device.id.as_str().to_string();
        let priorities = match device.direction {
            AudioDirection::Input => &mut self.state.input_priorities,
            AudioDirection::Output => &mut self.state.output_priorities,
        };
        priorities.retain(|u| u != &uid);
        priorities.insert(0, uid);
    }

    pub fn get_highest_priority_device<'a>(
        &self,
        devices: &'a [AudioDevice],
        direction: AudioDirection,
    ) -> Option<&'a AudioDevice> {
        let sorted = self.sort_by_priority(devices, direction);
        sorted
            .into_iter()
            .next()
            .and_then(|d| devices.iter().find(|dev| dev.id == d.id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_device(id: &str, name: &str, direction: AudioDirection) -> AudioDevice {
        AudioDevice::new(
            id,
            name,
            direction,
            hypr_audio_device::TransportType::Unknown,
        )
    }

    #[test]
    fn test_remember_device() {
        let mut manager = PriorityManager::new();

        manager.remember_device("uid1", "Device 1", true);
        assert!(manager.get_stored_device("uid1").is_some());
        assert_eq!(manager.get_stored_device("uid1").unwrap().name, "Device 1");

        manager.remember_device("uid1", "Device 1 Updated", true);
        assert_eq!(
            manager.get_stored_device("uid1").unwrap().name,
            "Device 1 Updated"
        );
    }

    #[test]
    fn test_forget_device() {
        let mut manager = PriorityManager::new();

        manager.remember_device("uid1", "Device 1", true);
        assert!(manager.get_stored_device("uid1").is_some());

        manager.forget_device("uid1");
        assert!(manager.get_stored_device("uid1").is_none());
    }

    #[test]
    fn test_hide_unhide_device() {
        let mut manager = PriorityManager::new();
        let device = make_device("uid1", "Device 1", AudioDirection::Input);

        assert!(!manager.is_hidden(&device));

        manager.hide_device(&device);
        assert!(manager.is_hidden(&device));

        manager.unhide_device(&device);
        assert!(!manager.is_hidden(&device));
    }
}
