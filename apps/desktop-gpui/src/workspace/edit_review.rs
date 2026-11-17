//! Shipping's Edit review tab (`edit/tab-content.tsx`), the note's pending-edits banner
//! (`session/components/pending-proposals-banner.tsx`) and their shared apply/decline actions
//! (`session/proposal-review.ts`).

use std::{sync::Arc, time::Duration};

use desktop_runtime::{CancellationToken, Result, RuntimeHandle};
use gpui::{
    App, Context, Div, EventEmitter, Global, Render, SharedString, Stateful, Task, WeakEntity,
    Window, div, prelude::*, px, rgb,
};
use serde_json::json;

use super::{
    navigation::Route,
    proposals::{self, Proposal},
    shell::WorkspaceView,
};
use crate::{
    meeting::{ai_view::AiViewServices, model::failure, tools::Approvals},
    ui::theme::{MEDIUM, Theme, monospace_font, theme},
};

/// Bumped whenever a proposal is created, applied or declined, so banners refresh at once.
#[derive(Default)]
pub struct ProposalRevision(pub u64);
impl Global for ProposalRevision {}

pub fn bump(cx: &mut App) {
    cx.default_global::<ProposalRevision>().0 += 1;
}

fn approvals(cx: &App) -> Option<Approvals> {
    cx.try_global::<AiViewServices>()
        .map(|services| services.approvals.clone())
}

/// A shadcn `Button` at `size="sm"`: outline, or the primary fill.
pub fn button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    primary: bool,
    enabled: bool,
    colors: &Theme,
) -> Stateful<Div> {
    let colors = *colors;
    div()
        .id(id.into())
        .h_7()
        .px_2()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .gap_2()
        .text_xs()
        .line_height(px(16.))
        .font_weight(MEDIUM)
        .shadow_xs()
        .map(|button| {
            if primary {
                button
                    .bg(colors.primary)
                    .text_color(colors.primary_foreground)
                    .when(enabled, |button| {
                        button.hover(move |style| style.bg(colors.primary.opacity(0.9)))
                    })
            } else {
                button
                    .border_1()
                    .border_color(colors.input)
                    .bg(colors.background)
                    .text_color(colors.foreground)
                    .when(enabled, |button| {
                        button.hover(move |style| {
                            style.bg(colors.accent).text_color(colors.accent_foreground)
                        })
                    })
            }
        })
        .when(enabled, |button| button.cursor_pointer())
        .when(!enabled, |button| button.opacity(0.5))
        .child(label.into())
}

/// Shipping's `applyProposalReview` / `declineProposalReview`: updates the proposal, answers a
/// waiting chat edit, then closes its review tab.
pub fn decide(
    runtime: RuntimeHandle,
    workspace: WeakEntity<WorkspaceView>,
    id: Arc<str>,
    approved: bool,
    cx: &mut App,
) -> Task<Result<()>> {
    let approvals = approvals(cx);
    cx.spawn(async move |cx| {
        if approved {
            proposals::apply(&runtime, id.clone()).await?;
        } else {
            proposals::decline(&runtime, id.clone()).await?;
        }
        if let Some(approvals) = approvals {
            approvals.resolve(&id, approved);
        }
        cx.update(|cx| {
            bump(cx);
            let _ = workspace.update(cx, |workspace, cx| {
                workspace.close_route(&Route::Edit(id), cx)
            });
        });
        Ok(())
    })
}

#[derive(Clone, Debug, PartialEq)]
struct Edit {
    session_id: Arc<str>,
    memo: bool,
    target_id: Arc<str>,
    current: Arc<str>,
    proposed: Arc<str>,
    source: Arc<str>,
}

impl Edit {
    fn from_proposal(proposal: Proposal) -> Option<Self> {
        (proposal.status.as_ref() == "pending").then(|| Self {
            memo: proposal.memo(),
            session_id: proposal.session_id,
            target_id: proposal.target_id,
            current: proposal.current,
            proposed: proposal.proposed,
            source: proposal.source,
        })
    }
}

pub struct EditReview {
    pub id: Arc<str>,
    runtime: RuntimeHandle,
    workspace: WeakEntity<WorkspaceView>,
    loading: bool,
    edit: Option<Edit>,
    session_title: Option<String>,
    summary_title: Option<String>,
    lines: Vec<DiffLine>,
    busy: bool,
    error: Option<String>,
}

