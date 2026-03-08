use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::{Mutex, MutexGuard};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModelKind {
    #[default]
    Whisper,
    Moonshine,
    Parakeet,
}

pub struct Model {
    handle: NonNull<std::ffi::c_void>,
    inference_lock: Mutex<()>,
    kind: ModelKind,
}

unsafe impl Send for Model {}
// SAFETY: FFI calls are serialized by `inference_lock` via `InferenceGuard`.
// `stop()` only sets a C++ `std::atomic<bool>` and needs no lock.
unsafe impl Sync for Model {}

pub(crate) struct InferenceGuard<'a> {
    handle: NonNull<std::ffi::c_void>,
    _guard: MutexGuard<'a, ()>,
}

impl InferenceGuard<'_> {
    pub(crate) fn raw_handle(&self) -> *mut std::ffi::c_void {
        self.handle.as_ptr()
    }
}

pub struct ModelBuilder {
    model_path: PathBuf,
    kind: ModelKind,
}

impl ModelBuilder {
    pub fn kind(mut self, kind: ModelKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn build(self) -> Result<Model> {
        let path = CString::new(self.model_path.to_string_lossy().into_owned())?;
        let raw = unsafe { cactus_sys::cactus_init(path.as_ptr(), std::ptr::null(), false) };
        let handle =
            NonNull::new(raw).ok_or_else(|| Error::Init("cactus_init returned null".into()))?;

        Ok(Model {
            handle,
            inference_lock: Mutex::new(()),
            kind: self.kind,
        })
    }
}

impl Model {
    pub fn builder(model_path: impl AsRef<Path>) -> ModelBuilder {
        ModelBuilder {
            model_path: model_path.as_ref().to_path_buf(),
            kind: ModelKind::default(),
        }
    }

    pub fn new(model_path: impl AsRef<Path>) -> Result<Self> {
        Self::builder(model_path).build()
    }

    pub fn kind(&self) -> ModelKind {
        self.kind
    }

    pub fn stop(&self) {
        unsafe {
            cactus_sys::cactus_stop(self.handle.as_ptr());
        }
    }

    pub fn reset(&mut self) {
        let guard = self.lock_inference();
        unsafe {
            cactus_sys::cactus_reset(guard.raw_handle());
        }
    }

    pub(crate) fn lock_inference(&self) -> InferenceGuard<'_> {
        let guard = self.inference_lock.lock().unwrap_or_else(|e| {
            tracing::warn!(
                "inference mutex was poisoned (a previous FFI call likely panicked); \
                 recovering, but model state may be inconsistent"
            );
            e.into_inner()
        });
        InferenceGuard {
            handle: self.handle,
            _guard: guard,
        }
    }
}

impl Drop for Model {
    fn drop(&mut self) {
        let guard = self.lock_inference();
        unsafe {
            cactus_sys::cactus_destroy(guard.raw_handle());
        }
    }
}
