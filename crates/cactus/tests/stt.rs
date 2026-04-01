use std::io::Write;

use cactus::{CloudConfig, Model, TranscribeOptions, Transcriber};

fn stt_model() -> Model {
    let home = std::env::var("HOME").unwrap_or_default();
    let default = format!(
        "{}/Library/Application Support/com.hyprnote.dev/models/cactus/whisper-small-int8-apple",
        home
    );
    let path = std::env::var("CACTUS_STT_MODEL").unwrap_or(default);
    Model::new(&path).unwrap()
}

fn en_options() -> TranscribeOptions {
    TranscribeOptions {
        language: Some("en".parse().unwrap()),
        ..Default::default()
    }
}

// cargo test -p cactus --test stt test_transcribe_file -- --ignored --nocapture
#[ignore]
#[test]
fn test_transcribe_file() {
    let model = stt_model();
    let options = en_options();

    let r = model
        .transcribe_file(hypr_data::english_1::AUDIO_PATH, &options)
        .unwrap();

    assert!(!r.text.is_empty());
    println!("transcription: {:?}", r.text);
}

// cargo test -p cactus --test stt test_transcribe_file_with_callback -- --ignored --nocapture
#[ignore]
#[test]
fn test_transcribe_file_with_callback() {
    let model = stt_model();
    let options = en_options();

    let mut tokens: Vec<String> = Vec::new();
    let r = model
        .transcribe_file_with_callback(hypr_data::english_1::AUDIO_PATH, &options, |token| {
            tokens.push(token.to_string());
            print!("{}", token);
            std::io::stdout().flush().ok();
            true
        })
        .unwrap();
    println!();

    assert!(!r.text.is_empty());
    println!("transcription: {:?}", r.text);
    println!("received {} tokens via callback", tokens.len());
    assert!(!tokens.is_empty(), "expected at least one progress token");
}

// cargo test -p cactus --test stt test_transcribe_pcm -- --ignored --nocapture
#[ignore]
#[test]
fn test_transcribe_pcm() {
    let model = stt_model();
    let options = en_options();

    let r = model
        .transcribe_pcm(hypr_data::english_1::AUDIO, &options)
        .unwrap();

    assert!(!r.text.is_empty());
    println!("pcm transcription: {:?}", r.text);
}

// cargo test -p cactus --test stt test_transcribe_with_language -- --ignored --nocapture
#[ignore]
#[test]
fn test_transcribe_with_language() {
    let model = stt_model();
    let options = TranscribeOptions {
        language: Some("en".parse().unwrap()),
        temperature: Some(0.0),
        ..Default::default()
    };

    let r = model
        .transcribe_file(hypr_data::english_1::AUDIO_PATH, &options)
        .unwrap();
    assert!(!r.text.is_empty());
    println!("en transcription: {:?}", r.text);
}

// cargo test -p cactus --test stt test_stream_transcriber -- --ignored --nocapture
#[ignore]
#[test]
fn test_stream_transcriber() {
    let model = stt_model();
    let pcm = hypr_data::english_1::AUDIO;
    let options = en_options();

    let mut transcriber = Transcriber::new(&model, &options, CloudConfig::default()).unwrap();

    let chunk_size = 32000; // 1 second at 16kHz 16-bit mono
    let mut had_confirmed = false;

    for chunk in pcm.chunks(chunk_size).take(30) {
        let r = transcriber.process(chunk).unwrap();
        if !r.confirmed.is_empty() {
            had_confirmed = true;
        }
        println!("confirmed={:?} pending={:?}", r.confirmed, r.pending);
    }

    let final_result = transcriber.stop().unwrap();
    println!("final: {:?}", final_result.confirmed);

    assert!(had_confirmed, "expected at least one confirmed segment");
}

