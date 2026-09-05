//! The overflow menu's Export dialog:
//! `session/components/outer-header/overflow/export-modal.tsx` + `export-utils.ts`.

use std::path::PathBuf;

use chrono::{Datelike, Timelike};
use gpui::{AnyElement, ClickEvent, Context, Div, MouseButton, SharedString, div, prelude::*, px};

use super::Workspace;
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileFormat {
    Pdf,
    Txt,
    Md,
    Org,
}

impl FileFormat {
    const ALL: [FileFormat; 4] = [Self::Pdf, Self::Txt, Self::Md, Self::Org];

    fn label(self) -> &'static str {
        match self {
            Self::Pdf => "PDF",
            Self::Txt => "TXT",
            Self::Md => "Markdown",
            Self::Org => "Org",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Txt => "txt",
            Self::Md => "md",
            Self::Org => "org",
        }
    }
}

/// `ExportModal`'s state: the defaults are PDF with only the summary.
pub(crate) struct ExportDialog {
    pub format: FileFormat,
    pub include_memo: bool,
    pub include_summary: bool,
    pub include_transcript: bool,
    pub pending: bool,
    pub error: Option<String>,
}

impl Default for ExportDialog {
    fn default() -> Self {
        Self {
            format: FileFormat::Pdf,
            include_memo: false,
            include_summary: true,
            include_transcript: false,
            pending: false,
            error: None,
        }
    }
}

/// Everything the builders read, gathered from the open note.
struct ExportSource {
    title: String,
    created_at: Option<String>,
    event_title: Option<String>,
    participants: Vec<String>,
    memo_md: String,
    summary_md: String,
    transcript: Vec<anlg_export_core::TranscriptItem>,
    duration: Option<String>,
}

impl Workspace {
    pub(crate) fn open_export_dialog(&mut self, cx: &mut Context<Self>) {
        self.export_dialog = Some(ExportDialog::default());
        cx.notify();
    }

    pub(crate) fn close_export_dialog(&mut self, cx: &mut Context<Self>) {
        self.export_dialog = None;
        cx.notify();
    }

    /// Reads the note the way the modal's hooks do: the memo's `raw_md`, the
    /// enhanced note of the current view, participants, transcript segments.
    fn export_source(&self) -> Option<ExportSource> {
        let (preview, tab) = match &self.note {
            super::Note::Ready { preview, tab } => (preview, tab),
            _ => return None,
        };
        let json_to_md = |body: &str| -> String {
            serde_json::from_str::<serde_json::Value>(body)
                .ok()
                .and_then(|json| anlg_tiptap::tiptap_json_to_md(&json).ok())
                .unwrap_or_default()
        };
        let summary_md = match tab {
            super::NoteTab::Enhanced(id) => preview
                .enhanced
                .iter()
                .find(|document| document.id == *id)
                .map(|document| json_to_md(&document.body))
                .unwrap_or_default(),
            _ => String::new(),
        };
        let transcript: Vec<anlg_export_core::TranscriptItem> = preview
            .transcripts
            .iter()
            .flat_map(|transcript| transcript.segments.iter())
            .map(|segment| anlg_export_core::TranscriptItem {
                speaker: (!segment.export_speaker.is_empty())
                    .then(|| segment.export_speaker.clone()),
                text: segment.export_text.clone(),
            })
            .collect();
        // `useSessionTranscriptMetadata` -> min started / max ended.
        let started = preview
            .transcripts
            .iter()
            .map(|transcript| transcript.started_at_ms)
            .min();
        let ended = preview
            .transcripts
            .iter()
            .filter_map(|transcript| transcript.ended_at_ms)
            .max();
        let duration = match (started, ended) {
            (Some(start), Some(end)) => Some(format_duration(start, end)),
            _ => None,
        };
        // Filled from `useSessionParticipants` right before writing.
        let participants = Vec::new();
        Some(ExportSource {
            title: preview.session.title.clone(),
            created_at: Some(preview.session.created_at.clone()).filter(|value| !value.is_empty()),
            event_title: preview
                .session_event()
                .and_then(|event| event.title)
                .filter(|title| !title.is_empty()),
            participants,
            memo_md: json_to_md(&preview.memo_body),
            summary_md,
            transcript,
            duration,
        })
    }

