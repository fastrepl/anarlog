use crate::json::{ImportedMeeting, nonempty};
use crate::time::hhmmss_to_ms;
use anlg_zoom::TranscriptSegment;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedFile {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileDetails {
    pub id: String,
    pub name: String,
    pub created_at: Option<String>,
    pub start_at: Option<String>,
    pub transcript_available: bool,
    pub summary_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Account {
    pub id: Option<String>,
    pub email: Option<String>,
    pub name: Option<String>,
}

impl Account {
    pub fn display_name(&self) -> String {
        nonempty(self.email.as_deref())
            .or_else(|| nonempty(self.name.as_deref()))
            .or_else(|| nonempty(self.id.as_deref()))
            .unwrap_or_else(|| "plaud".to_string())
    }
}

pub fn strip_ansi(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        output.push(character);
    }
    output
}

pub fn parse_login_url(output: &str) -> Option<String> {
    strip_ansi(output).lines().find_map(|line| {
        let trimmed = line.trim();
        let url = trimmed
            .split_whitespace()
            .find(|token| token.starts_with("https://") || token.starts_with("http://"))?;
        let url = url.trim_end_matches(['.', ',', ')', ']']);
        if url.contains("plaud.ai") {
            Some(url.to_string())
        } else {
            None
        }
    })
}

pub fn parse_me(output: &str) -> Account {
    let fields = parse_labeled_fields(output);
    Account {
        id: nonempty(fields.get("id").map(String::as_str)),
        email: nonempty(fields.get("email").map(String::as_str)),
        name: nonempty(fields.get("name").map(String::as_str))
            .or_else(|| nonempty(fields.get("nickname").map(String::as_str))),
    }
}

pub fn parse_files_table(output: &str) -> Vec<ListedFile> {
    strip_ansi(output)
        .lines()
        .filter_map(parse_files_row)
        .collect()
}

pub fn parse_file_details(output: &str) -> FileDetails {
    let fields = parse_labeled_fields(output);
    FileDetails {
        id: nonempty(fields.get("id").map(String::as_str)).unwrap_or_default(),
        name: nonempty(fields.get("name").map(String::as_str)).unwrap_or_default(),
        created_at: nonempty(fields.get("created_at").map(String::as_str)),
        start_at: nonempty(fields.get("start_at").map(String::as_str)).filter(|value| value != "-"),
        transcript_available: is_available(fields.get("transcript").map(String::as_str)),
        summary_available: is_available(fields.get("summary").map(String::as_str)),
    }
}

pub fn parse_transcript(output: &str) -> Vec<TranscriptSegment> {
    let text = strip_ansi(output);
    if is_unavailable_message(&text, "transcript") {
        return Vec::new();
    }

    text.lines()
        .filter_map(|line| parse_transcript_line(line.trim()))
        .collect()
}

pub fn parse_summary(output: &str) -> Option<String> {
    let text = strip_ansi(output);
    if is_unavailable_message(&text, "summary") || is_unavailable_message(&text, "note") {
        return None;
    }

    let mut lines = text.lines().peekable();
    while let Some(line) = lines.peek() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("Summary:") || trimmed.starts_with("Notes:") {
            lines.next();
            continue;
        }
        break;
    }

    let body = lines.collect::<Vec<_>>().join("\n");
    nonempty(Some(body.trim_end()))
}

pub fn parse_action_items(markdown: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut in_action_section = false;
    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some(heading) = heading_text(trimmed) {
            in_action_section = heading.to_ascii_lowercase().contains("action");
            continue;
        }
        let Some(item) = list_item_text(trimmed) else {
            continue;
        };
        if in_action_section || checkbox_item(trimmed) {
            items.push(item);
        }
    }
    items
}

pub fn meeting_from_cli(
    details: &FileDetails,
    transcript: Vec<TranscriptSegment>,
    summary: Option<String>,
) -> ImportedMeeting {
    let action_items = summary
        .as_deref()
        .map(parse_action_items)
        .unwrap_or_default();
    ImportedMeeting {
        id: details.id.clone(),
        title: if details.name.trim().is_empty() {
            details.id.clone()
        } else {
            details.name.clone()
        },
        start_time: details
            .start_at
            .clone()
            .or_else(|| details.created_at.clone()),
        url: None,
        summary,
        notes: None,
        transcript,
        action_items,
    }
}

