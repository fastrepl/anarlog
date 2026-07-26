use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;
use hound::{SampleFormat, WavSpec, WavWriter};
use hypr_audacity::{Project, Track};
use hypr_audio_utils::{audio_file_metadata, resample_audio, source_from_path};
use vad_masking::{StreamingVad, VadConfig};

const TARGET_SAMPLE_RATE: u32 = 16_000;

#[derive(Parser)]
struct Args {
    input: PathBuf,

    #[arg(long)]
    out_dir: Option<PathBuf>,

    #[arg(long, default_value_t = 320)]
    frame_hint: usize,

    #[arg(long, default_value_t = 6)]
    hangover_frames: usize,

    #[arg(long, default_value_t = 0.0005)]
    amplitude_floor: f32,

    #[arg(long, default_value_t = true)]
    start_in_speech: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let out_dir = args.out_dir.unwrap_or_else(|| default_out_dir(&args.input));
    fs::create_dir_all(&out_dir)?;

    let original = load_mono_16khz(&args.input)?;
    let cfg = VadConfig {
        hangover_frames: args.hangover_frames,
        amplitude_floor: args.amplitude_floor,
        start_in_speech: args.start_in_speech,
    };

    let (masked, removed, spans, speech_samples, muted_samples, frame_size) =
        run_masking(&original, args.frame_hint, cfg.clone());

    let original_path = out_dir.join("original.wav");
    let masked_path = out_dir.join("masked.wav");
    let removed_path = out_dir.join("removed.wav");
    let segments_path = out_dir.join("segments.tsv");
    let summary_path = out_dir.join("summary.txt");

    write_wav(&original_path, &original)?;
    write_wav(&masked_path, &masked)?;
    write_wav(&removed_path, &removed)?;
    write_segments(&segments_path, &spans)?;
    write_summary(
        &summary_path,
        &args.input,
        original.len(),
        speech_samples,
        muted_samples,
        frame_size,
        &cfg,
    )?;
    let bundle = Project::new()
        .with_track(Track::new(&original_path).with_name("original"))
        .with_track(Track::new(&masked_path).with_name("masked"))
        .with_track(Track::new(&removed_path).with_name("removed").muted(true))
        .write_bundle(&out_dir)?;

    println!("exported {}", out_dir.display());
    println!("  {}", original_path.display());
    println!("  {}", masked_path.display());
    println!("  {}", removed_path.display());
    println!("  {}", segments_path.display());
    println!("  {}", summary_path.display());
    println!("  {}", bundle.commands_path.display());
    println!("  {}", bundle.script_path.display());
    println!();
    println!("with Audacity pipe scripting enabled:");
    println!("  python3 {}", bundle.script_path.display());

    Ok(())
}

fn default_out_dir(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("input");
    input
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{stem}-vad-masking"))
}

fn load_mono_16khz(path: &Path) -> Result<Vec<f32>, Box<dyn Error>> {
    let metadata = audio_file_metadata(path)?;
    let channels = metadata.channels as usize;
    let source = source_from_path(path)?;
    let samples = if metadata.sample_rate == TARGET_SAMPLE_RATE {
        source.collect::<Vec<_>>()
    } else {
        resample_audio(source, TARGET_SAMPLE_RATE)?
    };

    Ok(downmix_to_mono(&samples, channels))
}

fn downmix_to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }

    samples
        .chunks_exact(channels)
        .map(|frame| frame.iter().copied().sum::<f32>() / channels as f32)
        .collect()
}

fn run_masking(
    original: &[f32],
    frame_hint: usize,
    cfg: VadConfig,
) -> (
    Vec<f32>,
    Vec<f32>,
    Vec<(usize, usize, bool)>,
    usize,
    usize,
    usize,
) {
    let mut masked = original.to_vec();
    let mut removed = vec![0.0; original.len()];
    let mut spans = Vec::new();
    let mut speech_samples = 0;
    let mut muted_samples = 0;
    let mut offset = 0;
    let mut vad = StreamingVad::with_config(frame_hint, cfg);
    let frame_size = vad.frame_size();

    vad.process_in_place(&mut masked, |frame, is_speech| {
        let start = offset;
        let end = start + frame.len();
        let original_frame = &original[start..end];

        if is_speech {
            speech_samples += frame.len();
        } else {
            frame.fill(0.0);
            removed[start..end].copy_from_slice(original_frame);
            muted_samples += frame.len();
        }

        let should_extend = spans
            .last()
            .is_some_and(|(_, _, last_is_speech)| *last_is_speech == is_speech);
        if should_extend {
            if let Some((_, last_end, _)) = spans.last_mut() {
                *last_end = end;
            }
        } else {
            spans.push((start, end, is_speech));
        }

        offset = end;
    });

    (
        masked,
        removed,
        spans,
        speech_samples,
        muted_samples,
        frame_size,
    )
}

fn write_wav(path: &Path, samples: &[f32]) -> Result<(), Box<dyn Error>> {
    let spec = WavSpec {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };
    let mut writer = WavWriter::create(path, spec)?;
    for sample in samples {
        writer.write_sample(*sample)?;
    }
    writer.finalize()?;
    Ok(())
}

fn write_segments(path: &Path, spans: &[(usize, usize, bool)]) -> Result<(), Box<dyn Error>> {
    let mut body = String::from("start_sec\tend_sec\tlabel\tduration_ms\n");
    for &(start, end, is_speech) in spans {
        let start_sec = start as f64 / TARGET_SAMPLE_RATE as f64;
        let end_sec = end as f64 / TARGET_SAMPLE_RATE as f64;
        let duration_ms = (end.saturating_sub(start) as f64 / TARGET_SAMPLE_RATE as f64) * 1000.0;
        let label = if is_speech { "speech" } else { "silence" };
        body.push_str(&format!(
            "{start_sec:.3}\t{end_sec:.3}\t{label}\t{duration_ms:.1}\n"
        ));
    }
    fs::write(path, body)?;
    Ok(())
}

fn write_summary(
    path: &Path,
    input: &Path,
    total_samples: usize,
    speech_samples: usize,
    muted_samples: usize,
    frame_size: usize,
    cfg: &VadConfig,
) -> Result<(), Box<dyn Error>> {
    let total_secs = total_samples as f64 / TARGET_SAMPLE_RATE as f64;
    let kept_ratio = if total_samples == 0 {
        0.0
    } else {
        speech_samples as f64 / total_samples as f64
    };
    let muted_ratio = if total_samples == 0 {
        0.0
    } else {
        muted_samples as f64 / total_samples as f64
    };

    let body = format!(
        "input={}\noutput_sample_rate={}\nframe_size={}\nduration_sec={:.3}\nkept_samples={}\nmuted_samples={}\nkept_ratio={:.4}\nmuted_ratio={:.4}\nhangover_frames={}\namplitude_floor={:.6}\nstart_in_speech={}\n",
        input.display(),
        TARGET_SAMPLE_RATE,
        frame_size,
        total_secs,
        speech_samples,
        muted_samples,
        kept_ratio,
        muted_ratio,
        cfg.hangover_frames,
        cfg.amplitude_floor,
        cfg.start_in_speech,
    );

    fs::write(path, body)?;
    Ok(())
}
