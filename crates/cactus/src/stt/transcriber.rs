use std::ffi::CString;
use std::ptr::NonNull;

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::ffi_utils::{RESPONSE_BUF_SIZE, read_cstr_from_buf};
use crate::model::Model;

use super::TranscribeOptions;

fn deserialize_number_as_u64<'de, D>(deserializer: D) -> std::result::Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    match v {
        serde_json::Value::Number(n) => n
            .as_u64()
            .or_else(|| n.as_f64().map(|f| f as u64))
            .ok_or_else(|| serde::de::Error::custom("expected non-negative number")),
        _ => Err(serde::de::Error::custom("expected a number")),
    }
}

#[derive(Debug, Clone, Default)]
pub struct CloudConfig {
    pub api_key: Option<String>,
    pub threshold: Option<f32>,
}

impl CloudConfig {
    pub(super) fn prepare_env(&self) {
        if let Some(key) = &self.api_key {
            // SAFETY: called under inference_lock, matching the C++ read.
            unsafe { std::env::set_var("CACTUS_CLOUD_API_KEY", key) };
        }
    }
}

fn serialize_stream_options(options: &TranscribeOptions, cloud: &CloudConfig) -> Result<CString> {
    let mut v = serde_json::to_value(options)?;
    if let (Some(map), Some(t)) = (v.as_object_mut(), cloud.threshold) {
        map.insert("cloud_handoff_threshold".into(), t.into());
    }
    Ok(CString::new(serde_json::to_string(&v)?)?)
}

pub struct Transcriber<'a> {
    pub(super) handle: Option<NonNull<std::ffi::c_void>>,
    pub(super) model: &'a Model,
    cloud: CloudConfig,
}

// SAFETY: FFI calls are serialized through Model's inference_lock.
unsafe impl Send for Transcriber<'_> {}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct StreamResult {
    #[serde(default)]
    pub confirmed: String,
    #[serde(default)]
    pub pending: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub cloud_handoff: bool,
    #[serde(default)]
    pub cloud_job_id: u64,
    #[serde(default)]
    pub cloud_result_job_id: u64,
    #[serde(default)]
    pub cloud_result: String,
    #[serde(default)]
    pub cloud_result_used_cloud: bool,
    #[serde(default)]
    pub cloud_result_error: Option<String>,
    #[serde(default)]
    pub cloud_result_source: String,
    #[serde(default)]
    pub confirmed_local: String,
    #[serde(default)]
    pub buffer_duration_ms: f64,
    #[serde(default)]
    pub confidence: f32,
    #[serde(default)]
    pub time_to_first_token_ms: f64,
    #[serde(default)]
    pub total_time_ms: f64,
    #[serde(default)]
    pub prefill_tps: f64,
    #[serde(default)]
    pub decode_tps: f64,
    #[serde(default)]
    pub ram_usage_mb: f64,
    #[serde(default, deserialize_with = "deserialize_number_as_u64")]
    pub prefill_tokens: u64,
    #[serde(default, deserialize_with = "deserialize_number_as_u64")]
    pub decode_tokens: u64,
    #[serde(default, deserialize_with = "deserialize_number_as_u64")]
    pub total_tokens: u64,
}

impl std::str::FromStr for StreamResult {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(serde_json::from_str(s).unwrap_or_else(|e| {
            tracing::warn!(
                error.message = %e,
                hyprnote.payload.raw = s,
                "cactus_stream_result_parse_failed"
            );
            Self {
                confirmed: s.to_string(),
                ..Default::default()
            }
        }))
    }
}

impl<'a> Transcriber<'a> {
    pub fn new(model: &'a Model, options: &TranscribeOptions, cloud: CloudConfig) -> Result<Self> {
        let guard = model.lock_inference();
        let options = model.transcribe_options(options);
        let options_c = serialize_stream_options(&options, &cloud)?;

        let raw = unsafe {
            cactus_sys::cactus_stream_transcribe_start(guard.raw_handle(), options_c.as_ptr())
        };

        let handle = NonNull::new(raw).ok_or_else(|| {
            Error::Inference("cactus_stream_transcribe_start returned null".into())
        })?;

        Ok(Self {
            handle: Some(handle),
            model,
            cloud,
        })
    }

    pub fn process(&mut self, pcm: &[u8]) -> Result<StreamResult> {
        let _guard = self.model.lock_inference();
        self.cloud.prepare_env();
        let mut buf = vec![0u8; RESPONSE_BUF_SIZE];

        let rc = unsafe {
            cactus_sys::cactus_stream_transcribe_process(
                self.raw_handle()?,
                pcm.as_ptr(),
                pcm.len(),
                buf.as_mut_ptr() as *mut std::ffi::c_char,
                buf.len(),
            )
        };

        if rc < 0 {
            return Err(Error::Inference(format!(
                "cactus_stream_transcribe_process failed ({rc})"
            )));
        }

        Ok(parse_stream_result(&buf))
    }

    pub fn process_samples(&mut self, samples: &[i16]) -> Result<StreamResult> {
        let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        self.process(&bytes)
    }

    pub fn process_f32(&mut self, samples: &[f32]) -> Result<StreamResult> {
        let converted: Vec<i16> = samples
            .iter()
            .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
            .collect();
        self.process_samples(&converted)
    }

    pub fn stop(mut self) -> Result<StreamResult> {
        let result = self.call_stop();
        self.handle = None;
        result
    }

    fn call_stop(&self) -> Result<StreamResult> {
        let _guard = self.model.lock_inference();
        self.cloud.prepare_env();
        let mut buf = vec![0u8; RESPONSE_BUF_SIZE];

        let rc = unsafe {
            cactus_sys::cactus_stream_transcribe_stop(
                self.raw_handle()?,
                buf.as_mut_ptr() as *mut std::ffi::c_char,
                buf.len(),
            )
        };

        if rc < 0 {
            return Err(Error::Inference(format!(
                "cactus_stream_transcribe_stop failed ({rc})"
            )));
        }

        Ok(parse_stream_result(&buf))
    }

    pub(super) fn raw_handle(&self) -> Result<*mut std::ffi::c_void> {
        self.handle
            .map(NonNull::as_ptr)
            .ok_or_else(|| Error::Inference("transcriber has already been stopped".to_string()))
    }
}

impl Drop for Transcriber<'_> {
    fn drop(&mut self) {
        let Some(handle) = self.handle.take() else {
            return;
        };
        let _guard = self.model.lock_inference();
        let mut buf = vec![0u8; RESPONSE_BUF_SIZE];
        unsafe {
            cactus_sys::cactus_stream_transcribe_stop(
                handle.as_ptr(),
                buf.as_mut_ptr() as *mut std::ffi::c_char,
                buf.len(),
            );
        }
    }
}

fn parse_stream_result(buf: &[u8]) -> StreamResult {
    read_cstr_from_buf(buf).parse().unwrap()
}
