use std::time::Duration;

#[derive(Clone)]
pub struct Config {
    pub interval: Duration,
    pub sync_timeout: Duration,
}

impl Config {
    pub fn every(interval: Duration) -> Self {
        assert!(
            !interval.is_zero(),
            "calendar sync interval must be greater than zero"
        );

        Self {
            interval,
            sync_timeout: Duration::from_secs(30),
        }
    }

    pub fn every_minute() -> Self {
        Self::every(Duration::from_secs(60))
    }
}
