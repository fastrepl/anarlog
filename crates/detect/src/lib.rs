#[cfg(feature = "app")]
mod app;
mod error;
#[cfg(all(target_os = "macos", feature = "language"))]
mod language;
#[cfg(feature = "list")]
mod list;
#[cfg(feature = "mic")]
mod mic;
#[cfg(all(target_os = "macos", feature = "sleep"))]
mod sleep;

mod utils;

pub use error::Error;

pub use utils::BackgroundTask;

#[cfg(all(
    target_os = "macos",
    feature = "zoom",
    feature = "mic",
    feature = "list"
))]
mod zoom;

#[cfg(feature = "app")]
pub use app::*;
#[cfg(all(target_os = "macos", feature = "language"))]
pub use language::*;
#[cfg(feature = "list")]
pub use list::*;
#[cfg(feature = "mic")]
pub use mic::*;

#[cfg(all(
    target_os = "macos",
    feature = "zoom",
    feature = "mic",
    feature = "list"
))]
pub use zoom::*;

#[cfg(all(target_os = "macos", feature = "sleep"))]
pub use sleep::*;

#[cfg(feature = "mic")]
#[derive(Debug, Clone)]
pub enum DetectEvent {
    MicStarted(Vec<InstalledApp>),
    MicStopped(Vec<InstalledApp>),
    #[cfg(all(target_os = "macos", feature = "zoom"))]
    ZoomMuteStateChanged {
        value: bool,
    },
    #[cfg(all(target_os = "macos", feature = "sleep"))]
    SleepStateChanged {
        value: bool,
    },
}

#[cfg(feature = "mic")]
pub type DetectCallback = std::sync::Arc<dyn Fn(DetectEvent) + Send + Sync + 'static>;

#[cfg(feature = "mic")]
pub fn new_callback<F>(f: F) -> DetectCallback
where
    F: Fn(DetectEvent) + Send + Sync + 'static,
{
    std::sync::Arc::new(f)
}

#[cfg(feature = "mic")]
pub(crate) trait Observer: Send + Sync {
    fn start(&mut self, f: DetectCallback);
    fn stop(&mut self);
}

#[cfg(feature = "mic")]
#[derive(Default)]
pub struct Detector {
    mic_detector: MicDetector,
    #[cfg(all(target_os = "macos", feature = "zoom", feature = "list"))]
    zoom_watcher: ZoomMuteWatcher,
    #[cfg(all(target_os = "macos", feature = "sleep"))]
    sleep_detector: SleepDetector,
}

#[cfg(feature = "mic")]
impl Detector {
    pub fn start(&mut self, f: DetectCallback) {
        self.mic_detector.start(f.clone());

        #[cfg(all(target_os = "macos", feature = "zoom", feature = "list"))]
        self.zoom_watcher.start(f.clone());

        #[cfg(all(target_os = "macos", feature = "sleep"))]
        self.sleep_detector.start(f);
    }

    pub fn stop(&mut self) {
        self.mic_detector.stop();

        #[cfg(all(target_os = "macos", feature = "zoom", feature = "list"))]
        self.zoom_watcher.stop();

        #[cfg(all(target_os = "macos", feature = "sleep"))]
        self.sleep_detector.stop();
    }
}
