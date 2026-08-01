use std::path::{Path, PathBuf};

use super::documents::{SummaryPair, detect_summary_pair};
use super::source_format::normalized_relative_path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceKind {
    Calendar,
    Event,
    Template,
    SessionMeta,
    SessionDocument,
    Transcript,
    Attachment,
    Human,
    Organization,
    Tasks,
    DailyNotes,
    Chat,
    Settings,
}

impl SourceKind {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Calendar => "calendar",
            Self::Event => "event",
            Self::Template => "template",
            Self::SessionMeta => "session_meta",
            Self::SessionDocument => "session_document",
            Self::Transcript => "transcript",
            Self::Attachment => "attachment",
            Self::Human => "human",
            Self::Organization => "organization",
            Self::Tasks => "tasks",
            Self::DailyNotes => "daily_notes",
            Self::Chat => "chat",
            Self::Settings => "settings",
        }
    }
}

#[derive(Debug)]
pub(super) struct SourceFile {
    pub(super) path: PathBuf,
    pub(super) relative_path: String,
    pub(super) kind: SourceKind,
}

impl SourceFile {
    fn import_sort_key(&self) -> (&str, u8, &str) {
        let group = match self.kind {
            SourceKind::Attachment => self
                .relative_path
                .split_once("/attachments/")
                .map_or(self.relative_path.as_str(), |(directory, _)| directory),
            SourceKind::SessionMeta | SourceKind::SessionDocument | SourceKind::Transcript => self
                .relative_path
                .rsplit_once('/')
                .map_or(self.relative_path.as_str(), |(directory, _)| directory),
            _ => self.relative_path.as_str(),
        };
        let dependency_order = match self.kind {
            SourceKind::SessionMeta => 0,
            SourceKind::SessionDocument if self.relative_path.ends_with("/.md") => 2,
            _ => 1,
        };

        (group, dependency_order, self.relative_path.as_str())
    }
}

#[derive(Debug)]
pub(super) struct SourceDiscovery {
    pub(super) files: Vec<SourceFile>,
    pub(super) summary_pairs: Vec<SummaryPair>,
}

pub(super) fn discover_sources(vault_base: &Path) -> std::io::Result<SourceDiscovery> {
    if !vault_base.exists() {
        return Ok(SourceDiscovery {
            files: Vec::new(),
            summary_pairs: Vec::new(),
        });
    }

    let mut files = Vec::new();
    let mut summary_pairs = Vec::new();
    collect_files(vault_base, vault_base, &mut files, &mut summary_pairs)?;
    files.sort_by(|left, right| left.import_sort_key().cmp(&right.import_sort_key()));
    summary_pairs.sort_by(|left, right| left.hidden_relative_path.cmp(&right.hidden_relative_path));
    Ok(SourceDiscovery {
        files,
        summary_pairs,
    })
}

fn collect_files(
    vault_base: &Path,
    directory: &Path,
    files: &mut Vec<SourceFile>,
    summary_pairs: &mut Vec<SummaryPair>,
) -> std::io::Result<()> {
    let mut entries = std::fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_files(vault_base, &path, files, summary_pairs)?;
            continue;
        }

        let relative_path = normalized_relative_path(vault_base, &path);
        if let Some(summary_pair) = detect_summary_pair(vault_base, &path, &relative_path) {
            let hide_hidden_source = summary_pair.hide_hidden_source;
            summary_pairs.push(summary_pair);
            if hide_hidden_source {
                continue;
            }
        }
        if let Some(kind) = classify_source(&relative_path) {
            files.push(SourceFile {
                path,
                relative_path,
                kind,
            });
        }
    }

    Ok(())
}

fn classify_source(relative_path: &str) -> Option<SourceKind> {
    let parts = relative_path.split('/').collect::<Vec<_>>();
    let filename = parts.last().copied()?;

    match parts.as_slice() {
        ["calendars.json"] => return Some(SourceKind::Calendar),
        ["events.json"] => return Some(SourceKind::Event),
        ["templates.json"] => return Some(SourceKind::Template),
        ["tasks.json"] => return Some(SourceKind::Tasks),
        ["daily_notes.json"] => return Some(SourceKind::DailyNotes),
        ["settings.json"] => return Some(SourceKind::Settings),
        _ => {}
    }

    match parts.first().copied() {
        Some("humans") if filename.ends_with(".md") => Some(SourceKind::Human),
        Some("organizations") if filename.ends_with(".md") => Some(SourceKind::Organization),
        Some("chats") if filename == "messages.json" => Some(SourceKind::Chat),
        Some("sessions") if parts.contains(&"attachments") => Some(SourceKind::Attachment),
        Some("sessions") if filename == "_meta.json" => Some(SourceKind::SessionMeta),
        Some("sessions") if filename == "transcript.json" => Some(SourceKind::Transcript),
        Some("sessions") if filename.ends_with(".md") => Some(SourceKind::SessionDocument),
        _ => None,
    }
}