impl EditReview {
    pub fn new(
        id: Arc<str>,
        runtime: RuntimeHandle,
        workspace: WeakEntity<WorkspaceView>,
        cx: &mut Context<Self>,
    ) -> Self {
        let pending = approvals(cx).and_then(|approvals| approvals.get(&id));
        let edit = pending.map(|edit| Edit {
            session_id: edit.session_id,
            memo: edit.target == crate::meeting::tools::EditTarget::Memo,
            target_id: match edit.target {
                crate::meeting::tools::EditTarget::Summary(id) => id,
                crate::meeting::tools::EditTarget::Memo => "".into(),
            },
            current: edit.current,
            proposed: edit.proposed,
            source: edit.source,
        });
        // Shipping's `useStrictModeUnmount(declineOnUnmount)`: leaving the review declines a
        // chat edit that is still waiting; CLI and MCP proposals stay pending.
        cx.on_release(|this: &mut Self, cx| {
            if let Some(approvals) = approvals(cx)
                && approvals
                    .get(&this.id)
                    .is_some_and(|edit| !matches!(edit.source.as_ref(), "cli" | "mcp"))
            {
                approvals.resolve(&this.id, false);
            }
        })
        .detach();
        let mut review = Self {
            id,
            runtime,
            workspace,
            loading: edit.is_none(),
            lines: Vec::new(),
            edit,
            session_title: None,
            summary_title: None,
            busy: false,
            error: None,
        };
        review.load(cx);
        review
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let (runtime, id, known) = (self.runtime.clone(), self.id.clone(), self.edit.clone());
        if let Some(edit) = &self.edit {
            self.lines = diff(&edit.current, &edit.proposed);
        }
        cx.spawn(async move |this, cx| {
            let edit = match known {
                Some(edit) => Some(edit),
                None => proposals::load(&runtime, id)
                    .await
                    .ok()
                    .flatten()
                    .and_then(Edit::from_proposal),
            };
            let titles = match &edit {
                Some(edit) => titles(&runtime, edit).await.unwrap_or_default(),
                None => (None, None),
            };
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                if let Some(edit) = &edit {
                    this.lines = diff(&edit.current, &edit.proposed);
                }
                this.edit = edit;
                (this.session_title, this.summary_title) = titles;
                cx.notify();
            });
        })
        .detach();
    }

    fn review(&mut self, approved: bool, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.error = None;
        let task = decide(
            self.runtime.clone(),
            self.workspace.clone(),
            self.id.clone(),
            approved,
            cx,
        );
        cx.spawn(async move |this, cx| {
            if let Err(error) = task.await {
                let _ = this.update(cx, |this, cx| {
                    this.busy = false;
                    this.error = Some(error.to_string());
                    cx.notify();
                });
            }
        })
        .detach();
        cx.notify();
    }
}

async fn titles(runtime: &RuntimeHandle, edit: &Edit) -> Result<(Option<String>, Option<String>)> {
    let (session, target) = (edit.session_id.clone(), edit.target_id.clone());
    let rows = runtime
        .read(CancellationToken::new(), move |services| async move {
            services
                .executor
                .execute(
                    "SELECT (SELECT title FROM sessions WHERE id = ?1 AND deleted_at IS NULL) AS session_title, (SELECT title FROM session_documents WHERE id = ?2 AND deleted_at IS NULL) AS summary_title".into(),
                    vec![json!(session), json!(target)],
                )
                .await
                .map_err(failure)
        })?
        .receive()
        .await?;
    let title = |key: &str| {
        rows.first()
            .and_then(|row| row[key].as_str())
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(str::to_owned)
    };
    Ok((title("session_title"), title("summary_title")))
}