    /// `mutate`: write `<Downloads>/<title>_<timestamp>.<ext>` and reveal it.
    fn run_export(&mut self, cx: &mut Context<Self>) {
        let Some(source) = self.export_source() else {
            return;
        };
        let Some(dialog) = self.export_dialog.as_mut() else {
            return;
        };
        let (format, memo, summary, transcript) = (
            dialog.format,
            dialog.include_memo,
            dialog.include_summary,
            dialog.include_transcript,
        );
        dialog.pending = true;
        dialog.error = None;
        cx.notify();
        let session_id = match &self.note {
            super::Note::Ready { preview, .. } => preview.session.id.clone(),
            _ => return,
        };
        let participants = self.store.list_session_participants(session_id);
        let task = self.store.runtime().spawn(async move {
            let mut source = source;
            // `participants.map(p => p.name).filter(Boolean)`
            source.participants = participants
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or_default()
                .into_iter()
                .filter(|participant| participant.source != "excluded")
                .map(|participant| participant.name)
                .filter(|name| !name.is_empty())
                .collect();
            tokio::task::spawn_blocking(move || {
                write_export(source, format, memo, summary, transcript)
            })
            .await
            .map_err(anyhow::Error::from)
            .and_then(|r| r)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update(cx, |this, cx| {
                match result {
                    Ok(path) => {
                        // `revealItemInDir`
                        if let Some(dir) = path.parent() {
                            cx.open_url(&format!("file://{}", dir.display()));
                        }
                        this.export_dialog = None;
                    }
                    Err(error) => {
                        tracing::error!(%error, "export failed");
                        if let Some(dialog) = this.export_dialog.as_mut() {
                            dialog.pending = false;
                            dialog.error = Some(error.to_string());
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `DialogContent`: `bg-black/20` overlay, a `max-w-xs p-4` slot holding
    /// the `rounded-xl border bg-background p-5 gap-4 text-center` card.
    pub(super) fn render_export_dialog(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let dialog = self.export_dialog.as_ref()?;
        let theme = self.theme;
        let has_selection =
            dialog.include_memo || dialog.include_summary || dialog.include_transcript;
        let disabled = dialog.pending || !has_selection;
        let hovered = self.hovered == Some("export-submit");
        let label = if dialog.pending {
            "Exporting..."
        } else {
            "Export"
        };

        let heading = |text: &'static str| {
            div()
                .tw_text_sm()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.foreground)
                .child(text)
        };

        let formats = div()
            .flex()
            .justify_center()
            .gap_4()
            .children(FileFormat::ALL.iter().map(|format| {
                let selected = dialog.format == *format;
                let format = *format;
                div()
                    .id(SharedString::from(format!(
                        "export-format-{}",
                        format.extension()
                    )))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .tw_text_sm()
                    .text_color(theme.foreground)
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(dialog) = this.export_dialog.as_mut() {
                            dialog.format = format;
                            cx.notify();
                        }
                    }))
                    .child(radio(theme, selected))
                    .child(format.label())
            }));

        let includes = div().flex().justify_center().gap_4().children(
            [
                ("memo", "Memo", dialog.include_memo),
                ("summary", "Summary", dialog.include_summary),
                ("transcript", "Transcript", dialog.include_transcript),
            ]
            .into_iter()
            .map(|(id, label, checked)| {
                div()
                    .id(SharedString::from(format!("export-include-{id}")))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .tw_text_sm()
                    .text_color(theme.foreground)
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(dialog) = this.export_dialog.as_mut() {
                            match id {
                                "memo" => dialog.include_memo = !dialog.include_memo,
                                "summary" => dialog.include_summary = !dialog.include_summary,
                                _ => dialog.include_transcript = !dialog.include_transcript,
                            }
                            cx.notify();
                        }
                    }))
                    .child(checkbox(theme, checked))
                    .child(label)
            }),
        );

        let card = div()
            .id("export-card")
            .w(px(288.0))
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .rounded_xl()
            .border_1()
            .border_color(alpha(theme.border, 0.8))
            .bg(theme.background)
            .shadow(vec![gpui::BoxShadow {
                color: gpui::hsla(0.0, 0.0, 0.0, 0.25),
                offset: gpui::point(px(0.0), px(25.0)),
                blur_radius: px(50.0),
                spread_radius: px(-12.0),
            }])
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .tw_text_base()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.foreground)
                            .child("Export"),
                    )
                    .child(
                        div()
                            .tw_text_sm()
                            .text_color(theme.muted_foreground)
                            .text_center()
                            .child("Choose a file format and what to include."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .child(heading("File format"))
                            .child(formats),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .child(heading("Include"))
                            .child(includes),
                    ),
            )
            .children(dialog.error.clone().map(|error| {
                div()
                    .tw_text_xs()
                    .text_color(theme.destructive)
                    .child(SharedString::from(error))
            }))
            .child(
                // `<button className="h-10 w-full rounded-full border-2 border-primary
                // bg-primary text-primary-foreground text-sm font-medium shadow">`
                // (`rounded-full` is `0.5rem` here).
                div()
                    .id("export-submit")
                    .flex()
                    .h(px(40.0))
                    .w_full()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.0))
                    .border_2()
                    .border_color(theme.primary)
                    .bg(if hovered && !disabled {
                        alpha(theme.primary, 0.9)
                    } else {
                        theme.primary
                    })
                    .shadow(vec![gpui::BoxShadow {
                        color: gpui::hsla(0.0, 0.0, 0.0, 0.4),
                        offset: gpui::point(px(0.0), px(4.0)),
                        blur_radius: px(14.0),
                        spread_radius: px(0.0),
                    }])
                    .tw_text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.primary_foreground)
                    .when(disabled, |button| button.opacity(0.5).cursor_not_allowed())
                    .when(!disabled, |button| {
                        button
                            .cursor_pointer()
                            .on_hover(cx.listener(|this, hovering: &bool, _, cx| {
                                this.set_hovered("export-submit", *hovering, cx);
                            }))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| this.run_export(cx)),
                            )
                    })
                    .child(label),
            );

        Some(
            gpui::deferred(
                div()
                    .id("export-overlay")
                    .occlude()
                    .absolute()
                    .inset_0()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gpui::hsla(0.0, 0.0, 0.0, 0.2))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| this.close_export_dialog(cx)),
                    )
                    .child(div().p_4().child(card)),
            )
            .with_priority(5)
            .into_any_element(),
        )
    }
}