fn parse_labeled_fields(output: &str) -> std::collections::HashMap<String, String> {
    strip_ansi(output)
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let (key, value) = trimmed.split_once(':')?;
            let key = key.trim().to_ascii_lowercase();
            if key.is_empty() || key.contains(' ') {
                return None;
            }
            Some((key, value.trim().to_string()))
        })
        .collect()
}

fn parse_files_row(line: &str) -> Option<ListedFile> {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("Files on this page")
        || trimmed.starts_with("Page ")
        || trimmed.chars().all(|character| {
            character == '─' || character == '-' || character == ' ' || character == '━'
        })
    {
        return None;
    }

    let id = trimmed.split_whitespace().next()?.to_string();
    if !is_file_id(&id) {
        return None;
    }

    let rest = trimmed[id.len()..].trim_start();
    let mut parts = rest.split_whitespace().collect::<Vec<_>>();
    if parts.len() >= 2 {
        let date = parts[parts.len() - 2];
        if date == "-" || is_iso_date(date) {
            parts.truncate(parts.len() - 2);
            return Some(ListedFile {
                id,
                name: parts.join(" ").trim_end_matches('…').trim().to_string(),
            });
        }
    }

    Some(ListedFile {
        id,
        name: rest.trim_end_matches('…').trim().to_string(),
    })
}

fn parse_transcript_line(line: &str) -> Option<TranscriptSegment> {
    let (times, rest) = line.strip_prefix('[')?.split_once(']')?;
    let (start, end) = times.split_once('-')?;
    let start_ms = hhmmss_to_ms(start.trim());
    let end_ms = hhmmss_to_ms(end.trim());
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }
    let (speaker, text) = match rest.split_once(": ") {
        Some((speaker, text)) if speaker.len() <= 80 && !speaker.contains('[') => {
            (speaker.trim().to_string(), text.trim().to_string())
        }
        _ => (String::new(), rest.to_string()),
    };
    nonempty(Some(&text)).map(|text| TranscriptSegment {
        speaker,
        text,
        start_ms,
        end_ms: if end_ms > start_ms {
            end_ms
        } else {
            start_ms.saturating_add(1_000)
        },
    })
}

fn heading_text(line: &str) -> Option<&str> {
    let stripped = line.trim_start_matches('#').trim();
    if stripped.len() < line.len() && !stripped.is_empty() {
        Some(stripped)
    } else {
        None
    }
}

fn list_item_text(line: &str) -> Option<String> {
    let content = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .or_else(|| {
            let (number, rest) = line.split_once('.')?;
            if number.chars().all(|character| character.is_ascii_digit()) {
                Some(rest.trim_start())
            } else {
                None
            }
        })?;
    let text = content
        .trim()
        .trim_start_matches("[ ]")
        .trim_start_matches("[x]")
        .trim_start_matches("[X]")
        .trim();
    nonempty(Some(text))
}

fn checkbox_item(line: &str) -> bool {
    let trimmed = line.trim_start_matches(['-', '*']).trim_start();
    trimmed.starts_with("[ ]") || trimmed.starts_with("[x]") || trimmed.starts_with("[X]")
}

fn is_available(value: Option<&str>) -> bool {
    value.is_some_and(|value| value.to_ascii_lowercase().starts_with("available"))
}

fn is_unavailable_message(text: &str, _kind: &str) -> bool {
    let lowered = text.to_ascii_lowercase();
    lowered.contains("not available")
        || lowered.contains("hasn't been generated")
        || lowered.contains("no \"")
}

