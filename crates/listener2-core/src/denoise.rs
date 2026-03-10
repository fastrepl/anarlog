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

    let source = hypr_audio_utils::source_from_path(&params.input_path)
        .map_err(|e| crate::Error::DenoiseError(e.to_string()))?;

    let samples = hypr_audio_utils::resample_audio(source, DENOISE_SAMPLE_RATE)
        .map_err(|e| crate::Error::DenoiseError(e.to_string()))?;

    let mut denoiser = hypr_denoise::onnx::Denoiser::new()
        .map_err(|e| crate::Error::DenoiseError(e.to_string()))?;

    let total_chunks = samples.len().div_ceil(CHUNK_SIZE);
    let mut output = Vec::with_capacity(samples.len());

    for (i, chunk) in samples.chunks(CHUNK_SIZE).enumerate() {
        let denoised = denoiser
            .process_streaming(chunk)
            .map_err(|e| crate::Error::DenoiseError(e.to_string()))?;
        output.extend_from_slice(&denoised);

        let percentage = ((i + 1) as f64 / total_chunks as f64).clamp(0.0, 1.0);
        runtime.emit(DenoiseEvent::DenoiseProgress {
            session_id: params.session_id.clone(),
            percentage,
        });
    }

    let spec = hound::WavSpec {
        channels: 1,
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
