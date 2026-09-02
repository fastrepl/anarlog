//! Scores the local diarizer against RTTM references.
//!
//! ```text
//! cargo run --release -p pyannote-local --features eval --bin diarization-eval -- \
//!     <dataset_dir> [--collar 0.25] [--limit N] [--config config.json] [--exact-speakers]
//! ```
//!
//! `dataset_dir` holds 16 kHz mono `<stem>.wav` files with a matching
//! `<stem>.rttm` (or `<stem>.json` in the pyannote.ai diarize response
//! shape). The final `DER=` line is what the autoresearch loop reads.

use std::path::{Path, PathBuf};
use std::time::Instant;

use pyannote_local::metrics::{DerReport, ReferenceTurn, diarization_error_rate};
use pyannote_local::{
    DiarizationConfig, DiarizeRequest, Diarizer, SAMPLE_RATE, SpeakerBounds, SpeakerSegment,
};

struct Args {
    dataset_dir: PathBuf,
    collar: f64,
    limit: Option<usize>,
    config: Option<PathBuf>,
    exact_speakers: bool,
    dump_dir: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = std::env::args().skip(1);
    let mut dataset_dir = None;
    let mut collar = 0.25;
    let mut limit = None;
    let mut config = None;
    let mut exact_speakers = false;
    let mut dump_dir = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--collar" => {
                collar = args
                    .next()
                    .ok_or("--collar needs a value")?
                    .parse()
                    .map_err(|e| format!("invalid collar: {e}"))?;
            }
            "--limit" => {
                limit = Some(
                    args.next()
                        .ok_or("--limit needs a value")?
                        .parse()
                        .map_err(|e| format!("invalid limit: {e}"))?,
                );
            }
            "--config" => config = Some(PathBuf::from(args.next().ok_or("--config needs a path")?)),
            "--exact-speakers" => exact_speakers = true,
            "--dump-dir" => {
                dump_dir = Some(PathBuf::from(args.next().ok_or("--dump-dir needs a path")?))
            }
            other if dataset_dir.is_none() => dataset_dir = Some(PathBuf::from(other)),
            other => return Err(format!("unexpected argument: {other}")),
        }
    }
    Ok(Args {
        dataset_dir: dataset_dir.ok_or("usage: diarization-eval <dataset_dir> [options]")?,
        collar,
        limit,
        config,
        exact_speakers,
        dump_dir,
    })
}

fn write_rttm(path: &Path, stem: &str, segments: &[SpeakerSegment]) -> Result<(), String> {
    let mut text = String::new();
    for segment in segments {
        text.push_str(&format!(
            "SPEAKER {stem} 1 {:.3} {:.3} <NA> <NA> SPEAKER_{:02} <NA> <NA>\n",
            segment.start,
            segment.end - segment.start,
            segment.speaker
        ));
    }
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

fn read_wav(path: &Path) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    if spec.channels != 1 || spec.sample_rate != SAMPLE_RATE {
        return Err(format!(
            "{}: expected 16 kHz mono, got {} Hz x{}",
            path.display(),
            spec.sample_rate,
            spec.channels
        ));
    }
    let samples: Result<Vec<f32>, _> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect(),
        hound::SampleFormat::Int => {
            let scale = (1u32 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|value| value as f32 / scale))
                .collect()
        }
    };
    samples.map_err(|e| e.to_string())
}

fn read_reference(stem: &Path) -> Result<Vec<ReferenceTurn>, String> {
    let rttm = stem.with_extension("rttm");
    if rttm.exists() {
        return parse_rttm(&std::fs::read_to_string(&rttm).map_err(|e| e.to_string())?);
    }
    let json = stem.with_extension("json");
    if json.exists() {
        return parse_pyannote_json(&std::fs::read_to_string(&json).map_err(|e| e.to_string())?);
    }
    Err(format!(
        "no .rttm or .json reference for {}",
        stem.display()
    ))
}