fn is_file_id(value: &str) -> bool {
    let len = value.len();
    (8..=64).contains(&len)
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

fn is_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[8..10].iter().all(u8::is_ascii_digit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_ansi_and_reads_login_url() {
        let output = "\u{1b}[33mCould not open browser. Open this URL manually:\n  https://web.plaud.ai/platform/oauth?state=abc\u{1b}[0m";
        assert_eq!(
            parse_login_url(output).as_deref(),
            Some("https://web.plaud.ai/platform/oauth?state=abc")
        );
    }

    #[test]
    fn parses_account_fields() {
        let output = "\nUser Info:\n\n  id: user-1\n  email: ada@example.com\n  name: Ada\n";
        assert_eq!(
            parse_me(output),
            Account {
                id: Some("user-1".into()),
                email: Some("ada@example.com".into()),
                name: Some("Ada".into()),
            }
        );
    }

    #[test]
    fn parses_files_table_rows() {
        let id = "abcdef1234567890abcdef1234567890ab";
        let output = format!(
            "Files on this page: 2\n\n  {:<34}  {:<36}  {:<12}  DURATION\n  {}\n  {id}  Weekly standup                      2026-08-01    32m10s\n  shorterid                         Catch-up                             2026-07-30    1h05m\n\nPage 1\n",
            "ID",
            "NAME",
            "DATE",
            "─".repeat(90),
        );
        let files = parse_files_table(&output);
        assert_eq!(
            files,
            vec![
                ListedFile {
                    id: id.into(),
                    name: "Weekly standup".into(),
                },
                ListedFile {
                    id: "shorterid".into(),
                    name: "Catch-up".into(),
                },
            ]
        );
    }

    #[test]
    fn parses_file_details_and_availability() {
        let output = "
File Details:

  id:           rec-1
  name:         Weekly standup
  created_at:   2026-08-01T10:00:00Z
  start_at:     2026-08-01T09:59:00Z
  duration:     32m10s
  serial_number: -
  audio:        available
  transcript:   available
  summary:      unavailable
";
        assert_eq!(
            parse_file_details(output),
            FileDetails {
                id: "rec-1".into(),
                name: "Weekly standup".into(),
                created_at: Some("2026-08-01T10:00:00Z".into()),
                start_at: Some("2026-08-01T09:59:00Z".into()),
                transcript_available: true,
                summary_available: false,
            }
        );
    }

    #[test]
    fn parses_timestamped_transcript() {
        let output = "
Transcript: Weekly standup

[00:01 - 00:04] Ada: Let's ship it.
[00:04 - 00:08] Tom: Agreed.
";
        let transcript = parse_transcript(output);
        assert_eq!(transcript.len(), 2);
        assert_eq!(transcript[0].speaker, "Ada");
        assert_eq!(transcript[0].text, "Let's ship it.");
        assert_eq!(transcript[0].start_ms, 1_000);
        assert_eq!(transcript[0].end_ms, 4_000);
        assert_eq!(transcript[1].speaker, "Tom");
    }

    #[test]
    fn ignores_missing_transcript() {
        assert!(
            parse_transcript(
                "No \"transaction\" transcript for this recording. Available: (none)."
            )
            .is_empty()
        );
    }

    #[test]
    fn parses_summary_and_action_items() {
        let output = "
Summary: Weekly standup

## Overview
We agreed to ship.

## Action items
- Prepare the release
- [ ] Ping design
";
        let summary = parse_summary(output).unwrap();
        assert!(summary.contains("We agreed to ship."));
        assert_eq!(
            parse_action_items(&summary),
            vec!["Prepare the release", "Ping design"]
        );
    }

    #[test]
    fn builds_importable_meeting() {
        let details = FileDetails {
            id: "rec-1".into(),
            name: "Weekly standup".into(),
            created_at: Some("2026-08-01T10:00:00Z".into()),
            start_at: None,
            transcript_available: true,
            summary_available: true,
        };
        let meeting = meeting_from_cli(
            &details,
            vec![TranscriptSegment {
                speaker: "Ada".into(),
                text: "Let's ship it.".into(),
                start_ms: 1_000,
                end_ms: 4_000,
            }],
            Some("## Action items\n- Prepare the release".into()),
        );
        assert_eq!(meeting.title, "Weekly standup");
        assert_eq!(meeting.start_time.as_deref(), Some("2026-08-01T10:00:00Z"));
        assert_eq!(meeting.action_items, vec!["Prepare the release"]);
    }
}
