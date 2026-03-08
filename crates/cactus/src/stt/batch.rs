use std::cell::{Cell, UnsafeCell};
use std::ffi::{CStr, CString};
use std::path::Path;

use crate::error::{Error, Result};
use crate::ffi_utils::{RESPONSE_BUF_SIZE, parse_buf};
use crate::model::Model;

use crate::model::ModelKind;

use super::whisper::build_whisper_prompt;
use super::{TranscribeOptions, TranscriptionResult};

type TokenCallback = unsafe extern "C" fn(*const std::ffi::c_char, u32, *mut std::ffi::c_void);

struct CallbackState<'a, F: FnMut(&str) -> bool> {
    on_token: UnsafeCell<&'a mut F>,
    model: &'a Model,
    stopped: Cell<bool>,
    in_callback: Cell<bool>,
}

unsafe extern "C" fn token_trampoline<F: FnMut(&str) -> bool>(
    token: *const std::ffi::c_char,
    _token_id: u32,
    user_data: *mut std::ffi::c_void,
) {
    if token.is_null() || user_data.is_null() {
        return;
    }

    let state = unsafe { &*(user_data as *const CallbackState<F>) };
    if state.stopped.get() || state.in_callback.get() {
        return;
    }
    state.in_callback.set(true);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let chunk = unsafe { CStr::from_ptr(token) }.to_string_lossy();
        if chunk.starts_with("<|") && chunk.ends_with("|>") {
            return;
        }
        let on_token = unsafe { &mut *state.on_token.get() };
        if !on_token(&chunk) {
            state.stopped.set(true);
            state.model.stop();
        }
    }));

    state.in_callback.set(false);
    if result.is_err() {
        state.stopped.set(true);
        state.model.stop();
    }
}

enum TranscribeInput<'a> {
    File(&'a CString),
    Pcm(&'a [u8]),
}

impl Model {
    fn call_transcribe_inner(
        &self,
        input: TranscribeInput<'_>,
        options: &TranscribeOptions,
        callback: Option<TokenCallback>,
        user_data: *mut std::ffi::c_void,
    ) -> Result<TranscriptionResult> {
        let guard = self.lock_inference();
        let prompt = match self.kind() {
            ModelKind::Moonshine | ModelKind::Parakeet => String::new(),
            ModelKind::Whisper => build_whisper_prompt(options),
        };
        let prompt_c = CString::new(prompt)?;
        let options_c = CString::new(serde_json::to_string(options)?)?;
        let mut buf = vec![0u8; RESPONSE_BUF_SIZE];

        let (path_ptr, pcm_ptr, pcm_len) = match &input {
            TranscribeInput::File(p) => (p.as_ptr(), std::ptr::null(), 0),
            TranscribeInput::Pcm(p) => (std::ptr::null(), p.as_ptr(), p.len()),
        };

        let rc = unsafe {
            cactus_sys::cactus_transcribe(
                guard.raw_handle(),
                path_ptr,
                prompt_c.as_ptr(),
                buf.as_mut_ptr() as *mut std::ffi::c_char,
                buf.len(),
                options_c.as_ptr(),
                callback,
                user_data,
                pcm_ptr,
                pcm_len,
            )
        };

        if rc < 0 {
            return Err(Error::Inference(format!("cactus_transcribe failed ({rc})")));
        }

        Ok(parse_buf(&buf)?)
    }

    pub fn transcribe_file(
        &self,
        audio_path: impl AsRef<Path>,
        options: &TranscribeOptions,
    ) -> Result<TranscriptionResult> {
        let path_c = CString::new(audio_path.as_ref().to_string_lossy().into_owned())?;
        self.call_transcribe_inner(
            TranscribeInput::File(&path_c),
            options,
            None,
            std::ptr::null_mut(),
        )
    }

    pub fn transcribe_file_with_callback<F>(
        &self,
        audio_path: impl AsRef<Path>,
        options: &TranscribeOptions,
        mut on_token: F,
    ) -> Result<TranscriptionResult>
    where
        F: FnMut(&str) -> bool,
    {
        let path_c = CString::new(audio_path.as_ref().to_string_lossy().into_owned())?;
        let state = CallbackState {
            on_token: UnsafeCell::new(&mut on_token),
            model: self,
            stopped: Cell::new(false),
            in_callback: Cell::new(false),
        };
        let result = self.call_transcribe_inner(
            TranscribeInput::File(&path_c),
            options,
            Some(token_trampoline::<F>),
            &state as *const CallbackState<F> as *mut std::ffi::c_void,
        );
        if result.is_err() && state.stopped.get() {
            return result;
        }
        result
    }

    pub fn transcribe_pcm(
        &self,
        pcm: &[u8],
        options: &TranscribeOptions,
    ) -> Result<TranscriptionResult> {
        self.call_transcribe_inner(
            TranscribeInput::Pcm(pcm),
            options,
            None,
            std::ptr::null_mut(),
        )
    }

    pub fn transcribe_pcm_with_callback<F>(
        &self,
        pcm: &[u8],
        options: &TranscribeOptions,
        mut on_token: F,
    ) -> Result<TranscriptionResult>
    where
        F: FnMut(&str) -> bool,
    {
        let state = CallbackState {
            on_token: UnsafeCell::new(&mut on_token),
            model: self,
            stopped: Cell::new(false),
            in_callback: Cell::new(false),
        };
        self.call_transcribe_inner(
            TranscribeInput::Pcm(pcm),
            options,
            Some(token_trampoline::<F>),
            &state as *const CallbackState<F> as *mut std::ffi::c_void,
        )
    }
}
