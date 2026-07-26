use segmentation::Segmenter;
use serde::Serialize;
use std::{fs, path::PathBuf};

#[derive(Debug, Serialize)]
struct SegmentEntry {
    index: usize,
    start_sec: f64,
    end_sec: f64,
    duration_sec: f64,
    sample_start: usize,
    sample_end: usize,
    wav_path: String,
}

#[derive(Debug, Serialize)]
struct SegmentExport {
    sample_rate: u32,
    total_samples: usize,
    total_duration_sec: f64,
    segment_count: usize,
    segments: Vec<SegmentEntry>,
}

fn pcm_bytes_to_i16(bytes: &[u8]) -> Vec<i16> {
    bytes
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sample_rate = 16_000u32;
    let samples = pcm_bytes_to_i16(hypr_data::english_1::AUDIO);

    let mut segmenter = Segmenter::new(sample_rate)?;
    let segments = segmenter.process(&samples, sample_rate)?;

    let output_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/outputs");
    fs::create_dir_all(&output_dir)?;

    let mut entries = Vec::with_capacity(segments.len());

    for (index, segment) in segments.iter().enumerate() {
        let file_name = format!("english_1_segment_{index:03}.wav");
        let wav_path = output_dir.join(&file_name);

        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = hound::WavWriter::create(&wav_path, spec)?;
        for sample in &samples[segment.sample_start..segment.sample_end] {
            writer.write_sample(*sample)?;
        }
        writer.finalize()?;

        entries.push(SegmentEntry {
            index,
            start_sec: segment.start,
            end_sec: segment.end,
            duration_sec: segment.end - segment.start,
            sample_start: segment.sample_start,
            sample_end: segment.sample_end,
            wav_path: format!("outputs/{file_name}"),
        });
    }

    let export = SegmentExport {
        sample_rate,
        total_samples: samples.len(),
        total_duration_sec: samples.len() as f64 / sample_rate as f64,
        segment_count: entries.len(),
        segments: entries,
    };

    let json_path = output_dir.join("english_1_segments.json");
    fs::write(&json_path, serde_json::to_string_pretty(&export)?)?;

    println!(
        "Exported {} segments to {}",
        export.segment_count,
        output_dir.display()
    );

    Ok(())
}
