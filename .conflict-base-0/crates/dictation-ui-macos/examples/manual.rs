mod common;

use std::time::Duration;

use dictation_ui_macos::{DictationState, Phase, hide, show, update_state};

fn main() {
    common::run_app(|| {
        std::thread::sleep(Duration::from_millis(200));
        show();

        let total_ms = 6000;
        let step_ms = 50;
        let steps = total_ms / step_ms;
        for i in 0..steps {
            let t = i as f32 * (step_ms as f32) / 1000.0;
            let amp = (t * 3.0).sin().abs() * 0.8;
            update_state(&DictationState {
                phase: Phase::Recording,
                amplitude: amp,
            });
            std::thread::sleep(Duration::from_millis(step_ms as u64));
        }

        update_state(&DictationState {
            phase: Phase::Processing,
            amplitude: 0.0,
        });
        std::thread::sleep(Duration::from_millis(1500));

        hide();
        std::thread::sleep(Duration::from_millis(500));
        std::process::exit(0);
    });
}
