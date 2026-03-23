use std::ffi::CStr;

pub(crate) const RESPONSE_BUF_SIZE: usize = 64 * 1024;

pub(crate) fn read_cstr_from_buf(buf: &[u8]) -> String {
    CStr::from_bytes_until_nul(buf)
        .map(|c| c.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub(crate) fn parse_buf<T: serde::de::DeserializeOwned>(buf: &[u8]) -> serde_json::Result<T> {
    serde_json::from_str(&read_cstr_from_buf(buf))
}
