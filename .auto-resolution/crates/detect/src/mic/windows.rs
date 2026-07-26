#[derive(Default)]
pub struct Detector {}

impl crate::Observer for Detector {
    fn start(&mut self, _f: crate::DetectCallback) {}
    fn stop(&mut self) {}
}
