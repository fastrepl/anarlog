use std::{io::Write, path::Path};

use desktop_runtime::Result;
use serde_json::Value;

use super::{data::CanonicalMeeting, failure};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    Pdf,
    Markdown,
    Text,
    Org,
    Canonical,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Markdown => "md",
            Self::Text => "txt",
            Self::Org => "org",
            Self::Canonical => "json",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExportOptions {
    pub format: Format,
    pub memo: bool,
    pub summary: bool,
    pub transcript: bool,
    pub summary_id: Option<String>,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: Format::Pdf,
            memo: false,
            summary: true,
            transcript: false,
            summary_id: None,
        }
    }
}

pub fn export(meeting: &CanonicalMeeting, options: &ExportOptions, path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| failure("Export path has no parent"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(failure)?;
    if options.format == Format::Canonical {
        serde_json::to_writer_pretty(&mut temporary, meeting).map_err(failure)?;
    } else {
        let input = content(meeting, options)?;
        match options.format {
            Format::Pdf => {
                anlg_export_core::export_pdf(temporary.path(), input).map_err(failure)?
            }
            format => {
                let text = render_text(&input, format);
                temporary.write_all(text.as_bytes()).map_err(failure)?;
            }
        }
    }
    temporary.as_file().sync_all().map_err(failure)?;
    temporary.persist(path).map_err(failure)?;
    Ok(())
}

pub fn content(
    meeting: &CanonicalMeeting,
    options: &ExportOptions,
) -> Result<anlg_export_core::ExportInput> {
    let mut memo = String::new();
    let mut summary = String::new();
    for document in &meeting.documents {
        let is_note = document["kind"].as_str().unwrap_or("note") == "note";
        if (is_note && !options.memo) || (!is_note && !options.summary) {
            continue;
        }
        if !is_note
            && options
                .summary_id
                .as_ref()
                .is_some_and(|id| document["id"].as_str() != Some(id))
        {
            continue;
        }
        if document["body_format"] != "prosemirror_json" {
            return Err(failure(
                "Document format cannot be rendered. Use canonical JSON to preserve it.",
            ));
        }
        let value: Value = serde_json::from_str(
            document["body"]
                .as_str()
                .ok_or_else(|| failure("Document lacks body"))?,
        )
        .map_err(failure)?;
        ensure_renderable(&value)?;
        let markdown = anlg_tiptap::tiptap_json_to_md(&value).map_err(failure)?;
        let output = if is_note { &mut memo } else { &mut summary };
        if !output.is_empty() {
            output.push_str("\n\n");
        }
        output.push_str(&markdown);
    }
    let items = if options.transcript {
        super::transcript::render(meeting)?
    } else {
        Vec::new()
    };
    let mut first = None;
    let mut last = None;
    for transcript in &meeting.transcripts {
        if let Some(start) = transcript["started_at_ms"].as_i64() {
            first = Some(first.map_or(start, |value: i64| value.min(start)));
        }
        if let Some(end) = transcript["ended_at_ms"].as_i64() {
            last = Some(last.map_or(end, |value: i64| value.max(end)));
        }
    }
    Ok(anlg_export_core::ExportInput {
        enhanced_md: summary,
        memo_md: options.memo.then_some(memo),
        transcript: options
            .transcript
            .then_some(anlg_export_core::Transcript { items }),
        metadata: Some(anlg_export_core::ExportMetadata {
            title: meeting.title().into(),
            created_at: formatted_date(meeting.session["created_at"].as_str().unwrap_or("")),
            participants: meeting
                .participants
                .iter()
                .filter_map(|participant| {
                    let human = participant["human_id"].as_str().and_then(|id| {
                        meeting
                            .humans
                            .iter()
                            .find(|human| human["id"].as_str() == Some(id))
                    });
                    human
                        .and_then(|human| human["name"].as_str())
                        .filter(|name| !name.is_empty())
                        .or_else(|| participant["display_name"].as_str())
                        .filter(|name| !name.is_empty())
                        .map(str::to_owned)
                })
                .collect(),
            event_title: meeting.session["event_json"]
                .as_str()
                .and_then(|event| serde_json::from_str::<Value>(event).ok())
                .and_then(|event| event["title"].as_str().map(str::to_owned)),
            duration: first.zip(last).map(|(start, end)| {
                let minutes = (end - start).max(0) / 60_000;
                if minutes >= 60 {
                    format!("{}h {}m", minutes / 60, minutes % 60)
                } else {
                    format!("{minutes}m")
                }
            }),
        }),
    })
}

fn ensure_renderable(value: &Value) -> Result<()> {
    if let Some(kind) = value["type"].as_str()
        && ![
            "doc",
            "paragraph",
            "text",
            "heading",
            "bulletList",
            "orderedList",
            "listItem",
            "blockquote",
            "codeBlock",
            "hardBreak",
            "horizontalRule",
            "taskList",
            "taskItem",
            "image",
        ]
        .contains(&kind)
    {
        return Err(failure(format!(
            "Cannot render document node {kind}; export canonical JSON to preserve it"
        )));
    }
    if let Some(marks) = value["marks"].as_array() {
        for mark in marks {
            if !["bold", "italic", "strike", "code", "link"]
                .contains(&mark["type"].as_str().unwrap_or(""))
            {
                return Err(failure(
                    "Cannot render this document mark; export canonical JSON to preserve it",
                ));
            }
        }
    }
    if let Some(children) = value["content"].as_array() {
        for child in children {
            ensure_renderable(child)?;
        }
    }
    Ok(())
}

fn formatted_date(value: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| {
            date.with_timezone(&chrono::Local)
                .format("%A, %B %-d, %Y at %-I:%M %p")
                .to_string()
        })
        .unwrap_or_else(|_| value.to_string())
}

