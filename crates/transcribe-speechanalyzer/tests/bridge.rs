//! Exercises the Swift bridge end to end. Skips when the host cannot run Apple Speech
//! (pre-macOS 26, ineligible device, or assets not installed yet).

use transcribe_speechanalyzer as apple_speech;

const LOCALE: &str = "en-US";

fn sample_wav() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../aec/data/inputs/anarlog_mic.wav")
}

fn read_wav_samples(path: &std::path::Path) -> Vec<f32> {
    let bytes = std::fs::read(path).expect("read wav");
    // Minimal 16-bit PCM WAV reader: skip the 44-byte canonical header.
    bytes[44..]
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / i16::MAX as f32)
        .collect()
}

/// Assets are reserved per calling binary, so even a system-installed locale needs a
/// reservation before it reports ready. That is instant when the files already exist.
fn skip_unless_ready() -> bool {
    match apple_speech::availability() {
        Ok(value) if value.status == "available" => {}
        other => {
            eprintln!("skipping: apple speech unavailable ({other:?})");
            return true;
        }
    }

    if apple_speech::is_model_downloaded(LOCALE).unwrap_or(false) {
        return false;
    }

    if let Err(error) = apple_speech::start_model_download(LOCALE) {
        eprintln!("skipping: could not start {LOCALE} install ({error})");
        return true;
    }

    for _ in 0..120 {
        match apple_speech::model_download_state(LOCALE) {
            Ok(state) if state.status == "ready" => return false,
            Ok(state) if state.status == "error" => {
                eprintln!("skipping: {LOCALE} install failed ({:?})", state.error);
                return true;
            }
            Ok(_) => std::thread::sleep(std::time::Duration::from_millis(500)),
            Err(error) => {
                eprintln!("skipping: {LOCALE} state unavailable ({error})");
                return true;
            }
        }
    }

    eprintln!("skipping: {LOCALE} install did not finish in time");
    true
}

#[test]
fn reports_a_known_availability_status() {
    let availability = apple_speech::availability().unwrap();
    assert!(matches!(
        availability.status.as_str(),
        "available" | "unavailable" | "unsupported"
    ));
}

#[test]
fn supported_locales_include_english() {
    if apple_speech::availability().is_ok_and(|value| value.status != "available") {
        return;
    }

    let locales = apple_speech::supported_locales().unwrap();
    assert!(locales.iter().any(|locale| locale == "en-US"));
}

#[test]
fn preferred_locales_are_a_subset_of_supported_ones() {
    if apple_speech::availability().is_ok_and(|value| value.status != "available") {
        return;
    }

    let supported = apple_speech::supported_locales().unwrap();
    let preferred = apple_speech::preferred_locales().unwrap();

    for locale in &preferred {
        assert!(
            supported.contains(locale),
            "{locale} is not a supported Apple Speech locale"
        );
    }
}

#[test]
fn unsupported_languages_never_resolve_to_a_locale() {
    if apple_speech::availability().is_ok_and(|value| value.status != "available") {
        return;
    }

    // Norwegian is not transcribable, and `supportedLocale(equivalentTo:)` echoes the tag
    // back rather than returning nil, so the bridge must reject it explicitly.
    let norwegian: anlg_language::Language = "nb".parse().unwrap();
    assert_eq!(apple_speech::resolve_session_locale(&[norwegian]), None);

    let hindi: anlg_language::Language = "hi".parse().unwrap();
    assert_eq!(apple_speech::resolve_session_locale(&[hindi]), None);

    // Rejection surfaces as an error state, matching the download-state contract.
    let state = apple_speech::model_download_state("nb-NO").unwrap();
    assert_eq!(state.status, "error");
    assert!(
        state
            .error
            .is_some_and(|error| error.contains("does not support")),
        "expected an explicit unsupported-locale message"
    );
}

#[test]
fn transcribes_a_file_with_native_word_timings() {
    if skip_unless_ready() {
        return;
    }

    let transcript = apple_speech::transcribe_file(sample_wav(), LOCALE).unwrap();

    assert!(
        !transcript.text.trim().is_empty(),
        "expected transcript text"
    );
    assert!(transcript.duration_seconds > 0.0);
    assert!(!transcript.words.is_empty(), "expected native word timings");

    let first = &transcript.words[0];
    assert!(first.end >= first.start);
}

#[test]
fn transcribes_raw_samples_for_the_batch_path() {
    if skip_unless_ready() {
        return;
    }

    let samples = read_wav_samples(&sample_wav());
    let transcript = apple_speech::transcribe_samples(&samples, LOCALE).unwrap();

    assert!(
        !transcript.text.trim().is_empty(),
        "expected transcript text"
    );
    assert!(!transcript.words.is_empty(), "expected native word timings");
    assert!((transcript.duration_seconds - samples.len() as f64 / 16_000.0).abs() < 0.01);
}

#[test]
fn empty_samples_transcribe_to_nothing() {
    if skip_unless_ready() {
        return;
    }

    let transcript = apple_speech::transcribe_samples(&[], LOCALE).unwrap();

    assert!(transcript.text.is_empty());
    assert_eq!(transcript.duration_seconds, 0.0);
}

#[test]
fn live_session_emits_partials_and_finals() {
    if skip_unless_ready() {
        return;
    }

    let samples = read_wav_samples(&sample_wav());
    let mut session = apple_speech::LiveTranscriptionSession::start(LOCALE).unwrap();
    let mut partials = Vec::new();

    // 250ms at 16kHz, matching the live flush interval.
    for chunk in samples.chunks(4_000) {
        partials.extend(
            session
                .append(apple_speech::TranscriptSource::Microphone, chunk)
                .unwrap(),
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    partials.extend(
        session
            .finalize(apple_speech::TranscriptSource::Microphone)
            .unwrap(),
    );
    session.stop().unwrap();

    assert!(!partials.is_empty(), "expected live partials");
    assert!(
        partials
            .iter()
            .all(|partial| partial.source == "microphone"),
        "partials must keep their source"
    );

    let finals = partials
        .iter()
        .filter(|partial| partial.is_final)
        .collect::<Vec<_>>();
    assert!(!finals.is_empty(), "expected at least one final partial");
    assert!(
        finals.iter().any(|partial| !partial.words.is_empty()),
        "finals should carry native word timings"
    );
}
