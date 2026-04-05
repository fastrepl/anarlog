use std::path::PathBuf;
use std::sync::Arc;

use crate::DenoiseEvent;
use crate::runtime::DenoiseRuntime;

const DENOISE_SAMPLE_RATE: u32 = 16000;
const CHUNK_SIZE: usize = 16000;

#[derive(Debug, Clone, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct DenoiseParams {
    pub session_id: String,
    pub input_path: PathBuf,
    pub output_path: PathBuf,
}

pub async fn run_denoise(
    runtime: Arc<dyn DenoiseRuntime>,
    params: DenoiseParams,
) -> crate::Result<()> {
    let rt = runtime.clone();
    let session_id = params.session_id.clone();

    let result = tokio::task::spawn_blocking(move || run_denoise_blocking(&runtime, &params))
        .await
        .map_err(|e| crate::Error::DenoiseError(e.to_string()))?;

    if let Err(e) = &result {
        rt.emit(DenoiseEvent::DenoiseFailed {
            session_id,
            error: e.to_string(),
        });
    }

    result
}

fn run_denoise_blocking(
    runtime: &Arc<dyn DenoiseRuntime>,
    params: &DenoiseParams,
) -> crate::Result<()> {
    runtime.emit(DenoiseEvent::DenoiseStarted {
        session_id: params.session_id.clone(),
    });

    let metadata = hypr_audio_utils::audio_file_metadata(&params.input_path)
        .map_err(|e| crate::Error::DenoiseError(e.to_string()))?;
    let channels = metadata.channels.max(1) as usize;

    let source = hypr_audio_utils::source_from_path(&params.input_path)
        .map_err(|e| crate::Error::DenoiseError(e.to_string()))?;

    let samples = hypr_audio_utils::resample_audio(source, DENOISE_SAMPLE_RATE)
        .map_err(|e| crate::Error::DenoiseError(e.to_string()))?;

    let channel_buffers = hypr_audio_utils::deinterleave(&samples, channels);

    let mut denoisers: Vec<hypr_denoise::onnx::Denoiser> = (0..channels)
        .map(|_| hypr_denoise::onnx::Denoiser::new())
        .collect::<Result<_, _>>()
        .map_err(|e| crate::Error::DenoiseError(e.to_string()))?;

    let total_chunks = channel_buffers[0].len().div_ceil(CHUNK_SIZE);
    let mut output_channels: Vec<Vec<f32>> = Vec::with_capacity(channels);

    for (ch_idx, (channel_data, denoiser)) in
        channel_buffers.iter().zip(denoisers.iter_mut()).enumerate()
    {
        let mut channel_output = Vec::with_capacity(channel_data.len());
        for (i, chunk) in channel_data.chunks(CHUNK_SIZE).enumerate() {
            let denoised = denoiser
                .process_streaming(chunk)
                .map_err(|e| crate::Error::DenoiseError(e.to_string()))?;
            channel_output.extend_from_slice(&denoised);

            let percentage = ((ch_idx * total_chunks + i + 1) as f64
                / (channels * total_chunks) as f64)
                .clamp(0.0, 1.0);
            runtime.emit(DenoiseEvent::DenoiseProgress {
                session_id: params.session_id.clone(),
                percentage,
            });
        }
        output_channels.push(channel_output);
    }

    let output = hypr_audio_utils::interleave(&output_channels);

    let spec = hound::WavSpec {
        channels: channels as u16,
        sample_rate: DENOISE_SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = hound::WavWriter::create(&params.output_path, spec)
        .map_err(|e| crate::Error::DenoiseError(e.to_string()))?;
    for &sample in &output {
        writer
            .write_sample(sample)
            .map_err(|e| crate::Error::DenoiseError(e.to_string()))?;
    }
    writer
        .finalize()
        .map_err(|e| crate::Error::DenoiseError(e.to_string()))?;

    runtime.emit(DenoiseEvent::DenoiseCompleted {
        session_id: params.session_id.clone(),
    });

    Ok(())
}
