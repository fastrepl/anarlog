#[allow(dead_code)]
const fn skip_wav_header(wav: &[u8]) -> &[u8] {
    unsafe { core::slice::from_raw_parts(wav.as_ptr().add(44), wav.len() - 44) }
}

macro_rules! include_wav {
    ($path:expr) => {{
        const WAV: &[u8] = include_bytes!($path);
        const PCM: &[u8] = crate::skip_wav_header(WAV);
        PCM
    }};
}

pub mod english_1;
pub mod english_10;
pub mod english_2;
pub mod english_3;
pub mod english_4;
pub mod english_5;
pub mod english_6;
pub mod english_7;
pub mod english_8;
pub mod english_9;

pub mod korean_1;
pub mod korean_2;
