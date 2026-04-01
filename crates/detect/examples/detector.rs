use std::time::Duration;

use detect::{Detector, new_callback};

#[tokio::main]
async fn main() {
    let mut detector = Detector::default();

    let callback = new_callback(|event| {
        println!("{:?}", event);
    });

    detector.start(callback);
    tokio::time::sleep(Duration::from_secs(30)).await;
    detector.stop();
}
