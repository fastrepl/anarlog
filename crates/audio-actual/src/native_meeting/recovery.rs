//! Rebuilds a capture from the files its helper left in staging, for when Anarlog or the helper
//! died before the live audio was durably saved. Both CAF tracks start at frame 0 and hold silence
//! wherever no audio arrived, so they are read side by side and converted with the live
//! converter: recovered audio matches what the live stream would have delivered.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use anlg_audio::CaptureFrame;
use anlg_audio::meeting::CaptureInstant;

use super::convert::Converter;
use super::wire::{EventLine, Track};

const BLOCK_FRAMES: usize = 48_000;
/// `kCAFLinearPCMFormatFlagIsFloat | kCAFLinearPCMFormatFlagIsLittleEndian`.
const FLOAT_LITTLE_ENDIAN: u32 = 3;

/// What the journal says about a staged capture.
#[derive(Debug, Clone, PartialEq)]
pub struct StagedJournal {
    pub capture_id: Option<String>,
    /// The helper finished and padded both tracks; false after a crash or force quit.
    pub finalized: bool,
    pub end_reason: Option<String>,
    pub pauses: Vec<(CaptureInstant, Option<CaptureInstant>)>,
}

/// Reads `capture.journal`. A torn last line (a crash mid-write) is ignored.
pub fn read_journal(dir: &Path) -> io::Result<StagedJournal> {
    let mut journal = StagedJournal {
        capture_id: None,
        finalized: false,
        end_reason: None,
        pauses: Vec::new(),
    };
    for line in BufReader::new(File::open(dir.join("capture.journal"))?).lines() {
        let Some(line) = EventLine::parse(&line?) else {
            continue;
        };
        let frame = line.u64("frame").map(CaptureInstant);
        match line.event() {
            "prepared" => journal.capture_id = line.get("captureId").map(str::to_string),
            "paused" => journal.pauses.extend(frame.map(|at| (at, None))),
            "resumed" => {
                if let (Some(last), Some(at)) = (journal.pauses.last_mut(), frame) {
                    last.1.get_or_insert(at);
                }
            }
            "ended" => {
                journal.finalized = true;
                journal.end_reason = line.get("reason").map(str::to_string);
            }
            _ => {}
        }
    }
    Ok(journal)
}

/// Both staged tracks converted to `sample_rate`, in `chunk_size` frames, from frame 0 to the end
/// of the longer track.
pub fn recover_tracks(
    dir: &Path,
    sample_rate: u32,
    chunk_size: usize,
) -> io::Result<RecoveredTracks> {
    let converter = Converter::new(sample_rate, chunk_size)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error.to_string()))?;
    let mic = CafTrack::open(&dir.join("microphone.caf"))?;
    let remote = CafTrack::open(&dir.join("remote.caf"))?;
    let frames = mic.frames.max(remote.frames);
    Ok(RecoveredTracks {
        converter,
        tracks: [mic, remote],
        frames,
        position: 0,
        pending: VecDeque::new(),
        done: false,
    })
}

pub struct RecoveredTracks {
    converter: Converter,
    tracks: [CafTrack; 2],
    frames: u64,
    position: u64,
    pending: VecDeque<CaptureFrame>,
    done: bool,
}

impl RecoveredTracks {
    /// Source frames at 48 kHz.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    fn fill(&mut self) -> io::Result<()> {
        if self.position >= self.frames {
            self.pending.extend(
                self.converter
                    .finish(self.frames)
                    .into_iter()
                    .map(|c| c.frame),
            );
            self.done = true;
            return Ok(());
        }
        let count = (self.frames - self.position).min(BLOCK_FRAMES as u64) as usize;
        for (track, caf) in [Track::Microphone, Track::Remote]
            .into_iter()
            .zip(self.tracks.iter_mut())
        {
            let samples = caf.read(self.position, count)?;
            self.converter.push(track, self.position, &samples);
        }
        self.position += count as u64;
        self.converter.advance(self.position);
        let chunks = self
            .converter
            .drain()
            .map_err(|error| io::Error::other(error.to_string()))?;
        self.pending
            .extend(chunks.into_iter().map(|chunk| chunk.frame));
        Ok(())
    }
}

