use rodio::stream::{DeviceSinkBuilder, DeviceSinkError, MixerDeviceSink};

pub fn open_default_playback_sink() -> Result<MixerDeviceSink, DeviceSinkError> {
    // rodio's default callback uses eprintln!, which panics if writing to stderr fails.
    let mut sink = DeviceSinkBuilder::from_default_device()?
        .with_error_callback(|err| log_audio_stream_error(&err))
        .open_stream()?;
    sink.log_on_drop(false);
    Ok(sink)
}

fn log_audio_stream_error(err: &dyn std::fmt::Display) {
    tracing::warn!(error = %err, "audio output stream error");
}

#[cfg(test)]
mod tests {
    use super::log_audio_stream_error;

    #[test]
    fn audio_stream_error_log_does_not_panic() {
        log_audio_stream_error(&"alsa::poll() returned POLLERR");
        log_audio_stream_error(&"device not available");
    }
}