fn parse_rttm(text: &str) -> Result<Vec<ReferenceTurn>, String> {
    text.lines()
        .filter(|line| line.starts_with("SPEAKER"))
        .map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 8 {
                return Err(format!("malformed RTTM line: {line}"));
            }
            let start: f64 = fields[3].parse().map_err(|e| format!("{e}: {line}"))?;
            let duration: f64 = fields[4].parse().map_err(|e| format!("{e}: {line}"))?;
            Ok(ReferenceTurn {
                start,
                end: start + duration,
                speaker: fields[7].to_string(),
            })
        })
        .collect()
}

fn parse_pyannote_json(text: &str) -> Result<Vec<ReferenceTurn>, String> {
    #[derive(serde::Deserialize)]
    struct Turn {
        speaker: String,
        start: f64,
        end: f64,
    }
    #[derive(serde::Deserialize)]
    struct Response {
        diarization: Vec<Turn>,
    }
    let response: Response = serde_json::from_str(text).map_err(|e| e.to_string())?;
    Ok(response
        .diarization
        .into_iter()
        .map(|turn| ReferenceTurn {
            start: turn.start,
            end: turn.end,
            speaker: turn.speaker,
        })
        .collect())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let config = match &args.config {
        Some(path) => serde_json::from_str::<DiarizationConfig>(
            &std::fs::read_to_string(path).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("invalid config: {e}"))?,
        None => DiarizationConfig::default(),
    };
    eprintln!("config: {config:?}");

    let mut files: Vec<PathBuf> = std::fs::read_dir(&args.dataset_dir)
        .map_err(|e| format!("{}: {e}", args.dataset_dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "wav"))
        .collect();
    files.sort();
    if let Some(limit) = args.limit {
        files.truncate(limit);
    }
    if files.is_empty() {
        return Err(format!("no .wav files in {}", args.dataset_dir.display()));
    }

    let mut diarizer = Diarizer::new(config).map_err(|e| e.to_string())?;
    let mut total = DerReport::default();
    let mut audio_seconds = 0.0;
    let started = Instant::now();
    println!("file\tder\tmiss\tfa\tconf\tref_spk\thyp_spk\tseconds");
    for path in &files {
        let stem = path.with_extension("");
        let reference = read_reference(&stem)?;
        let audio = read_wav(path)?;
        audio_seconds += audio.len() as f64 / SAMPLE_RATE as f64;

        let reference_speakers = reference
            .iter()
            .map(|turn| turn.speaker.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let bounds = if args.exact_speakers {
            SpeakerBounds::exact(reference_speakers)
        } else {
            SpeakerBounds::default()
        };
        let file_started = Instant::now();
        let diarization = diarizer
            .diarize(
                &mut audio.as_slice(),
                &DiarizeRequest {
                    bounds,
                    ..DiarizeRequest::default()
                },
            )
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let report = diarization_error_rate(&reference, &diarization.segments, args.collar);
        let name = stem.file_name().unwrap_or_default().to_string_lossy();
        if let Some(dump_dir) = &args.dump_dir {
            std::fs::create_dir_all(dump_dir).map_err(|e| e.to_string())?;
            write_rttm(
                &dump_dir.join(format!("{name}.rttm")),
                &name,
                &diarization.segments,
            )?;
        }
        println!(
            "{}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{}\t{}\t{:.1}",
            name,
            report.der(),
            report.missed / report.total.max(f64::EPSILON),
            report.false_alarm / report.total.max(f64::EPSILON),
            report.confusion / report.total.max(f64::EPSILON),
            report.reference_speakers,
            report.hypothesis_speakers,
            file_started.elapsed().as_secs_f64(),
        );
        total.accumulate(&report);
    }

    let elapsed = started.elapsed().as_secs_f64();
    println!(
        "TOTAL\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{}\t{}\t{:.1}",
        total.der(),
        total.missed / total.total.max(f64::EPSILON),
        total.false_alarm / total.total.max(f64::EPSILON),
        total.confusion / total.total.max(f64::EPSILON),
        total.reference_speakers,
        total.hypothesis_speakers,
        elapsed,
    );
    println!(
        "DER={:.4} files={} audio_hours={:.2} rtf={:.3}",
        total.der(),
        files.len(),
        audio_seconds / 3600.0,
        elapsed / audio_seconds.max(f64::EPSILON),
    );
    Ok(())
}
