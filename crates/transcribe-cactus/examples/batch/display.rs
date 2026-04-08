use colored::Colorize;
use owhisper_interface::batch;
use owhisper_interface::progress::InferenceProgress;
use owhisper_interface::stream::StreamResponse;

const LABEL: (u8, u8, u8) = (130, 170, 255);
const TRANSCRIPT: (u8, u8, u8) = (180, 255, 180);
const TIMING: (u8, u8, u8) = (255, 200, 100);
const DIM: (u8, u8, u8) = (100, 100, 100);

fn fmt_time(secs: f64) -> String {
    let m = (secs / 60.0) as u32;
    let s = secs % 60.0;
    format!("{}:{:06.3}", m, s)
}

fn fmt_elapsed(t0: std::time::Instant) -> String {
    format!("{:.3}s", t0.elapsed().as_secs_f64())
}

pub fn print_header(
    file_path: &str,
    file_size: usize,
    content_type: &str,
    model: &str,
    sse: bool,
    languages: &[String],
) {
    let sep = "─".repeat(48).truecolor(DIM.0, DIM.1, DIM.2);
    eprintln!("{}", sep);
    eprintln!(
        "{}  {} ({} bytes)",
        "file: ".truecolor(LABEL.0, LABEL.1, LABEL.2),
        file_path,
        file_size
    );
    eprintln!(
        "{}  {}",
        "type: ".truecolor(LABEL.0, LABEL.1, LABEL.2),
        content_type
    );
    eprintln!(
        "{}  {}",
        "model:".truecolor(LABEL.0, LABEL.1, LABEL.2),
        model
    );
    eprintln!(
        "{}  {}",
        "lang: ".truecolor(LABEL.0, LABEL.1, LABEL.2),
        languages.join(", ")
    );
    eprintln!(
        "{}  {}",
        "mode: ".truecolor(LABEL.0, LABEL.1, LABEL.2),
        if sse { "sse" } else { "sync" }
    );
    eprintln!("{}", sep);
    eprintln!();
}

pub fn print_result(response: &batch::Response, t0: std::time::Instant) {
    let duration = response
        .metadata
        .get("duration")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let channels = response
        .metadata
        .get("channels")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let request_id = response
        .metadata
        .get("request_id")
        .and_then(|v| v.as_str())
        .unwrap_or("-");

    eprintln!(
        "{} {}   {} {}   {} {}",
        "duration:".truecolor(LABEL.0, LABEL.1, LABEL.2),
        fmt_time(duration),
        "channels:".truecolor(LABEL.0, LABEL.1, LABEL.2),
        channels,
        "request_id:".truecolor(LABEL.0, LABEL.1, LABEL.2),
        request_id.dimmed(),
    );
    eprintln!(
        "{} {}",
        "elapsed:".truecolor(LABEL.0, LABEL.1, LABEL.2),
        fmt_elapsed(t0),
    );
    eprintln!();

    for (ch_idx, channel) in response.results.channels.iter().enumerate() {
        if response.results.channels.len() > 1 {
            eprintln!(
                "{}",
                format!("── channel {} ──", ch_idx).truecolor(DIM.0, DIM.1, DIM.2)
            );
        }

        for alt in &channel.alternatives {
            eprintln!(
                "{} {:.1}%",
                "confidence:".truecolor(LABEL.0, LABEL.1, LABEL.2),
                alt.confidence * 100.0,
            );
            eprintln!(
                "{}",
                alt.transcript
                    .truecolor(TRANSCRIPT.0, TRANSCRIPT.1, TRANSCRIPT.2)
            );
            eprintln!();

            if !alt.words.is_empty() {
                print_words(&alt.words);
            }
        }
    }
}

fn print_words(words: &[batch::Word]) {
    const MAX_WORDS: usize = 30;

    eprintln!(
        "  {:<20} {:>10} {:>10} {:>6}",
        "word".truecolor(LABEL.0, LABEL.1, LABEL.2),
        "start".truecolor(LABEL.0, LABEL.1, LABEL.2),
        "end".truecolor(LABEL.0, LABEL.1, LABEL.2),
        "conf".truecolor(LABEL.0, LABEL.1, LABEL.2),
    );
    eprintln!("  {}", "─".repeat(50).truecolor(DIM.0, DIM.1, DIM.2));

    if words.len() <= MAX_WORDS {
        for w in words {
            print_word(w);
        }
    } else {
        let half = MAX_WORDS / 2;
        for w in &words[..half] {
            print_word(w);
        }
        eprintln!(
            "  {}",
            format!("... {} words omitted ...", words.len() - MAX_WORDS)
                .truecolor(DIM.0, DIM.1, DIM.2)
        );
        for w in &words[words.len() - half..] {
            print_word(w);
        }
    }
    eprintln!();
}

fn print_word(w: &batch::Word) {
    eprintln!(
        "  {:<20} {:>10} {:>10} {:>6.2}",
        w.word,
        fmt_time(w.start).truecolor(TIMING.0, TIMING.1, TIMING.2),
        fmt_time(w.end).truecolor(TIMING.0, TIMING.1, TIMING.2),
        w.confidence,
    );
}

pub fn print_progress(progress: &InferenceProgress, t0: std::time::Instant) {
    let elapsed = fmt_elapsed(t0);
    let pct = format!("{:5.1}%", progress.percentage * 100.0);
    let phase = format!("[{:?}]", progress.phase);

    let words_info = progress
        .partial_text
        .as_ref()
        .map(|t| {
            let count = t.split_whitespace().count();
            format!("({count} words)")
        })
        .unwrap_or_default();

    eprintln!(
        "{} {} {} {}",
        elapsed.truecolor(DIM.0, DIM.1, DIM.2),
        pct.truecolor(LABEL.0, LABEL.1, LABEL.2),
        phase.dimmed(),
        words_info.truecolor(DIM.0, DIM.1, DIM.2),
    );
}

pub fn print_segment(response: &StreamResponse, t0: std::time::Instant) {
    if let StreamResponse::TranscriptResponse {
        start,
        duration,
        channel,
        ..
    } = response
    {
        let text = channel
            .alternatives
            .first()
            .map(|a| a.transcript.as_str())
            .unwrap_or("");
        let conf = channel
            .alternatives
            .first()
            .map(|a| a.confidence)
            .unwrap_or(0.0);

        let conf_str = if (conf - 1.0).abs() < 0.005 {
            String::new()
        } else {
            format!(" ({:.2})", conf)
        };

        eprintln!(
            "{} {} \"{}\"{}",
            fmt_elapsed(t0).truecolor(DIM.0, DIM.1, DIM.2),
            format!("[{} -> {}]", fmt_time(*start), fmt_time(start + duration))
                .truecolor(TIMING.0, TIMING.1, TIMING.2),
            text.truecolor(TRANSCRIPT.0, TRANSCRIPT.1, TRANSCRIPT.2),
            conf_str,
        );
    }
}

pub fn print_error(error: &str, detail: &str) {
    eprintln!("{} {}", "error:".red().bold(), error);
    eprintln!("{} {}", "detail:".red(), detail);
}
