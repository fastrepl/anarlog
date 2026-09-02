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
    let result = match commit_accelerated(bytes) {
        Ok(session) => Ok(session),
        // A compiled model that no longer loads (a crash or a concurrent
        // process left it half-written) would otherwise pin every future
        // load to the CPU path; a rebuild from scratch is cheap by comparison.
        Err(err) if clear_accelerator_cache() => {
            tracing::warn!(error = %err, "accelerated session cache rebuilt");
            commit_accelerated(bytes)
        }
        Err(err) => Err(err),
    };
    match result {
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

// Keyed by executable so an installed build and a dev build (or two app
// versions during an update) never compile into each other's cache; the
// in-process lock cannot protect against another process.
#[cfg(feature = "coreml")]
fn accelerator_cache_dir() -> std::path::PathBuf {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::hash::DefaultHasher::new();
    std::env::current_exe()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default()
        .hash(&mut hasher);
    std::env::temp_dir()
        .join("anlg-onnx-coreml-cache")
        .join(format!("{:016x}", hasher.finish()))
}

/// Returns whether there was a cache to remove.
#[cfg(feature = "coreml")]
fn clear_accelerator_cache() -> bool {
    let _guard = COREML_COMPILE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::fs::remove_dir_all(accelerator_cache_dir()).is_ok()
}

#[cfg(not(feature = "coreml"))]
fn clear_accelerator_cache() -> bool {
    false
}

#[cfg(feature = "coreml")]
fn commit_accelerated(bytes: &[u8]) -> Result<Session, Error> {
    use ort::execution_providers::{
        CoreMLExecutionProvider,
        coreml::{CoreMLComputeUnits, CoreMLModelFormat},
    };

    // MLProgram cannot bound the unbounded input dims these models declare (the
    // speaker embedding extractor takes a variable frame count) and returns
    // non-finite output instead of refusing the graph; the GPU unit fails to
    // resize dynamically. NeuralNetwork on the ANE is the one combination that
    // both compiles and matches CPU output, and it runs ~10x faster than CPU.
    let provider = CoreMLExecutionProvider::default()
        .with_model_format(CoreMLModelFormat::NeuralNetwork)
        .with_compute_units(CoreMLComputeUnits::CPUAndNeuralEngine)
        .with_model_cache_dir(accelerator_cache_dir().to_string_lossy());
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