fn render_text(input: &anlg_export_core::ExportInput, format: Format) -> String {
    let Some(metadata) = &input.metadata else {
        return String::new();
    };
    let mut lines = match format {
        Format::Text => vec![
            metadata.title.clone(),
            "=".repeat(metadata.title.encode_utf16().count()),
        ],
        Format::Org => vec![format!("#+TITLE: {}", metadata.title)],
        _ => vec![format!("# {}", metadata.title)],
    };
    if format == Format::Org {
        if !metadata.created_at.is_empty() {
            lines.push(format!("#+DATE: {}", metadata.created_at));
        }
        lines.extend([String::new(), "* Metadata".into()]);
    }
    for (key, value) in [
        ("Created", metadata.created_at.clone()),
        ("Participants", metadata.participants.join(", ")),
        ("Duration", metadata.duration.clone().unwrap_or_default()),
    ] {
        if value.is_empty() {
            continue;
        }
        lines.push(match format {
            Format::Text if key == "Created" => value,
            Format::Text => format!("{key}: {value}"),
            Format::Org => format!("- {key} :: {value}"),
            _ => format!("- {key}: {value}"),
        });
    }
    let transcript = input
        .transcript
        .as_ref()
        .map(|transcript| {
            transcript
                .items
                .iter()
                .map(|item| match &item.speaker {
                    Some(speaker) => format!("{speaker}: {}", item.text),
                    None => item.text.clone(),
                })
                .collect::<Vec<_>>()
                .join("\n\n")
        })
        .unwrap_or_default();
    for (title, body) in [
        ("Memo", input.memo_md.as_deref().unwrap_or("")),
        ("Summary", input.enhanced_md.as_str()),
        ("Transcript", transcript.as_str()),
    ] {
        if body.is_empty() {
            continue;
        }
        lines.push(String::new());
        match format {
            Format::Text => {
                lines.push(title.into());
                lines.push("-".repeat(title.len()));
            }
            Format::Org => lines.push(format!("* {title}")),
            _ => lines.push(format!("## {title}")),
        }
        lines.push(if title == "Transcript" {
            body.into()
        } else {
            match format {
                Format::Text => markdown_to_text(body),
                Format::Org => markdown_to_org(body),
                _ => body.into(),
            }
        });
    }
    lines.join("\n")
}

fn markdown_to_text(markdown: &str) -> String {
    let mut output = markdown.to_owned();
    for (pattern, replacement) in [
        (r"(?m)^#{1,6}\s+", ""),
        (r"\[([^\]]+)\]\(([^)]+)\)", "$1 ($2)"),
        (r"(?m)^\s*[-*+]\s+", "• "),
        (r"(?m)^\s*\d+\.\s+", ""),
        (r"\*\*(.*?)\*\*", "$1"),
        (r"\*(.*?)\*", "$1"),
        (r"__(.*?)__", "$1"),
        (r"_(.*?)_", "$1"),
        (r"`([^`]+)`", "$1"),
        (r"\n{3,}", "\n\n"),
    ] {
        output = regex::Regex::new(pattern)
            .expect("constant expression")
            .replace_all(&output, replacement)
            .into_owned();
    }
    output.trim().into()
}

fn markdown_to_org(markdown: &str) -> String {
    let mut output = regex::Regex::new(r"(?m)^(#{1,6})\s+")
        .expect("constant expression")
        .replace_all(markdown, |captures: &regex::Captures<'_>| {
            format!("{} ", "*".repeat(captures[1].len()))
        })
        .into_owned();
    for (pattern, replacement) in [
        (r"\[([^\]]+)\]\(([^)]+)\)", "[[$2][$1]]"),
        (r"\*\*(.*?)\*\*", "*$1*"),
        (r"__(.*?)__", "*$1*"),
        (r"`([^`]+)`", "~$1~"),
    ] {
        output = regex::Regex::new(pattern)
            .expect("constant expression")
            .replace_all(&output, replacement)
            .into_owned();
    }
    output.trim().into()
}
