use rodio::cpal::traits::HostTrait;
use rodio::stream::{DeviceSinkBuilder, DeviceSinkError, MixerDeviceSink};
use rodio::{DeviceTrait, cpal};

pub fn open_default_playback_sink() -> Result<MixerDeviceSink, DeviceSinkError> {
    DeviceSinkBuilder::from_default_device()
        .and_then(open_builder)
        .or_else(|original_err| {
            let devices = match cpal::default_host().output_devices() {
                Ok(devices) => devices,
                Err(err) => {
                    tracing::warn!(error = %err, "error getting list of output devices");
                    return Err(original_err);
                }
            };
            devices
                .filter(is_usable_output_device)
                .find_map(|device| {
                    DeviceSinkBuilder::from_device(device)
                        .and_then(open_builder)
                        .ok()
                })
                .ok_or(original_err)
        })
}

fn open_builder(builder: DeviceSinkBuilder) -> Result<MixerDeviceSink, DeviceSinkError> {
    // rodio's default callback uses eprintln!, which panics if writing to stderr fails.
    builder
        .with_error_callback(log_audio_stream_error)
        .open_sink_or_fallback()
        .map(disable_drop_log)
}

fn disable_drop_log(mut sink: MixerDeviceSink) -> MixerDeviceSink {
    sink.log_on_drop(false);
    sink
}

fn is_usable_output_device(device: &impl DeviceTrait) -> bool {
    is_usable_output_driver(
        device
            .description()
            .ok()
            .as_ref()
            .and_then(|description| description.driver()),
    )
}

fn is_usable_output_driver(driver: Option<&str>) -> bool {
    driver.is_some_and(|driver| driver != "null")
}

fn log_audio_stream_error(err: cpal::StreamError) {
    tracing::warn!(error = %err, "audio output stream error");
}

#[cfg(test)]
mod tests {
    use super::{is_usable_output_driver, log_audio_stream_error};
    use rodio::cpal::StreamError;

    #[test]
    fn audio_stream_error_log_does_not_panic() {
        log_audio_stream_error(StreamError::DeviceNotAvailable);
    }

    #[test]
    fn skips_null_and_unknown_output_drivers() {
        assert!(!is_usable_output_driver(None));
        assert!(!is_usable_output_driver(Some("null")));
        assert!(is_usable_output_driver(Some("alsa")));
    }
}
