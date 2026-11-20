use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

pub const SAMPLE_RATE: u32 = 16_000;
const HEADER_LEN: u64 = 44;

/// Mono 16 kHz PCM16 WAV. Every frame is flushed, and a file left behind by a crash is made
/// readable again by `repair`, so recovery never depends on `finish` having run.
pub struct WavWriter {
    file: BufWriter<File>,
    samples: u64,
}

impl WavWriter {
    pub fn create(path: &Path) -> std::io::Result<Self> {
        let mut file = BufWriter::new(super::private_file(path)?);
        file.write_all(&header(0))?;
        file.flush()?;
        Ok(Self { file, samples: 0 })
    }

    pub fn write(&mut self, samples: &[i16]) -> std::io::Result<()> {
        for sample in samples {
            self.file.write_all(&sample.to_le_bytes())?;
        }
        self.samples += samples.len() as u64;
        self.file.flush()
    }

    pub fn finish(mut self) -> std::io::Result<u64> {
        self.file.flush()?;
        let mut file = self.file.into_inner().map_err(|error| error.into_error())?;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(&header(self.samples * 2))?;
        file.sync_all()?;
        Ok(self.samples)
    }
}

pub fn write_all(path: &Path, samples: &[i16]) -> std::io::Result<()> {
    let mut writer = WavWriter::create(path)?;
    writer.write(samples)?;
    writer.finish().map(|_| ())
}

/// Rewrites the header from the file length and returns the sample count.
pub fn repair(path: &Path) -> std::io::Result<u64> {
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?;
    let mut magic = [0_u8; 4];
    file.read_exact(&mut magic)?;
    if &magic != b"RIFF" {
        return Err(std::io::Error::other("not a dictation recording"));
    }
    let length = file.metadata()?.len();
    let data = length.saturating_sub(HEADER_LEN) & !1;
    file.set_len(HEADER_LEN + data)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&header(data))?;
    file.sync_all()?;
    Ok(data / 2)
}

pub fn to_pcm16(samples: &[f32]) -> Vec<i16> {
    samples
        .iter()
        .map(|sample| (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16)
        .collect()
}

pub fn duration_ms(samples: u64) -> u64 {
    samples.saturating_mul(1_000) / u64::from(SAMPLE_RATE)
}

fn header(data_len: u64) -> [u8; 44] {
    let data_len = u32::try_from(data_len).unwrap_or(u32::MAX - 36);
    let mut header = [0_u8; 44];
    header[0..4].copy_from_slice(b"RIFF");
    header[4..8].copy_from_slice(&(36 + data_len).to_le_bytes());
    header[8..16].copy_from_slice(b"WAVEfmt ");
    header[16..20].copy_from_slice(&16_u32.to_le_bytes());
    header[20..22].copy_from_slice(&1_u16.to_le_bytes());
    header[22..24].copy_from_slice(&1_u16.to_le_bytes());
    header[24..28].copy_from_slice(&SAMPLE_RATE.to_le_bytes());
    header[28..32].copy_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    header[32..34].copy_from_slice(&2_u16.to_le_bytes());
    header[34..36].copy_from_slice(&16_u16.to_le_bytes());
    header[36..40].copy_from_slice(b"data");
    header[40..44].copy_from_slice(&data_len.to_le_bytes());
    header
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recording_cut_off_mid_write_is_repaired_into_a_readable_wav() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("audio.wav");
        let mut writer = WavWriter::create(&path).unwrap();
        writer.write(&[1, -2, 3]).unwrap();
        drop(writer);
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        file.write_all(&[9]).unwrap();

        assert_eq!(repair(&path).unwrap(), 3);
        let reader = hound::WavReader::open(&path).unwrap();
        assert_eq!(reader.spec().sample_rate, SAMPLE_RATE);
        assert_eq!(reader.spec().bits_per_sample, 16);
        let samples: Vec<i16> = reader.into_samples().map(Result::unwrap).collect();
        assert_eq!(samples, [1, -2, 3]);
    }
}
