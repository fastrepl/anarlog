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
    if device
        .id()
        .ok()
        .is_some_and(|id| is_silent_output_pcm(&id.1))
    {
        return false;
    }

    let Some(description) = device.description().ok() else {
        return false;
    };
    if is_silent_output_pcm(description.name())
        || description.driver().is_some_and(is_silent_output_pcm)
    {
        return false;
    }

    description.driver().is_some()
}

fn is_silent_output_pcm(id: &str) -> bool {
    matches!(
        id.trim()
            .split_once([':', ','])
            .map_or(id.trim(), |(plugin, _)| plugin),
        "null" | "dummy"
    )
}

fn log_audio_stream_error(err: cpal::StreamError) {
    tracing::warn!(error = %err, "audio output stream error");
}

#[cfg(test)]
mod tests {
    use super::{is_silent_output_pcm, log_audio_stream_error};
    use rodio::cpal::StreamError;

    #[test]
    fn audio_stream_error_log_does_not_panic() {
        log_audio_stream_error(StreamError::DeviceNotAvailable);
    }

    #[test]
    fn skips_alsa_discard_pcms() {
        assert!(is_silent_output_pcm("null"));
        assert!(is_silent_output_pcm("dummy"));
        assert!(is_silent_output_pcm("null:CARD=Dummy"));
        assert!(!is_silent_output_pcm("alsa"));
        assert!(!is_silent_output_pcm("default"));
        assert!(!is_silent_output_pcm("pipewire"));
        assert!(!is_silent_output_pcm("hw:0,0"));
    }
}
