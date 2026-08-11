mod error;
pub use error::*;

use ort::{
    Result,
    session::{
        Session,
        builder::{GraphOptimizationLevel, SessionBuilder},
    },
};

pub use ndarray;
pub use ort;

pub fn load_model_from_bytes(bytes: &[u8]) -> Result<Session, Error> {
    Ok(session_builder()?.commit_from_memory(bytes)?)
}

// Opt-in per call site rather than global: small per-frame models on the
// live path (e.g. Silero VAD) are faster on plain CPU than through CoreML
// dispatch, so only batch workloads should ask for acceleration.
pub fn load_model_from_bytes_accelerated(bytes: &[u8]) -> Result<Session, Error> {
    match commit_accelerated(bytes) {
        Ok(session) => Ok(session),
        // A model the accelerator cannot compile must not take the feature down
        // with it; plain CPU produces the same values, only slower.
        Err(err) => {
            tracing::warn!(
                error = %err,
                "accelerated session unavailable, falling back to CPU"
            );
            load_model_from_bytes(bytes)
        }
    }
}

fn session_builder() -> Result<SessionBuilder, Error> {
    Ok(Session::builder()?
        .with_intra_threads(1)?
        .with_inter_threads(1)?
        .with_optimization_level(GraphOptimizationLevel::Level3)?)
}

// onnxruntime compiles the model into the shared cache with a non-atomic move,
// so two sessions built at the same time race and all but one fail to commit.
#[cfg(feature = "coreml")]
static COREML_COMPILE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(feature = "coreml")]
fn commit_accelerated(bytes: &[u8]) -> Result<Session, Error> {
    use ort::execution_providers::{
        CoreMLExecutionProvider,
        coreml::{CoreMLComputeUnits, CoreMLModelFormat},
    };

    // MLProgram cannot bound the unbounded input dims these models declare (the
    // WeSpeaker extractor takes a variable frame count) and returns non-finite
    // output instead of refusing the graph; the GPU unit fails to resize
    // dynamically. NeuralNetwork on the ANE is the one combination that both
    // compiles and matches CPU output, and it runs ~10x faster than CPU.
    let cache_dir = std::env::temp_dir().join("anlg-onnx-coreml-cache");
    let provider = CoreMLExecutionProvider::default()
        .with_model_format(CoreMLModelFormat::NeuralNetwork)
        .with_compute_units(CoreMLComputeUnits::CPUAndNeuralEngine)
        .with_model_cache_dir(cache_dir.to_string_lossy());
    let builder = session_builder()?.with_execution_providers([provider.build()])?;

    let _guard = COREML_COMPILE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    Ok(builder.commit_from_memory(bytes)?)
}

#[cfg(not(feature = "coreml"))]
fn commit_accelerated(bytes: &[u8]) -> Result<Session, Error> {
    Ok(session_builder()?.commit_from_memory(bytes)?)
}

pub fn load_model_from_path(path: impl AsRef<std::path::Path>) -> Result<Session, Error> {
    let bytes = std::fs::read(path)?;
    load_model_from_bytes(&bytes)
}
