mod encoder;
mod error;
mod wav;

pub use encoder::{Mono, MonoStreamEncoder, Stereo, StereoStreamEncoder, StreamEncoder};
pub use error::Error;
pub use wav::{concat_files, decode_to_wav, encode_wav};
