use crate::Result;
use flate2::read::GzDecoder;
use std::io::Read;

const IOS_CORE_TIME_OFFSET: i64 = 978307200;

pub fn core_time_to_unix(core_time: i64) -> i64 {
    core_time + IOS_CORE_TIME_OFFSET
}

pub fn is_gzip(data: &[u8]) -> bool {
    data.len() > 2 && data[0] == 0x1f && data[1] == 0x8b
}

pub fn decompress_gzip(data: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = GzDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}
