use std::cell::{Cell, UnsafeCell};
use std::ffi::{CStr, CString};

use crate::error::{Error, Result};
use crate::ffi_utils::{RESPONSE_BUF_SIZE, parse_buf};
use crate::model::{InferenceGuard, Model};

use super::request::{PreparedRequest, serialize_complete_request};
use super::{CompleteOptions, CompletionResult, Message};

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

pub(super) fn complete_error(rc: i32) -> Error {
    Error::Inference(format!("cactus_complete failed ({rc})"))
}

impl Model {
    fn call_complete(
        &self,
        guard: &InferenceGuard<'_>,
        messages_c: &CString,
        options_c: &CString,
        callback: Option<TokenCallback>,
        user_data: *mut std::ffi::c_void,
    ) -> (i32, Vec<u8>) {
        let mut buf = vec![0u8; RESPONSE_BUF_SIZE];

        let rc = unsafe {
            cactus_sys::cactus_complete(
                guard.raw_handle(),
                messages_c.as_ptr(),
                buf.as_mut_ptr().cast::<std::ffi::c_char>(),
                buf.len(),
                options_c.as_ptr(),
                std::ptr::null(),
                callback,
                user_data,
            )
        };

        (rc, buf)
    }

    pub fn complete(
        &self,
        messages: &[Message],
        options: &CompleteOptions,
    ) -> Result<CompletionResult> {
        let guard = self.lock_inference();
        let request = serialize_complete_request(messages, options)?;
        self.complete_prepared(&guard, &request)
    }

    pub fn complete_streaming<F>(
        &self,
        messages: &[Message],
        options: &CompleteOptions,
        on_token: F,
    ) -> Result<CompletionResult>
    where
        F: FnMut(&str) -> bool,
    {
        let guard = self.lock_inference();
        let request = serialize_complete_request(messages, options)?;
        self.complete_prepared_streaming_with_guard(&guard, &request, on_token)
    }

    pub(super) fn complete_prepared_streaming<F>(
        &self,
        request: &PreparedRequest,
        on_token: F,
    ) -> Result<CompletionResult>
    where
        F: FnMut(&str) -> bool,
    {
        let guard = self.lock_inference();
        self.complete_prepared_streaming_with_guard(&guard, request, on_token)
    }

    fn complete_prepared(
        &self,
        guard: &InferenceGuard<'_>,
        request: &PreparedRequest,
    ) -> Result<CompletionResult> {
        let (rc, buf) = self.call_complete(
            guard,
            &request.messages_c,
            &request.options_c,
            None,
            std::ptr::null_mut(),
        );

        if rc < 0 {
            return Err(complete_error(rc));
        }

        Ok(parse_buf(&buf)?)
    }

    fn complete_prepared_streaming_with_guard<F>(
        &self,
        guard: &InferenceGuard<'_>,
        request: &PreparedRequest,
        mut on_token: F,
    ) -> Result<CompletionResult>
    where
        F: FnMut(&str) -> bool,
    {
        let state = CallbackState {
            on_token: UnsafeCell::new(&mut on_token),
            model: self,
            stopped: Cell::new(false),
            in_callback: Cell::new(false),
        };

        let (rc, buf) = self.call_complete(
            guard,
            &request.messages_c,
            &request.options_c,
            Some(token_trampoline::<F>),
            &state as *const CallbackState<F> as *mut std::ffi::c_void,
        );

        if rc < 0 && !state.stopped.get() {
            return Err(complete_error(rc));
        }

        Ok(parse_buf(&buf)?)
    }
}
