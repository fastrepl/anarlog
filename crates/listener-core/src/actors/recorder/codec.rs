use std::path::Path;

pub struct Mp3Codec;

impl AudioCodec for Mp3Codec {
    fn extension(&self) -> &str {
        "mp3"
    }

    fn encode(&self, input: &Path, output: &Path) -> Result<(), Box<dyn std::error::Error>> {
        Ok(hypr_mp3::encode_wav(input, output)?)
    }

    fn decode(&self, input: &Path, output: &Path) -> Result<(), Box<dyn std::error::Error>> {
        Ok(hypr_mp3::decode_to_wav(input, output)?)
    }
}

pub trait AudioCodec: Send + Sync + 'static {
    fn extension(&self) -> &str;
    fn encode(&self, input: &Path, output: &Path) -> Result<(), Box<dyn std::error::Error>>;
    fn decode(&self, input: &Path, output: &Path) -> Result<(), Box<dyn std::error::Error>>;
}