impl Iterator for RecoveredTracks {
    type Item = io::Result<CaptureFrame>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.pending.is_empty() && !self.done {
            if let Err(error) = self.fill() {
                self.done = true;
                return Some(Err(error));
            }
        }
        self.pending.pop_front().map(Ok)
    }
}

struct CafTrack {
    file: Option<BufReader<File>>,
    start: u64,
    frames: u64,
}

impl CafTrack {
    /// A missing track is silence. A crash leaves the data size unset (-1): the samples then run
    /// to the end of the file, and a half-written last sample is dropped.
    fn open(path: &Path) -> io::Result<Self> {
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(Self {
                    file: None,
                    start: 0,
                    frames: 0,
                });
            }
            Err(error) => return Err(error),
        };
        let size = file.metadata()?.len();
        let mut header = [0u8; 8];
        file.read_exact(&mut header)?;
        if &header[..4] != b"caff" {
            return Err(invalid("not a CAF file"));
        }
        let mut described = false;
        let mut at = 8u64;
        while at + 12 <= size {
            let mut chunk = [0u8; 12];
            file.seek(SeekFrom::Start(at))?;
            file.read_exact(&mut chunk)?;
            let length = i64::from_be_bytes(chunk[4..12].try_into().unwrap());
            let body = at + 12;
            match &chunk[..4] {
                b"desc" => {
                    let mut desc = [0u8; 32];
                    file.read_exact(&mut desc)?;
                    let rate = f64::from_be_bytes(desc[..8].try_into().unwrap());
                    let flags = u32::from_be_bytes(desc[12..16].try_into().unwrap());
                    let channels = u32::from_be_bytes(desc[24..28].try_into().unwrap());
                    let bits = u32::from_be_bytes(desc[28..32].try_into().unwrap());
                    if rate != 48_000.0
                        || &desc[8..12] != b"lpcm"
                        || flags != FLOAT_LITTLE_ENDIAN
                        || channels != 1
                        || bits != 32
                    {
                        return Err(invalid("unexpected track format"));
                    }
                    described = true;
                }
                b"data" => {
                    if !described {
                        return Err(invalid("track data before its format"));
                    }
                    // Four bytes of edit count, then the samples.
                    let start = body + 4;
                    let end = if length < 0 {
                        size
                    } else {
                        size.min(body + length as u64)
                    };
                    return Ok(Self {
                        file: Some(BufReader::new(file)),
                        start,
                        frames: end.saturating_sub(start) / 4,
                    });
                }
                _ => {}
            }
            if length < 0 {
                break;
            }
            at = body + length as u64;
        }
        Err(invalid("track has no audio"))
    }

    fn read(&mut self, frame: u64, count: usize) -> io::Result<Vec<f32>> {
        let available = self.frames.saturating_sub(frame).min(count as u64) as usize;
        let mut samples = vec![0.0; count];
        let Some(file) = self.file.as_mut() else {
            return Ok(samples);
        };
        if available == 0 {
            return Ok(samples);
        }
        file.seek(SeekFrom::Start(self.start + frame * 4))?;
        let mut bytes = vec![0u8; available * 4];
        file.read_exact(&mut bytes)?;
        for (sample, raw) in samples.iter_mut().zip(bytes.chunks_exact(4)) {
            let value = f32::from_le_bytes(raw.try_into().unwrap());
            *sample = if value.is_finite() { value } else { 0.0 };
        }
        Ok(samples)
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_meeting::wire::EventLine as Line;

    /// The layout AVAudioFile writes for the helper's tracks: a `desc`, a `free` pad, then `data`
    /// whose size stays -1 until the file is closed.
    fn caf(samples: &[f32], finalized: bool) -> Vec<u8> {
        let mut bytes = b"caff\x00\x01\x00\x00".to_vec();
        bytes.extend(b"desc");
        bytes.extend(32i64.to_be_bytes());
        bytes.extend(48_000f64.to_be_bytes());
        bytes.extend(b"lpcm");
        for value in [3u32, 4, 1, 1, 32] {
            bytes.extend(value.to_be_bytes());
        }
        bytes.extend(b"free");
        bytes.extend(16i64.to_be_bytes());
        bytes.extend([0u8; 16]);
        bytes.extend(b"data");
        let length = if finalized {
            4 + samples.len() as i64 * 4
        } else {
            -1
        };
        bytes.extend(length.to_be_bytes());
        bytes.extend(0u32.to_be_bytes());
        for sample in samples {
            bytes.extend(sample.to_le_bytes());
        }
        bytes
    }

    fn staging(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "anarlog-native-recovery-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn journal(lines: &[(&str, &[(&str, &str)])]) -> String {
        lines
            .iter()
            .map(|(event, fields)| Line::encode(event, fields) + "\n")
            .collect()
    }

    #[test]
    fn a_crashed_capture_is_rebuilt_from_its_unfinished_tracks() {
        let dir = staging("crash");
        // The microphone ran 1 s; the call 0.5 s from 0.25 s on. A torn final sample is dropped.
        let mic = vec![0.25; 48_000];
        let mut remote = vec![0.0; 12_000];
        remote.extend(vec![0.5; 24_000]);
        std::fs::write(dir.join("microphone.caf"), caf(&mic, false)).unwrap();
        let mut torn = caf(&remote, false);
        torn.extend([0x00, 0x00]);
        std::fs::write(dir.join("remote.caf"), torn).unwrap();
        std::fs::write(
            dir.join("capture.journal"),
            journal(&[
                ("prepared", &[("captureId", "capture-1")]),
                ("paused", &[("frame", "30000")]),
                ("resumed", &[("frame", "36000")]),
            ]) + "event=meeting-obs",
        )
        .unwrap();
        let staged = read_journal(&dir).unwrap();
        assert_eq!(staged.capture_id.as_deref(), Some("capture-1"));
        assert!(!staged.finalized);
        assert_eq!(
            staged.pauses,
            vec![(CaptureInstant(30_000), Some(CaptureInstant(36_000)))]
        );

        let tracks = recover_tracks(&dir, 16_000, 1_920).unwrap();
        assert_eq!(tracks.frames(), 48_000);
        let frames = tracks.collect::<io::Result<Vec<_>>>().unwrap();
        let mic: Vec<f32> = frames.iter().flat_map(|f| f.raw_mic.to_vec()).collect();
        let remote: Vec<f32> = frames.iter().flat_map(|f| f.raw_speaker.to_vec()).collect();
        assert_eq!((mic.len(), remote.len()), (16_000, 16_000));
        assert!((mic[8_000] - 0.25).abs() < 0.001);
        // The call stays where it was heard: silent before 0.25 s and after 0.75 s.
        assert!(remote[..3_900].iter().all(|s| s.abs() < 1e-6));
        assert!((remote[8_000] - 0.5).abs() < 0.001);
        assert!(remote[12_100..].iter().all(|s| s.abs() < 1e-6));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_finished_capture_reports_its_end_and_a_missing_track_is_silence() {
        let dir = staging("finished");
        std::fs::write(dir.join("microphone.caf"), caf(&[0.1; 4_800], true)).unwrap();
        std::fs::write(
            dir.join("capture.journal"),
            journal(&[
                ("prepared", &[("captureId", "capture-2")]),
                ("ended", &[("reason", "user_stopped"), ("frames", "4800")]),
            ]),
        )
        .unwrap();
        let staged = read_journal(&dir).unwrap();
        assert!(staged.finalized);
        assert_eq!(staged.end_reason.as_deref(), Some("user_stopped"));
        let frames = recover_tracks(&dir, 16_000, 1_920)
            .unwrap()
            .collect::<io::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(
            frames.iter().map(|f| f.raw_speaker.len()).sum::<usize>(),
            1_600
        );
        assert!(
            frames
                .iter()
                .all(|f| f.raw_speaker.iter().all(|s| *s == 0.0))
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_track_in_another_format_is_refused() {
        let dir = staging("format");
        let mut wrong = caf(&[0.1; 10], true);
        wrong[28..32].copy_from_slice(&2u32.to_be_bytes());
        std::fs::write(dir.join("microphone.caf"), wrong).unwrap();
        assert!(recover_tracks(&dir, 16_000, 1_920).is_err());
        std::fs::write(dir.join("microphone.caf"), b"RIFF0000").unwrap();
        assert!(recover_tracks(&dir, 16_000, 1_920).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
