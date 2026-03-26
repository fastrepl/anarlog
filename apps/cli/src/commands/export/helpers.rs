pub(super) struct Segment {
    pub speaker: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

pub(super) fn build_segments(
    words: &[hypr_transcript::FinalizedWord],
    hints: &[hypr_db_app::StorageSpeakerHint],
) -> Vec<Segment> {
    use hypr_db_app::StorageSpeakerHintData;

    let hint_map: std::collections::HashMap<&str, &hypr_db_app::StorageSpeakerHint> =
        hints.iter().map(|h| (h.word_id.as_str(), h)).collect();

    let mut segments: Vec<Segment> = Vec::new();

    for word in words {
        let speaker = hint_map
            .get(word.id.as_str())
            .map(|h| match &h.data {
                StorageSpeakerHintData::UserSpeakerAssignment { human_id } => human_id.clone(),
                StorageSpeakerHintData::ProviderSpeakerIndex {
                    speaker_index,
                    channel,
                    ..
                } => {
                    let ch = channel.unwrap_or(word.channel);
                    format!("Speaker {ch}-{speaker_index}")
                }
            })
            .unwrap_or_else(|| format!("Channel {}", word.channel));

        if let Some(last) = segments.last_mut() {
            if last.speaker == speaker {
                last.text.push(' ');
                last.text.push_str(&word.text);
                last.end_ms = word.end_ms;
                continue;
            }
        }

        segments.push(Segment {
            speaker,
            start_ms: word.start_ms,
            end_ms: word.end_ms,
            text: word.text.clone(),
        });
    }

    segments
}

pub(super) fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub(super) fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().to_string() + c.as_str(),
    }
}
