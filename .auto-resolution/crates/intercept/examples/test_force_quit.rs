mod common;

use intercept::*;

fn main() {
    common::run_app(|| {
        std::thread::sleep(std::time::Duration::from_millis(200));
        demo_quit_progress();

        std::thread::sleep(std::time::Duration::from_secs(5));
        std::process::exit(0);
    });
}
