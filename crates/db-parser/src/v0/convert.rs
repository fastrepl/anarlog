use crate::types::*;

pub(super) fn session_to_transcript(session: &hypr_db_user::Session) -> Transcript {
    let record_start_ms = session
        .record_start
        .map(|dt| dt.timestamp_millis() as u64)
        .or_else(|| session.words.first().and_then(|w| w.start_ms));

    let texts_with_spacing = fix_spacing_for_words(
        session
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>(),
    );

    let words: Vec<Word> = session
        .words
        .iter()
        .enumerate()
        .map(|(idx, word)| {
            let speaker = get_speaker_label(&word.speaker);
            let relative_start_ms = compute_relative_ms(word.start_ms, record_start_ms);
            let relative_end_ms = compute_relative_ms(word.end_ms, record_start_ms);

            Word {
                id: format!("{}-{}", session.id, idx),
                text: texts_with_spacing[idx].clone(),
                start_ms: relative_start_ms,
                end_ms: relative_end_ms,
                channel: 0,
                speaker: Some(speaker),
            }
        })
        .collect();

    let started_at = session
        .record_start
        .map(|dt| dt.timestamp_millis() as f64)
        .unwrap_or_default();
    let ended_at = session.record_end.map(|dt| dt.timestamp_millis() as f64);

    let start_ms = words.first().and_then(|w| w.start_ms);
    let end_ms = words.last().and_then(|w| w.end_ms);

    Transcript {
        id: session.id.clone(),
        user_id: String::new(),
        created_at: session.created_at.to_rfc3339(),
        session_id: session.id.clone(),
        title: session.title.clone(),
        started_at,
        ended_at,
        start_ms,
        end_ms,
        words,
        speaker_hints: vec![],
    }
}

fn get_speaker_label(speaker: &Option<owhisper_interface::SpeakerIdentity>) -> String {
    match speaker {
        Some(owhisper_interface::SpeakerIdentity::Assigned { label, .. }) => label.clone(),
        Some(owhisper_interface::SpeakerIdentity::Unassigned { index }) => {
            format!("Speaker {}", index)
        }
        None => "Unknown".to_string(),
    }
}

fn compute_relative_ms(absolute_ms: Option<u64>, base_ms: Option<u64>) -> Option<f64> {
    match (absolute_ms, base_ms) {
        (Some(abs), Some(base)) => Some(abs.saturating_sub(base) as f64),
        _ => None,
    }
}

pub(super) fn html_to_markdown(html: &str) -> String {
    htmd::convert(html).unwrap_or_else(|_| html.to_string())
}

fn fix_spacing_for_words(words: Vec<&str>) -> Vec<String> {
    words
        .iter()
        .map(|word| {
            let trimmed = word.trim();
            if trimmed.is_empty() {
                return word.to_string();
            }

            if word.starts_with(' ') {
                return word.to_string();
            }

            if should_skip_leading_space(trimmed) {
                return trimmed.to_string();
            }

            format!(" {}", trimmed)
        })
        .collect()
}

fn should_skip_leading_space(word: &str) -> bool {
    match word.chars().next() {
        None => true,
        Some(c) => {
            matches!(
                c,
                '\'' | '\u{2019}'
                    | ','
                    | '.'
                    | '!'
                    | '?'
                    | ':'
                    | ';'
                    | ')'
                    | ']'
                    | '}'
                    | '"'
                    | '\u{201D}'
            )
        }
    }
}