// cargo test -p cactus --test stt test_stream_transcriber_segments -- --ignored --nocapture
#[ignore]
#[test]
fn test_stream_transcriber_segments() {
    let model = stt_model();
    let pcm = hypr_data::english_1::AUDIO;
    let options = en_options();

    let mut transcriber = Transcriber::new(&model, &options, CloudConfig::default()).unwrap();

    let mut first_segmented = None;
    let mut first_confirmed_with_segments = None;

    for chunk in pcm.chunks(32000).take(30) {
        let r = transcriber.process(chunk).unwrap();

        if first_segmented.is_none() && !r.segments.is_empty() {
            first_segmented = Some(r.clone());
        }

        if first_confirmed_with_segments.is_none()
            && !r.confirmed.trim().is_empty()
            && !r.segments.is_empty()
        {
            first_confirmed_with_segments = Some(r.clone());
            break;
        }
    }

    let segmented = first_segmented.expect("expected at least one streaming result with segments");
    for segment in &segmented.segments {
        assert!(
            segment.end >= segment.start,
            "segment end ({}) should be >= start ({})",
            segment.end,
            segment.start
        );
        assert!(!segment.text.is_empty(), "segment text should not be empty");
    }

    let confirmed =
        first_confirmed_with_segments.expect("expected a confirmed streaming result with segments");
    assert!(
        !confirmed.confirmed.trim().is_empty(),
        "confirmed text should not be empty"
    );
    assert!(
        confirmed.buffer_duration_ms > 0.0,
        "buffer_duration_ms should be positive, got {}",
        confirmed.buffer_duration_ms
    );

    let first_segment = confirmed.segments.first().unwrap();
    assert!(
        first_segment.start >= 0.0,
        "first segment start should be non-negative, got {}",
        first_segment.start
    );

    let last_segment = confirmed.segments.last().unwrap();
    let last_segment_end = last_segment.end as f64;
    assert!(
        last_segment_end > 0.0,
        "expected positive segment end timestamp, got {last_segment_end}"
    );
    assert!(
        (last_segment_end - confirmed.buffer_duration_ms / 1000.0).abs() < 0.25,
        "last segment end ({last_segment_end}) should roughly match buffer duration ({}s)",
        confirmed.buffer_duration_ms / 1000.0
    );

    for pair in confirmed.segments.windows(2) {
        assert!(
            pair[1].start >= pair[0].start,
            "segments should be ordered: {} came after {}",
            pair[0].start,
            pair[1].start
        );
    }

    let _ = transcriber.stop().unwrap();
}

// cargo test -p cactus --test stt test_stream_transcriber_drop -- --ignored --nocapture
#[ignore]
#[test]
fn test_stream_transcriber_drop() {
    let model = stt_model();
    let options = en_options();

    {
        let mut transcriber = Transcriber::new(&model, &options, CloudConfig::default()).unwrap();
        let silence = vec![0u8; 32000];
        let _ = transcriber.process(&silence);
    }
}

// cargo test -p cactus --test stt test_stream_transcriber_process_samples -- --ignored --nocapture
#[ignore]
#[test]
fn test_stream_transcriber_process_samples() {
    let model = stt_model();
    let options = en_options();
    let mut transcriber = Transcriber::new(&model, &options, CloudConfig::default()).unwrap();

    let samples = vec![0i16; 16000];
    let r = transcriber.process_samples(&samples).unwrap();
    println!(
        "silence result: confirmed={:?} pending={:?}",
        r.confirmed, r.pending
    );

    let _ = transcriber.stop().unwrap();
}

// cargo test -p cactus --test stt test_stream_transcriber_process_f32 -- --ignored --nocapture
#[ignore]
#[test]
fn test_stream_transcriber_process_f32() {
    let model = stt_model();
    let options = en_options();
    let mut transcriber = Transcriber::new(&model, &options, CloudConfig::default()).unwrap();

    let samples = vec![0.0f32; 16000];
    let r = transcriber.process_f32(&samples).unwrap();
    println!(
        "f32 silence result: confirmed={:?} pending={:?}",
        r.confirmed, r.pending
    );

    let _ = transcriber.stop().unwrap();
}
