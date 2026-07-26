use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use clap::{Parser, ValueEnum};
use hound::{SampleFormat, WavSpec, WavWriter};
use hypr_audacity::{Project, Track};
use hypr_audio_utils::{audio_file_metadata, resample_audio, source_from_path};

const TARGET_SAMPLE_RATE: u32 = 16_000;

#[cfg(not(feature = "onnx"))]
fn main() -> Result<(), Box<dyn Error>> {
    Err("the audacity example requires the `onnx` feature".into())
}

#[cfg(feature = "onnx")]
use aec::{AEC, BLOCK_SIZE};

#[cfg(feature = "onnx")]
#[derive(Clone, ValueEnum)]
enum Mode {
    Batch,
    Streaming,
}

#[cfg(feature = "onnx")]
#[derive(Parser)]
struct Args {
    mic: PathBuf,
    lpb: PathBuf,

    #[arg(long)]
    out_dir: Option<PathBuf>,

    #[arg(long, value_enum, default_value = "streaming")]
    mode: Mode,

    #[arg(long, default_value_t = BLOCK_SIZE * 2)]
    chunk_size: usize,
}

#[cfg(feature = "onnx")]
fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let out_dir = args
        .out_dir
        .unwrap_or_else(|| default_out_dir(&args.mic, &args.mode));
    fs::create_dir_all(&out_dir)?;

    let mic = load_mono_16khz(&args.mic)?;
    let lpb = load_mono_16khz(&args.lpb)?;
    let len_audio = mic.len().min(lpb.len());
    let mic = mic[..len_audio].to_vec();
    let lpb = lpb[..len_audio].to_vec();

    let processed = run_aec(&mic, &lpb, &args.mode, args.chunk_size)?;
    let removed = subtract(&mic, &processed);

    let mic_path = out_dir.join("mic_input.wav");
    let lpb_path = out_dir.join("speaker_reference.wav");
    let aec_path = out_dir.join("aec_output.wav");
    let removed_path = out_dir.join("cancelled_from_mic.wav");
    let summary_path = out_dir.join("summary.txt");

    write_wav(&mic_path, &mic)?;
    write_wav(&lpb_path, &lpb)?;
    write_wav(&aec_path, &processed)?;
    write_wav(&removed_path, &removed)?;
    write_summary(
        &summary_path,
        &args.mic,
        &args.lpb,
        &args.mode,
        args.chunk_size,
        len_audio,
        &mic,
        &processed,
        &removed,
    )?;

    let bundle = Project::new()
        .with_track(Track::new(&mic_path).with_name("mic_input"))
        .with_track(Track::new(&aec_path).with_name("aec_output"))
        .with_track(
            Track::new(&removed_path)
                .with_name("cancelled_from_mic")
                .muted(true),
        )
        .with_track(
            Track::new(&lpb_path)
                .with_name("speaker_reference")
                .muted(true),
        )
        .write_bundle(&out_dir)?;

    println!("exported {}", out_dir.display());
    println!("  {}", mic_path.display());
    println!("  {}", lpb_path.display());
    println!("  {}", aec_path.display());
    println!("  {}", removed_path.display());
    println!("  {}", summary_path.display());
    println!("  {}", bundle.commands_path.display());
    println!("  {}", bundle.script_path.display());
    println!();
    println!("with Audacity pipe scripting enabled:");
    println!("  python3 {}", bundle.script_path.display());

    Ok(())
}

#[cfg(feature = "onnx")]
fn default_out_dir(mic: &Path, mode: &Mode) -> PathBuf {
    let stem = mic
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("mic");
    let mode = match mode {
        Mode::Batch => "batch",
        Mode::Streaming => "streaming",
    };
    mic.parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{stem}-aec-{mode}"))
}

#[cfg(feature = "onnx")]
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

#[cfg(feature = "onnx")]
fn downmix_to_mono(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }

    samples
        .chunks_exact(channels)
        .map(|frame| frame.iter().copied().sum::<f32>() / channels as f32)
        .collect()
}

#[cfg(feature = "onnx")]
fn run_aec(
    mic: &[f32],
    lpb: &[f32],
    mode: &Mode,
    chunk_size: usize,
) -> Result<Vec<f32>, Box<dyn Error>> {
    let mut aec = AEC::new()?;

    match mode {
        Mode::Batch => Ok(aec.process(mic, lpb)?),
        Mode::Streaming => {
            let mut output = Vec::with_capacity(mic.len());
            let chunk_size = chunk_size.max(1);
            let mut processed = 0;

            while processed < mic.len() {
                let end = (processed + chunk_size).min(mic.len());
                output.extend(aec.process_streaming(&mic[processed..end], &lpb[processed..end])?);
                processed = end;
            }

            Ok(output)
        }
    }
}

#[cfg(feature = "onnx")]
fn subtract(input: &[f32], output: &[f32]) -> Vec<f32> {
    input
        .iter()
        .zip(output.iter())
        .map(|(input, output)| (input - output).clamp(-1.0, 1.0))
        .collect()
}

#[cfg(feature = "onnx")]
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

#[cfg(feature = "onnx")]
fn write_summary(
    path: &Path,
    mic_path: &Path,
    lpb_path: &Path,
    mode: &Mode,
    chunk_size: usize,
    total_samples: usize,
    mic: &[f32],
    processed: &[f32],
    removed: &[f32],
) -> Result<(), Box<dyn Error>> {
    let body = format!(
        "mic={}\nlpb={}\nmode={}\nchunk_size={}\nsample_rate={}\nduration_sec={:.3}\nmic_rms={:.6}\naec_rms={:.6}\nremoved_rms={:.6}\n",
        mic_path.display(),
        lpb_path.display(),
        match mode {
            Mode::Batch => "batch",
            Mode::Streaming => "streaming",
        },
        chunk_size,
        TARGET_SAMPLE_RATE,
        total_samples as f64 / TARGET_SAMPLE_RATE as f64,
        rms(mic),
        rms(processed),
        rms(removed),
    );

    fs::write(path, body)?;
    Ok(())
}

#[cfg(feature = "onnx")]
fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|sample| sample * sample).sum();
    (sum_sq / samples.len() as f32).sqrt()
}
