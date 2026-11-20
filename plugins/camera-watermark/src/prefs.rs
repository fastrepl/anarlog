use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// A built-in scene the camera draws behind the person (`WatermarkBackground.Scene`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum CameraScene {
    Studio,
    Dusk,
    Daylight,
}

impl CameraScene {
    fn wire(self) -> &'static str {
        match self {
            Self::Studio => "studio",
            Self::Dusk => "dusk",
            Self::Daylight => "daylight",
        }
    }
}

/// What Anarlog Camera puts behind the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CameraBackground {
    None,
    Blur,
    Scene { scene: CameraScene },
}

impl CameraBackground {
    /// The scene used when Anarlog replaces a meeting app's background image.
    pub const DEFAULT_IMAGE: Self = Self::Scene {
        scene: CameraScene::Studio,
    };

    pub fn hides_room(self) -> bool {
        !matches!(self, Self::None)
    }

    /// The agent's wire value, read by `WatermarkBackground.decode`.
    pub fn wire(self) -> serde_json::Value {
        match self {
            Self::None => serde_json::json!({ "kind": "none" }),
            Self::Blur => serde_json::json!({ "kind": "blur" }),
            Self::Scene { scene } => serde_json::json!({ "kind": "image", "image": scene.wire() }),
        }
    }
}

/// The person's camera choices. Everything is off until they opt in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct CameraPrefs {
    pub enabled: bool,
    pub auto_switch: bool,
    pub background: CameraBackground,
}

impl Default for CameraPrefs {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_switch: false,
            background: CameraBackground::None,
        }
    }
}

impl CameraPrefs {
    /// Both the camera and automatic switching are on.
    pub fn switches(&self) -> bool {
        self.enabled && self.auto_switch
    }
}

/// The saved background and the write that produced it. Every background write gets a new
/// revision, so a compare-and-set can tell "still what I wrote" from "changed back to the same
/// value".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackgroundRevision {
    pub background: CameraBackground,
    pub revision: u64,
}

/// This device's camera preferences, in one JSON file of their own. They stay local: the camera
/// extension and the meeting apps are per Mac. A missing or unreadable file is everything off.
///
/// Settings and the automatic switch write the Background from different threads. Each write
/// hands it to `apply` (the camera) before the next one can start, so the camera gets them in the
/// order they were saved and ends on the saved one.
pub struct PrefsStore {
    path: Option<PathBuf>,
    state: Mutex<(CameraPrefs, u64)>,
}

