use crate::types::TranscriptSegment;

pub fn parse_vtt(content: &str) -> Vec<TranscriptSegment> {
    let body = content
        .trim_start()
        .strip_prefix("WEBVTT")
        .map(|rest| rest.trim_start_matches(['\u{feff}', ' ', '\t']))
        .map(|rest| rest.trim_start_matches(['\r', '\n']))
        .unwrap_or(content);

    body.split("\n\n")
        .flat_map(|block| block.split("\r\n\r\n"))
        .filter_map(parse_vtt_block)
        .collect()
}

fn parse_vtt_block(block: &str) -> Option<TranscriptSegment> {
    let lines = block
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let timing_index = lines.iter().position(|line| line.contains("-->"))?;
    let mut parts = lines[timing_index].split("-->");
    let start = parse_timestamp(parts.next()?.trim())?;
    let end = parse_timestamp(parts.next()?.split_whitespace().next().unwrap_or(""))?;
    let raw_text = lines[(timing_index + 1)..].join(" ");
    if raw_text.is_empty() {
        return None;
    }

    let (speaker, text) = if let Some(rest) = raw_text.strip_prefix("<v ") {
        let (speaker, text) = rest.split_once('>')?;
        (speaker.trim().to_string(), strip_caption_markup(text))
    } else if let Some((speaker, text)) = raw_text.split_once(": ")
        && speaker.len() <= 60
        && !speaker.contains('>')
    {
        (speaker.trim().to_string(), strip_caption_markup(text))
    } else {
        (String::new(), strip_caption_markup(&raw_text))
    };

    if text.is_empty() {
        return None;
    }

    Some(TranscriptSegment {
        speaker,
        text,
        start_ms: start,
        end_ms: end,
    })
}

fn strip_caption_markup(value: &str) -> String {
    let mut text = String::new();
    let mut chars = value.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '<' {
            for next in chars.by_ref() {
                if next == '>' {
                    break;
                }
            }
        } else {
            text.push(character);
        }
    }
    text.trim().to_string()
}

fn parse_timestamp(value: &str) -> Option<u64> {
    let (time, fraction) = match value.split_once('.') {
        Some((time, fraction)) => (time, fraction),
        None => value.split_once(',').unwrap_or((value, "0")),
    };
    let parts = time
        .split(':')
        .map(|part| part.parse::<u64>().ok())
        .collect::<Option<Vec<_>>>()?;
    let (hours, minutes, seconds) = match parts.as_slice() {
        [minutes, seconds] => (0, *minutes, *seconds),
        [hours, minutes, seconds] => (*hours, *minutes, *seconds),
        _ => return None,
    };
    let millis = fraction
        .chars()
        .take(3)
        .collect::<String>()
        .parse::<u64>()
        .unwrap_or(0);
    Some(
        hours
            .saturating_mul(3_600_000)
            .saturating_add(minutes.saturating_mul(60_000))
            .saturating_add(seconds.saturating_mul(1_000))
            .saturating_add(millis),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_speaker_labeled_vtt() {
        let segments = parse_vtt(
            "WEBVTT\n\n00:00:01.000 --> 00:00:04.000\nAda: Let's ship it.\n\n00:00:04.500 --> 00:00:08.000\n<v Sam>Sounds good.\n",
        );

        assert_eq!(
            segments,
            vec![
                TranscriptSegment {
                    speaker: "Ada".into(),
                    text: "Let's ship it.".into(),
                    start_ms: 1_000,
                    end_ms: 4_000,
                },
                TranscriptSegment {
                    speaker: "Sam".into(),
                    text: "Sounds good.".into(),
                    start_ms: 4_500,
                    end_ms: 8_000,
                },
            ]
        );
    }
}
