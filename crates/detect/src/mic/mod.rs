#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
type PlatformDetector = macos::Detector;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
type PlatformDetector = windows::Detector;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
type PlatformDetector = linux::Detector;

#[derive(Default)]
pub struct MicDetector {
    inner: PlatformDetector,
}

impl crate::Observer for MicDetector {
    fn start(&mut self, f: crate::DetectCallback) {
        self.inner.start(f);
    }
    fn stop(&mut self) {
        self.inner.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Observer, new_callback};
    use std::time::Duration;

    // cargo test --package detect --lib --features mic,list -- mic::tests::test_detector --exact --nocapture --ignored
    #[tokio::test]
    #[ignore]
    async fn test_detector() {
        let mut detector = MicDetector::default();
        detector.start(new_callback(|v| {
            println!("{:?}", v);
        }));

        tokio::time::sleep(Duration::from_secs(60)).await;
        detector.stop();
    }
}