impl PrefsStore {
    pub fn load(path: Option<PathBuf>) -> Self {
        let prefs = path
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<CameraPrefs>(&bytes).ok())
            .unwrap_or_default();
        Self {
            path,
            state: Mutex::new((prefs, 0)),
        }
    }

    pub fn get(&self) -> CameraPrefs {
        self.lock().0
    }

    pub fn background(&self) -> BackgroundRevision {
        let (prefs, revision) = *self.lock();
        BackgroundRevision {
            background: prefs.background,
            revision,
        }
    }

    pub fn set_enabled(&self, enabled: bool) -> crate::Result<CameraPrefs> {
        self.update(|prefs| prefs.enabled = enabled)
    }

    pub fn set_auto_switch(&self, auto_switch: bool) -> crate::Result<CameraPrefs> {
        self.update(|prefs| prefs.auto_switch = auto_switch)
    }

    /// The person's own choice: always wins, and is a new write even when the value is the same,
    /// so an automatic undo never takes it back.
    pub fn set_background<T>(
        &self,
        background: CameraBackground,
        apply: impl FnOnce(CameraBackground) -> T,
    ) -> crate::Result<(BackgroundRevision, T)> {
        let mut state = self.lock();
        let written = self.write_background(&mut state, background)?;
        Ok((written, apply(background)))
    }

    /// Writes `background` only while the saved one is still exactly `expected`; None, and
    /// nothing applied, when the person changed it since.
    pub fn set_background_if<T>(
        &self,
        expected: BackgroundRevision,
        background: CameraBackground,
        apply: impl FnOnce(CameraBackground) -> T,
    ) -> crate::Result<Option<(BackgroundRevision, T)>> {
        let mut state = self.lock();
        if state.1 != expected.revision || state.0.background != expected.background {
            return Ok(None);
        }
        let written = self.write_background(&mut state, background)?;
        Ok(Some((written, apply(background))))
    }

    fn write_background(
        &self,
        state: &mut (CameraPrefs, u64),
        background: CameraBackground,
    ) -> crate::Result<BackgroundRevision> {
        let next = CameraPrefs {
            background,
            ..state.0
        };
        self.persist(&next)?;
        *state = (next, state.1 + 1);
        Ok(BackgroundRevision {
            background,
            revision: state.1,
        })
    }

    fn update(&self, change: impl FnOnce(&mut CameraPrefs)) -> crate::Result<CameraPrefs> {
        let mut state = self.lock();
        let mut next = state.0;
        change(&mut next);
        // Fail closed: memory changes only once the file did.
        self.persist(&next)?;
        state.0 = next;
        Ok(next)
    }

    fn persist(&self, prefs: &CameraPrefs) -> crate::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
        std::fs::write(&temporary, serde_json::to_vec_pretty(prefs)?)?;
        std::fs::rename(&temporary, path)?;
        Ok(())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, (CameraPrefs, u64)> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DUSK: CameraBackground = CameraBackground::Scene {
        scene: CameraScene::Dusk,
    };

    #[test]
    fn everything_is_off_until_the_person_opts_in() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("camera-watermark.json");
        assert_eq!(
            PrefsStore::load(Some(path.clone())).get(),
            CameraPrefs::default()
        );
        std::fs::write(&path, b"{not json").unwrap();
        assert_eq!(PrefsStore::load(Some(path)).get(), CameraPrefs::default());
    }

    #[test]
    fn choices_survive_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("camera-watermark.json");
        let store = PrefsStore::load(Some(path.clone()));
        store.set_enabled(true).unwrap();
        store.set_auto_switch(true).unwrap();
        store.set_background(DUSK, |_| ()).unwrap();
        let reloaded = PrefsStore::load(Some(path));
        assert_eq!(
            reloaded.get(),
            CameraPrefs {
                enabled: true,
                auto_switch: true,
                background: DUSK
            }
        );
    }

    #[test]
    fn a_failed_write_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        // A directory where the file should be: every write fails.
        let path = dir.path().join("camera-watermark.json");
        std::fs::create_dir(&path).unwrap();
        let store = PrefsStore::load(Some(path));
        assert!(store.set_enabled(true).is_err());
        assert!(!store.get().enabled);
    }

    #[test]
    fn compare_and_set_refuses_a_background_the_person_changed() {
        let store = PrefsStore::load(None);
        let before = store.background();
        let (written, ()) = store
            .set_background_if(before, CameraBackground::Blur, |_| ())
            .unwrap()
            .unwrap();
        assert_eq!(store.get().background, CameraBackground::Blur);

        // The person picks Blur themselves: same value, but their write.
        store
            .set_background(CameraBackground::Blur, |_| ())
            .unwrap();
        let undo = store
            .set_background_if(written, CameraBackground::None, |_| {
                panic!("a refused write reaches the camera")
            })
            .unwrap();
        assert!(undo.is_none());
        assert_eq!(store.get().background, CameraBackground::Blur);

        // Turning switching off is no background write: an undo still applies.
        let written = store.background();
        store.set_auto_switch(false).unwrap();
        assert!(
            store
                .set_background_if(written, DUSK, |_| ())
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn the_helper_reads_each_background_as_char_camera_did() {
        assert_eq!(CameraBackground::None.wire()["kind"], "none");
        assert_eq!(CameraBackground::Blur.wire()["kind"], "blur");
        assert_eq!(
            DUSK.wire(),
            serde_json::json!({"kind": "image", "image": "dusk"})
        );
        assert!(!CameraBackground::None.hides_room());
        assert!(CameraBackground::DEFAULT_IMAGE.hides_room());
    }
}
