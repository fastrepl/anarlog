use std::ffi::CString;
use std::path::Path;

use crate::error::{Error, Result};
use crate::ffi_utils::{RESPONSE_BUF_SIZE, parse_buf};
use crate::model::Model;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct VadOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub neg_threshold: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_speech_duration_ms: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_speech_duration_s: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_silence_duration_ms: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speech_pad_ms: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_size_samples: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling_rate: Option<i32>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VadSegment {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VadResult {
    pub segments: Vec<VadSegment>,
    pub total_time_ms: f64,
}

#[derive(serde::Deserialize)]
struct RawVadResponse {
    success: bool,
    error: Option<String>,
    #[serde(default)]
    segments: Vec<VadSegment>,
    #[serde(default)]
    total_time_ms: f64,
}

impl Model {
    fn call_vad(
        &self,
        path: Option<&CString>,
        pcm: Option<&[u8]>,
        options: &VadOptions,
    ) -> Result<VadResult> {
        let guard = self.lock_inference();
        let options_c = CString::new(serde_json::to_string(options)?)?;
        let mut buf = vec![0u8; RESPONSE_BUF_SIZE];

        let (pcm_ptr, pcm_len) = pcm
            .map(|p| (p.as_ptr(), p.len()))
            .unwrap_or((std::ptr::null(), 0));

        let rc = unsafe {
            cactus_sys::cactus_vad(
                guard.raw_handle(),
                path.map_or(std::ptr::null(), |p| p.as_ptr()),
                buf.as_mut_ptr() as *mut std::ffi::c_char,
                buf.len(),
                options_c.as_ptr(),
                pcm_ptr,
                pcm_len,
            )
        };

        if rc < 0 {
            return Err(Error::Inference(format!("cactus_vad failed ({rc})")));
        }

        let resp: RawVadResponse = parse_buf(&buf)
            .map_err(|e| Error::Inference(format!("failed to parse VAD response: {e}")))?;

        if !resp.success {
            return Err(Error::Inference(
                resp.error.unwrap_or_else(|| "unknown VAD error".into()),
            ));
        }

        Ok(VadResult {
            segments: resp.segments,
            total_time_ms: resp.total_time_ms,
        })
    }

    pub fn vad_file(
        &self,
        audio_path: impl AsRef<Path>,
        options: &VadOptions,
    ) -> Result<VadResult> {
        let path_c = CString::new(audio_path.as_ref().to_string_lossy().into_owned())?;
        self.call_vad(Some(&path_c), None, options)
    }

    pub fn vad_pcm(&self, pcm: &[u8], options: &VadOptions) -> Result<VadResult> {
        self.call_vad(None, Some(pcm), options)
    }
}