/// `mutationFn`: `<Downloads>/<sanitized title>_<timestamp>.<ext>`, the PDF
/// through `anlg-export-core`, the text formats through the builders.
fn write_export(
    source: ExportSource,
    format: FileFormat,
    memo: bool,
    summary: bool,
    transcript: bool,
) -> anyhow::Result<PathBuf> {
    let downloads =
        dirs::download_dir().ok_or_else(|| anyhow::anyhow!("no downloads directory"))?;
    let title = source.title.trim();
    let title = if title.is_empty() { "Untitled" } else { title };
    let sanitized: String = title
        .chars()
        .map(|c| if "<>:\"/\\|?*".contains(c) { '_' } else { c })
        .collect();
    let timestamp = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string()
        .replace([':', '.'], "-");
    let path = downloads.join(format!("{sanitized}_{timestamp}.{}", format.extension()));
    match format {
        FileFormat::Pdf => {
            let input = build_pdf(&source, memo, summary, transcript);
            anlg_export_core::export_pdf(&path, input)
                .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        }
        FileFormat::Md => std::fs::write(&path, build_md(&source, memo, summary, transcript))?,
        FileFormat::Org => std::fs::write(&path, build_org(&source, memo, summary, transcript))?,
        FileFormat::Txt => std::fs::write(&path, build_txt(&source, memo, summary, transcript))?,
    }
    Ok(path)
}