impl Render for EditReview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let centered = |text: &'static str| {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(colors.muted_foreground)
                .child(text)
        };
        let Some(edit) = self.edit.clone() else {
            return if self.loading {
                centered("Loading edit…")
            } else {
                centered("This edit is no longer pending.")
            };
        };
        let name = if edit.memo { "memo.md" } else { "summary.md" };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .gap_3()
                    .border_b_1()
                    .border_color(colors.border)
                    .px_4()
                    .py_3()
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .line_height(px(19.5))
                                    .font_weight(MEDIUM)
                                    .text_color(colors.foreground)
                                    .child(
                                        self.session_title
                                            .clone()
                                            .unwrap_or_else(|| "Untitled session".into()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .line_height(px(18.))
                                    .text_color(colors.muted_foreground)
                                    .child(if edit.memo {
                                        "Memo".to_owned()
                                    } else {
                                        self.summary_title
                                            .clone()
                                            .unwrap_or_else(|| "Summary".into())
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .gap_2()
                            .child(
                                button("edit-decline", "Decline", false, !self.busy, &colors).when(
                                    !self.busy,
                                    |button| {
                                        button.on_click(
                                            cx.listener(|this, _, _, cx| this.review(false, cx)),
                                        )
                                    },
                                ),
                            )
                            .child(
                                button(
                                    "edit-apply",
                                    if edit.memo {
                                        "Apply to memo"
                                    } else {
                                        "Apply to summary"
                                    },
                                    true,
                                    !self.busy,
                                    &colors,
                                )
                                .when(!self.busy, |button| {
                                    button.on_click(
                                        cx.listener(|this, _, _, cx| this.review(true, cx)),
                                    )
                                }),
                            ),
                    ),
            )
            .when_some(self.error.clone(), |view, error| {
                view.child(
                    div()
                        .bg(rgb(0xfef2f2))
                        .px_4()
                        .py_2()
                        .text_size(px(13.))
                        .line_height(px(19.5))
                        .text_color(rgb(0xdc2626))
                        .child(error),
                )
            })
            .child(
                div()
                    .id("edit-diff")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(diff_view(
                        name,
                        &self.lines,
                        edit.proposed.is_empty() || edit.proposed.ends_with('\n'),
                        cx,
                    )),
            )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Change {
    Same,
    Removed,
    Added,
}

#[derive(Clone, Debug, PartialEq)]
struct DiffLine {
    change: Change,
    old: Option<usize>,
    new: Option<usize>,
    text: SharedString,
}

/// A line diff (longest common subsequence after trimming the shared ends).
fn diff(old: &str, new: &str) -> Vec<DiffLine> {
    let (a, b) = (
        old.lines().collect::<Vec<_>>(),
        new.lines().collect::<Vec<_>>(),
    );
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (middle_a, middle_b) = (&a[prefix..a.len() - suffix], &b[prefix..b.len() - suffix]);
    let mut changes = vec![Change::Same; prefix];
    let (n, m) = (middle_a.len(), middle_b.len());
    if n * m <= 4_000_000 {
        let mut table = vec![0u32; (n + 1) * (m + 1)];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                table[i * (m + 1) + j] = if middle_a[i] == middle_b[j] {
                    table[(i + 1) * (m + 1) + j + 1] + 1
                } else {
                    table[(i + 1) * (m + 1) + j].max(table[i * (m + 1) + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n || j < m {
            if i < n && j < m && middle_a[i] == middle_b[j] {
                changes.push(Change::Same);
                i += 1;
                j += 1;
            } else if j == m
                || (i < n && table[(i + 1) * (m + 1) + j] >= table[i * (m + 1) + j + 1])
            {
                changes.push(Change::Removed);
                i += 1;
            } else {
                changes.push(Change::Added);
                j += 1;
            }
        }
    } else {
        changes.extend(std::iter::repeat_n(Change::Removed, n));
        changes.extend(std::iter::repeat_n(Change::Added, m));
    }
    changes.extend(std::iter::repeat_n(Change::Same, suffix));
    let (mut old_line, mut new_line) = (0, 0);
    changes
        .into_iter()
        .map(|change| {
            let (old, new, text) = match change {
                Change::Same => {
                    old_line += 1;
                    new_line += 1;
                    (Some(old_line), Some(new_line), b[new_line - 1])
                }
                Change::Removed => {
                    old_line += 1;
                    (Some(old_line), None, a[old_line - 1])
                }
                Change::Added => {
                    new_line += 1;
                    (None, Some(new_line), b[new_line - 1])
                }
            };
            DiffLine {
                change,
                old,
                new,
                text: text.to_owned().into(),
            }
        })
        .collect()
}

/// `@pierre/diffs`' light theme, sampled from the shipping review tab.
const ADDED: u32 = 0x00cab1;
const REMOVED: u32 = 0xff2e3f;
const ADDED_LINE: u32 = 0xeaf9f5;
const ADDED_NUMBER: u32 = 0xeffbf8;
const ADDED_WORDS: u32 = 0xc7f2ea;
const REMOVED_LINE: u32 = 0xffeae6;
const REMOVED_NUMBER: u32 = 0xffefec;
const REMOVED_WORDS: u32 = 0xffcdc6;
const SYNTAX: u32 = 0xd52c36;

/// Shiki's Markdown colors for a line: headings, and list or quote markers.
fn syntax(text: &str) -> (usize, bool) {
    let trimmed = text.trim_start();
    let indent = text.len() - trimmed.len();
    if trimmed.starts_with('#') {
        return (text.len(), true);
    }
    let marker = ["- ", "* ", "+ ", "> "]
        .iter()
        .find(|marker| trimmed.starts_with(**marker))
        .map(|_| 1)
        .or_else(|| {
            let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
            (digits > 0 && trimmed[digits..].starts_with(". ")).then_some(digits + 1)
        });
    (marker.map_or(0, |marker| indent + marker), false)
}

/// Words and the whitespace between them, as the diff's word highlights split a line.
fn tokens(line: &str) -> Vec<std::ops::Range<usize>> {
    let mut tokens = Vec::new();
    let mut start = 0;
    let mut kind = None;
    for (index, character) in line.char_indices() {
        let next = if character.is_alphanumeric() {
            0
        } else if character.is_whitespace() {
            1
        } else {
            2
        };
        // Punctuation stands alone; words and whitespace runs group.
        if kind.is_some() && (kind != Some(next) || next == 2) {
            tokens.push(start..index);
            start = index;
        }
        kind = Some(next);
    }
    if start < line.len() {
        tokens.push(start..line.len());
    }
    tokens
}

/// The parts of a replaced line missing from its partner, by word.
fn changed_ranges(line: &str, other: &str) -> Vec<std::ops::Range<usize>> {
    let (a, b) = (tokens(line), tokens(other));
    let (n, m) = (a.len(), b.len());
    if n * m > 250_000 {
        return Vec::new();
    }
    let same = |i: usize, j: usize| line[a[i].clone()] == other[b[j].clone()];
    let mut table = vec![0u32; (n + 1) * (m + 1)];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[i * (m + 1) + j] = if same(i, j) {
                table[(i + 1) * (m + 1) + j + 1] + 1
            } else {
                table[(i + 1) * (m + 1) + j].max(table[i * (m + 1) + j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();
    while i < n {
        if j < m && same(i, j) {
            i += 1;
            j += 1;
        } else if j < m && table[(i + 1) * (m + 1) + j] < table[i * (m + 1) + j + 1] {
            j += 1;
        } else {
            match ranges.last_mut() {
                Some(range) if range.end == a[i].start => range.end = a[i].end,
                _ => ranges.push(a[i].clone()),
            }
            i += 1;
        }
    }
    // A wholly replaced line is shown by its row color alone.
    if ranges.len() == 1 && ranges[0] == (0..line.len()) {
        return Vec::new();
    }
    ranges
}

/// Splits syntax colors and word highlights into the sorted, disjoint runs text needs.
fn highlight_runs(
    length: usize,
    syntax: std::ops::Range<usize>,
    words: &[std::ops::Range<usize>],
    background: gpui::Hsla,
) -> Vec<(std::ops::Range<usize>, gpui::HighlightStyle)> {
    let mut bounds = vec![0, length, syntax.start, syntax.end];
    for range in words {
        bounds.extend([range.start, range.end]);
    }
    bounds.sort_unstable();
    bounds.dedup();
    bounds
        .windows(2)
        .filter_map(|pair| {
            let range = pair[0]..pair[1];
            let colored = syntax.start <= range.start && range.end <= syntax.end;
            let marked = words
                .iter()
                .any(|word| word.start <= range.start && range.end <= word.end);
            (!range.is_empty() && (colored || marked)).then(|| {
                (
                    range,
                    gpui::HighlightStyle {
                        color: colored.then(|| rgb(SYNTAX).into()),
                        background_color: marked.then_some(background),
                        ..Default::default()
                    },
                )
            })
        })
        .collect()
}

fn diff_view(name: &str, lines: &[DiffLine], ends_with_newline: bool, cx: &App) -> Div {
    let added = lines
        .iter()
        .filter(|line| line.change == Change::Added)
        .count();
    let removed = lines
        .iter()
        .filter(|line| line.change == Change::Removed)
        .count();
    let mono = monospace_font(cx);
    // Pair each run of removed lines with the added run after it, for word highlights.
    let mut partners = vec![None; lines.len()];
    let mut index = 0;
    while index < lines.len() {
        let removed_start = index;
        while index < lines.len() && lines[index].change == Change::Removed {
            index += 1;
        }
        let added_start = index;
        while index < lines.len() && lines[index].change == Change::Added {
            index += 1;
        }
        let pairs = (added_start - removed_start).min(index - added_start);
        for offset in 0..pairs {
            partners[removed_start + offset] = Some(added_start + offset);
            partners[added_start + offset] = Some(removed_start + offset);
        }
        if index == removed_start {
            index += 1;
        }
    }
    let row = |line: &DiffLine, partner: Option<&DiffLine>| {
        let (line_bg, number_bg, accent, words) = match line.change {
            Change::Same => (None, None, None, None),
            Change::Added => (
                Some(ADDED_LINE),
                Some(ADDED_NUMBER),
                Some(ADDED),
                Some(ADDED_WORDS),
            ),
            Change::Removed => (
                Some(REMOVED_LINE),
                Some(REMOVED_NUMBER),
                Some(REMOVED),
                Some(REMOVED_WORDS),
            ),
        };
        let text = line.text.to_string();
        let (marker, heading) = syntax(&text);
        let words_ranges = match (words, partner) {
            (Some(_), Some(partner)) => changed_ranges(&text, &partner.text),
            _ => Vec::new(),
        };
        let highlights = highlight_runs(
            text.len(),
            if heading { 0..text.len() } else { 0..marker },
            &words_ranges,
            words.map_or(gpui::transparent_black(), |color| rgb(color).into()),
        );
        let removed = line.change == Change::Removed;
        div()
            .flex()
            .h(px(20.))
            .child(
                div()
                    .w(px(4.))
                    .h_full()
                    .flex_shrink_0()
                    .when_some(accent.filter(|_| !removed), |bar, color| bar.bg(rgb(color)))
                    // Deletions get a dashed bar.
                    .when(removed, |bar| {
                        bar.flex().flex_col().children((0..10).map(|_| {
                            div()
                                .h(px(2.))
                                .w_full()
                                .child(div().h(px(1.)).w_full().bg(rgb(REMOVED)))
                        }))
                    }),
            )
            .child(
                div()
                    .w(px(35.))
                    .h_full()
                    .flex_shrink_0()
                    .pr(px(8.))
                    .flex()
                    .justify_end()
                    .when_some(number_bg, |cell, background| cell.bg(rgb(background)))
                    .text_color(accent.map_or(rgb(0x555555).into(), |color| -> gpui::Hsla {
                        rgb(color).into()
                    }))
                    .child(
                        line.new
                            .filter(|_| !removed)
                            .or(line.old)
                            .map(|number| number.to_string())
                            .unwrap_or_default(),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .ml(px(2.))
                    .pl(px(8.))
                    .when_some(line_bg, |content, background| content.bg(rgb(background)))
                    .text_color(rgb(0x070707))
                    .whitespace_nowrap()
                    .child(gpui::StyledText::new(text).with_highlights(highlights)),
            )
    };
    div()
        .flex()
        .flex_col()
        .font_family(mono.clone())
        .text_size(px(13.))
        .line_height(px(20.))
        .child(
            div()
                .h(px(44.))
                .flex()
                .items_center()
                .gap_2()
                .px_4()
                .child(
                    div()
                        .size(px(16.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .border(px(1.5))
                        .border_color(rgb(0x009fff))
                        .child(div().size(px(6.)).bg(rgb(0x009fff))),
                )
                .child(
                    div()
                        .flex_1()
                        .font_family(crate::ui::theme::system_font(cx))
                        .text_color(rgb(0x070707))
                        .child(name.to_owned()),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(div().text_color(rgb(REMOVED)).child(format!("-{removed}")))
                        .child(div().text_color(rgb(ADDED)).child(format!("+{added}"))),
                ),
        )
        .children(
            lines
                .iter()
                .enumerate()
                .map(|(index, line)| row(line, partners[index].map(|partner| &lines[partner]))),
        )
        .when(!ends_with_newline, |view| {
            view.child(
                div()
                    .h(px(20.))
                    .pl(px(49.))
                    .text_color(rgb(0x696969))
                    .child("No newline at end of file"),
            )
        })
}

/// Opens a proposal's review from the note banner.
pub struct OpenReview(pub Arc<str>);

/// Shipping's `PendingProposalsBanner`, refreshed every four seconds like its query, since
/// the CLI and MCP write proposals from other processes.
pub struct PendingEdits {
    runtime: RuntimeHandle,
    session: Option<Arc<str>>,
    proposals: Vec<Proposal>,
    generation: u64,
    _poll: Option<Task<()>>,
    _revision: gpui::Subscription,
}

impl EventEmitter<OpenReview> for PendingEdits {}

impl PendingEdits {
    pub fn new(runtime: RuntimeHandle, cx: &mut Context<Self>) -> Self {
        cx.default_global::<ProposalRevision>();
        Self {
            runtime,
            session: None,
            proposals: Vec::new(),
            generation: 0,
            _poll: None,
            _revision: cx.observe_global::<ProposalRevision>(|this, cx| this.refresh(cx)),
        }
    }

    pub fn set_session(&mut self, session: Option<Arc<str>>, cx: &mut Context<Self>) {
        if self.session == session {
            return;
        }
        self.session = session;
        self.proposals.clear();
        self._poll = self.session.as_ref().map(|_| {
            cx.spawn(async move |this, cx| {
                loop {
                    if this.update(cx, |this, cx| this.refresh(cx)).is_err() {
                        break;
                    }
                    cx.background_executor().timer(Duration::from_secs(4)).await;
                }
            })
        });
        cx.notify();
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.session.clone() else {
            return;
        };
        self.generation += 1;
        let (generation, runtime) = (self.generation, self.runtime.clone());
        cx.spawn(async move |this, cx| {
            let Ok(proposals) = proposals::pending(&runtime, session).await else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                if this.generation == generation && this.proposals != proposals {
                    this.proposals = proposals;
                    cx.notify();
                }
            });
        })
        .detach();
    }
}

impl Render for PendingEdits {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        if self.proposals.is_empty() {
            return div();
        }
        let count = self.proposals.len();
        // Shipping's `px-1` sits inside the note body's 8px inset.
        div().flex_shrink_0().px_3().pt_1().pb_2().child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .border_1()
                .border_color(colors.border.opacity(0.7))
                .bg(colors.card.opacity(0.8))
                .px_3()
                .py_2()
                .child(
                    div()
                        .text_size(px(13.))
                        .line_height(px(19.5))
                        .font_weight(MEDIUM)
                        .text_color(colors.foreground)
                        .child(if count == 1 {
                            "1 pending edit".to_owned()
                        } else {
                            format!("{count} pending edits")
                        }),
                )
                .child(div().flex().flex_wrap().justify_end().gap_2().children(
                    self.proposals.iter().map(|proposal| {
                        let id = proposal.id.clone();
                        button(
                            format!("review-{id}"),
                            if proposal.memo() {
                                "Review memo"
                            } else {
                                "Review summary"
                            },
                            false,
                            true,
                            &colors,
                        )
                        .on_click(cx.listener(move |_, _, _, cx| cx.emit(OpenReview(id.clone()))))
                    }),
                )),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaced_lines_highlight_only_the_changed_words() {
        let line = "Ship the simplified onboarding in the next release.";
        let ranges = changed_ranges(line, "Ship the simplified onboarding next release.");
        assert_eq!(
            ranges
                .iter()
                .map(|range| &line[range.clone()])
                .collect::<Vec<_>>(),
            ["in the "]
        );
        assert!(changed_ranges("Old", "New").is_empty());
    }

    #[test]
    fn diff_keeps_shared_lines_and_orders_removals_before_additions() {
        let lines = diff(
            "# Plan\nShip Friday\nAsk Ben\n",
            "# Plan\nShip Monday\nAsk Ben\nAdd QA\n",
        );
        let changes = lines
            .iter()
            .map(|line| (line.change, line.old, line.new, line.text.as_ref()))
            .collect::<Vec<_>>();
        assert_eq!(
            changes,
            vec![
                (Change::Same, Some(1), Some(1), "# Plan"),
                (Change::Removed, Some(2), None, "Ship Friday"),
                (Change::Added, None, Some(2), "Ship Monday"),
                (Change::Same, Some(3), Some(3), "Ask Ben"),
                (Change::Added, None, Some(4), "Add QA"),
            ]
        );
    }
}
