use colored::Colorize;

fn enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("CACTUS_DEBUG").as_deref() == Ok("1"))
}

pub(super) enum Kind {
    Partial,
    Confirmed,
    Cloud,
}

struct Event<'a> {
    ch: usize,
    audio_offset: f64,
    kind: Kind,
    text: &'a str,
    seg_start: f64,
    seg_dur: f64,
    confidence: f64,
    decode_tps: f64,
    buffer_duration_ms: f64,
}

impl std::fmt::Display for Event<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let header = format!("[{:>7.2}s ch{}]", self.audio_offset, self.ch).dimmed();

        let label = match self.kind {
            Kind::Partial => " partial".yellow(),
            Kind::Confirmed => " CONFIRM".green().bold(),
            Kind::Cloud => "   cloud".cyan(),
        };

        let text = truncate(self.text, 60);

        let timing = format!(
            "seg:{:.2}\u{2192}{:.2}s  conf:{:.2}  dec:{:.0}tps  buf:{:.0}ms",
            self.seg_start,
            self.seg_start + self.seg_dur,
            self.confidence,
            self.decode_tps,
            self.buffer_duration_ms,
        )
        .dimmed();

        write!(f, "{header} {label}  \"{text}\"  {timing}")
    }
}

pub(super) fn log(
    ch: usize,
    audio_offset: f64,
    kind: Kind,
    segment: &crate::service::Segment<'_>,
    result: &hypr_cactus::StreamResult,
) {
    if !enabled() {
        return;
    }
    let event = Event {
        ch,
        audio_offset,
        kind,
        text: segment.text,
        seg_start: segment.start,
        seg_dur: segment.duration,
        confidence: segment.confidence,
        decode_tps: result.decode_tps,
        buffer_duration_ms: result.buffer_duration_ms,
    };
    eprintln!("{event}");
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let end = text
        .char_indices()
        .nth(max_chars - 1)
        .map_or(text.len(), |(i, _)| i);
    format!("{}\u{2026}", &text[..end])
}