/// WebKitGTK's native radio: a 13px circle, filled `accent-primary` with a
/// white dot when checked.
fn radio(theme: crate::theme::Theme, checked: bool) -> Div {
    div()
        .size(px(13.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .border_1()
        .border_color(if checked {
            theme.primary
        } else {
            gpui::rgb(0x8f8f8f)
        })
        .bg(if checked {
            theme.primary
        } else {
            theme.background
        })
        .when(checked, |dot| {
            dot.child(
                div()
                    .size(px(5.0))
                    .rounded_full()
                    .bg(theme.primary_foreground),
            )
        })
}

/// WebKitGTK's native checkbox: a 13px `rounded-sm` box, filled with a white
/// check when checked.
fn checkbox(theme: crate::theme::Theme, checked: bool) -> Div {
    div()
        .size(px(13.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(2.0))
        .border_1()
        .border_color(if checked {
            theme.primary
        } else {
            gpui::rgb(0x8f8f8f)
        })
        .bg(if checked {
            theme.primary
        } else {
            theme.background
        })
        .when(checked, |b| {
            b.child(icon("check", px(11.0), theme.primary_foreground))
        })
}

/// `formatDate`: `toLocaleDateString("en-US", { weekday: "long", year: "numeric",
/// month: "long", day: "numeric", hour: "numeric", minute: "2-digit" })`.
fn format_date(iso: &str) -> String {
    let Some(utc) = crate::timeline::parse_date(iso, &chrono::Local) else {
        return "Invalid Date".to_string();
    };
    let local = utc.with_timezone(&chrono::Local);
    let (pm, hour12) = local.hour12();
    format!(
        "{}, {} {}, {} at {}:{:02} {}",
        local.format("%A"),
        local.format("%B"),
        local.day(),
        local.year(),
        hour12,
        local.minute(),
        if pm { "PM" } else { "AM" }
    )
}

/// `formatDuration`
fn format_duration(start_ms: i64, end_ms: i64) -> String {
    let minutes = (end_ms - start_ms) / 60_000;
    let hours = minutes / 60;
    let remaining = minutes % 60;
    if hours > 0 {
        format!("{hours}h {remaining}m")
    } else {
        format!("{minutes}m")
    }
}

fn transcript_text(items: &[anlg_export_core::TranscriptItem]) -> String {
    items
        .iter()
        .map(|item| match &item.speaker {
            Some(speaker) => format!("{speaker}: {}", item.text),
            None => item.text.clone(),
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn title_of(source: &ExportSource) -> String {
    if source.title.is_empty() {
        "Untitled".to_string()
    } else {
        source.title.clone()
    }
}

/// `buildMdContent`
fn build_md(source: &ExportSource, memo: bool, summary: bool, transcript: bool) -> String {
    let mut sections = vec![format!("# {}", title_of(source))];
    if let Some(created) = &source.created_at {
        sections.push(format!("- Created: {}", format_date(created)));
    }
    if !source.participants.is_empty() {
        sections.push(format!(
            "- Participants: {}",
            source.participants.join(", ")
        ));
    }
    if let Some(duration) = &source.duration {
        sections.push(format!("- Duration: {duration}"));
    }
    if memo && !source.memo_md.is_empty() {
        sections.extend(["".into(), "## Memo".into(), source.memo_md.clone()]);
    }
    if summary && !source.summary_md.is_empty() {
        sections.extend(["".into(), "## Summary".into(), source.summary_md.clone()]);
    }
    if transcript && !source.transcript.is_empty() {
        sections.extend([
            "".into(),
            "## Transcript".into(),
            transcript_text(&source.transcript),
        ]);
    }
    sections.join("\n")
}

/// `markdownToText`
fn markdown_to_text(content: &str) -> String {
    let rules: [(&str, &str); 10] = [
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
    ];
    let mut text = content.to_string();
    for (pattern, replacement) in rules {
        text = regex::Regex::new(pattern)
            .expect("static pattern")
            .replace_all(&text, replacement)
            .into_owned();
    }
    text.trim().to_string()
}

/// `markdownToOrg`
fn markdown_to_org(content: &str) -> String {
    let heading = regex::Regex::new(r"(?m)^(#{1,6})\s+").expect("static pattern");
    let mut text = heading
        .replace_all(content, |caps: &regex::Captures| {
            format!("{} ", "*".repeat(caps[1].len()))
        })
        .into_owned();
    for (pattern, replacement) in [
        (r"\[([^\]]+)\]\(([^)]+)\)", "[[$2][$1]]"),
        (r"\*\*(.*?)\*\*", "*$1*"),
        (r"__(.*?)__", "*$1*"),
        (r"`([^`]+)`", "~$1~"),
    ] {
        text = regex::Regex::new(pattern)
            .expect("static pattern")
            .replace_all(&text, replacement)
            .into_owned();
    }
    text.trim().to_string()
}

/// `buildTxtContent`
fn build_txt(source: &ExportSource, memo: bool, summary: bool, transcript: bool) -> String {
    let title = title_of(source);
    let mut sections = vec![title.clone(), "=".repeat(title.chars().count())];
    if let Some(created) = &source.created_at {
        sections.push(format_date(created));
    }
    if !source.participants.is_empty() {
        sections.push(format!("Participants: {}", source.participants.join(", ")));
    }
    if let Some(duration) = &source.duration {
        sections.push(format!("Duration: {duration}"));
    }
    if memo && !source.memo_md.is_empty() {
        sections.extend([
            "".into(),
            "Memo".into(),
            "-".repeat(4),
            markdown_to_text(&source.memo_md),
        ]);
    }
    if summary && !source.summary_md.is_empty() {
        sections.extend([
            "".into(),
            "Summary".into(),
            "-".repeat(7),
            markdown_to_text(&source.summary_md),
        ]);
    }
    if transcript && !source.transcript.is_empty() {
        sections.extend([
            "".into(),
            "Transcript".into(),
            "-".repeat(10),
            transcript_text(&source.transcript),
        ]);
    }
    sections.join("\n")
}

/// `buildOrgContent`
fn build_org(source: &ExportSource, memo: bool, summary: bool, transcript: bool) -> String {
    let mut sections = vec![format!("#+TITLE: {}", title_of(source))];
    if let Some(created) = &source.created_at {
        sections.push(format!("#+DATE: {}", format_date(created)));
    }
    sections.extend(["".into(), "* Metadata".into()]);
    if let Some(created) = &source.created_at {
        sections.push(format!("- Created :: {}", format_date(created)));
    }
    if !source.participants.is_empty() {
        sections.push(format!(
            "- Participants :: {}",
            source.participants.join(", ")
        ));
    }
    if let Some(duration) = &source.duration {
        sections.push(format!("- Duration :: {duration}"));
    }
    if memo && !source.memo_md.is_empty() {
        sections.extend(["".into(), "* Memo".into(), markdown_to_org(&source.memo_md)]);
    }
    if summary && !source.summary_md.is_empty() {
        sections.extend([
            "".into(),
            "* Summary".into(),
            markdown_to_org(&source.summary_md),
        ]);
    }
    if transcript && !source.transcript.is_empty() {
        sections.extend([
            "".into(),
            "* Transcript".into(),
            transcript_text(&source.transcript),
        ]);
    }
    sections.join("\n")
}

/// `buildPdfContent`
fn build_pdf(
    source: &ExportSource,
    memo: bool,
    summary: bool,
    transcript: bool,
) -> anlg_export_core::ExportInput {
    anlg_export_core::ExportInput {
        enhanced_md: if summary {
            source.summary_md.clone()
        } else {
            String::new()
        },
        memo_md: (memo && !source.memo_md.is_empty()).then(|| source.memo_md.clone()),
        transcript: (transcript && !source.transcript.is_empty()).then(|| {
            anlg_export_core::Transcript {
                items: source.transcript.clone(),
            }
        }),
        metadata: Some(anlg_export_core::ExportMetadata {
            title: title_of(source),
            created_at: source
                .created_at
                .as_deref()
                .map(format_date)
                .unwrap_or_default(),
            participants: source.participants.clone(),
            event_title: source.event_title.clone(),
            duration: source.duration.clone(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> ExportSource {
        ExportSource {
            title: "Kickoff".into(),
            created_at: None,
            event_title: None,
            participants: vec!["Ada".into(), "Bob".into()],
            memo_md: "# Notes\n\n- **Bold** item\n- [Site](https://x.y)".into(),
            summary_md: "## Summary\n\nDone.".into(),
            transcript: vec![anlg_export_core::TranscriptItem {
                speaker: Some("Ada".into()),
                text: "Hello".into(),
            }],
            duration: Some(format_duration(0, 65 * 60_000)),
        }
    }

    #[test]
    fn markdown_conversions_follow_the_modal() {
        // `^\s*[-*+]\s+` with the `m` flag also swallows the blank line
        // before a list, exactly as the JS does.
        assert_eq!(
            markdown_to_text("# Notes\n\n- **Bold** item\n- [Site](https://x.y)"),
            "Notes\n• Bold item\n• Site (https://x.y)"
        );
        assert_eq!(
            markdown_to_org("## Head\n\n**b** and `c` [t](u)"),
            "** Head\n\n*b* and ~c~ [[u][t]]"
        );
    }

    #[test]
    fn builders_match_the_section_layout() {
        let source = source();
        let md = build_md(&source, true, true, true);
        assert!(
            md.starts_with("# Kickoff\n- Participants: Ada, Bob\n- Duration: 1h 5m\n\n## Memo\n")
        );
        assert!(md.ends_with("## Transcript\nAda: Hello"));
        let txt = build_txt(&source, false, true, false);
        assert_eq!(
            txt,
            "Kickoff\n=======\nParticipants: Ada, Bob\nDuration: 1h 5m\n\nSummary\n-------\nSummary\n\nDone."
        );
        let org = build_org(&source, false, false, true);
        assert!(org.starts_with("#+TITLE: Kickoff\n\n* Metadata\n- Participants :: Ada, Bob"));
        assert!(org.ends_with("* Transcript\nAda: Hello"));
        let pdf = build_pdf(&source, true, false, true);
        assert_eq!(pdf.enhanced_md, "");
        assert_eq!(pdf.memo_md.as_deref(), Some(source.memo_md.as_str()));
        assert_eq!(pdf.transcript.map(|t| t.items.len()), Some(1));
    }

    #[test]
    fn duration_formats_like_the_modal() {
        assert_eq!(format_duration(0, 5 * 60_000), "5m");
        assert_eq!(format_duration(0, 125 * 60_000), "2h 5m");
    }
}
