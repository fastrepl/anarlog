//! `edit/tab-content.tsx` (`TabContentEdit`): the diff review a chat
//! `edit_memo` / `edit_summary` proposal opens — the session title and
//! target, `Decline` / `Apply to memo|summary`, the error banner, and the
//! unified diff of `memo.md` / `summary.md`.

use std::rc::Rc;

use gpui::{AnyElement, Context, SharedString, Window, div, prelude::*, px};

use super::Workspace;
use super::automations_tab::SmallButton;
use crate::chat_tools::{EditTarget, PendingEdit};
use crate::theme::alpha;
use crate::ui::TailwindText as _;

/// The edit tab's content: a proposal handed over by a chat tool, or one
/// loaded by id (`loadSessionProposal`) from the pending-proposals banner.
pub(crate) enum EditReview {
    /// `isLoading`: the proposal row is being read.
    Loading {
        request_id: String,
    },
    /// "This edit is no longer pending."
    Missing,
    Pending(PendingReview),
}

pub(crate) struct PendingReview {
    pub request_id: String,
    pub session_id: String,
    pub target: EditTarget,
    pub current_content: String,
    pub proposed_content: String,
    pub source: String,
    /// The waiting tool's decision channel; `None` once answered or when
    /// the review was opened from the banner.
    pub responder: Option<tokio::sync::oneshot::Sender<bool>>,
    pub error: Option<String>,
    pub busy: bool,
}

/// `shouldAutoDeclineProposal`: closing the review without a choice declines
/// chat proposals; CLI / MCP ones stay pending.
fn should_auto_decline(source: &str) -> bool {
    source != "cli" && source != "mcp"
}

impl Workspace {
    /// `openEditTab(requestId)`: show the proposal's review in the main
    /// surface; an earlier review still open is declined first.
    pub(crate) fn open_edit_review(&mut self, edit: PendingEdit, cx: &mut Context<Self>) {
        self.show_edit_review(
            EditReview::Pending(PendingReview {
                request_id: edit.request_id,
                session_id: edit.session_id,
                target: edit.target,
                current_content: edit.current_content,
                proposed_content: edit.proposed_content,
                source: edit.source,
                responder: Some(edit.responder),
                error: None,
                busy: false,
            }),
            cx,
        );
    }

    /// `openProposalReview(proposalId)` from the pending-proposals banner:
    /// the tab opens loading and shows the `pending` row, or the
    /// no-longer-pending notice.
    pub(crate) fn open_proposal_review(&mut self, proposal_id: String, cx: &mut Context<Self>) {
        self.show_edit_review(
            EditReview::Loading {
                request_id: proposal_id.clone(),
            },
            cx,
        );
        let task = self.store.load_session_proposal(proposal_id.clone());
        cx.spawn(async move |this, cx| {
            let loaded = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update(cx, |this, cx| {
                if !matches!(
                    &this.edit_review,
                    Some(EditReview::Loading { request_id }) if *request_id == proposal_id
                ) {
                    return;
                }
                this.edit_review = Some(match loaded {
                    Ok(Some(proposal)) if proposal.status == "pending" => {
                        EditReview::Pending(PendingReview {
                            request_id: proposal.id,
                            session_id: proposal.session_id,
                            target: if proposal.kind == "memo_replace" {
                                EditTarget::Memo
                            } else {
                                EditTarget::Summary {
                                    enhanced_note_id: proposal.target_id,
                                }
                            },
                            current_content: proposal.current_markdown,
                            proposed_content: proposal.proposed_markdown,
                            source: proposal.source,
                            responder: None,
                            error: None,
                            busy: false,
                        })
                    }
                    Ok(_) => EditReview::Missing,
                    Err(error) => {
                        tracing::error!(%error, "failed to load session proposal");
                        EditReview::Missing
                    }
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn show_edit_review(&mut self, review: EditReview, cx: &mut Context<Self>) {
        self.close_edit_review(cx);
        // The edit tab keeps the timeline sidebar, like a sessions tab.
        self.close_settings(cx);
        self.close_folders(cx);
        self.close_templates(cx);
        self.close_calendar(cx);
        self.close_contacts(cx);
        self.close_automations(cx);
        self.edit_review = Some(review);
        cx.notify();
    }

    pub(crate) fn edit_review_open(&self) -> bool {
        self.edit_review.is_some()
    }

    /// The review's tab closing (`declineOnUnmount`): a chat proposal still
    /// waiting on its tool is declined.
    pub(crate) fn close_edit_review(&mut self, cx: &mut Context<Self>) {
        let Some(review) = self.edit_review.take() else {
            return;
        };
        if let EditReview::Pending(review) = review
            && let Some(responder) = review.responder
            && should_auto_decline(&review.source)
        {
            let decline = self.store.decline_session_proposal(review.request_id);
            cx.spawn(async move |_, _| {
                let _ = decline.await;
            })
            .detach();
            let _ = responder.send(false);
        }
        cx.notify();
    }

    fn pending_review_mut(&mut self, request_id: &str) -> Option<&mut PendingReview> {
        match self.edit_review.as_mut() {
            Some(EditReview::Pending(review)) if review.request_id == request_id => Some(review),
            _ => None,
        }
    }

    /// `applyProposalReview` / `declineProposalReview`: commit the decision,
    /// answer the waiting tool, and close the review; an apply failure stays
    /// on the review with its message.
    pub(crate) fn review_edit(&mut self, request_id: &str, approved: bool, cx: &mut Context<Self>) {
        let Some(review) = self.pending_review_mut(request_id) else {
            return;
        };
        if review.busy {
            return;
        }
        review.busy = true;
        review.error = None;
        cx.notify();
        let request_id = request_id.to_string();
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let outcome: Result<(), String> = if approved {
                store
                    .apply_session_proposal(request_id.clone())
                    .await
                    .unwrap_or_else(|error| Err(error.to_string()))
            } else {
                store
                    .decline_session_proposal(request_id.clone())
                    .await
                    .map_err(|error| error.to_string())
                    .and_then(|result| result.map_err(|error| error.to_string()))
            };
            this.update(cx, |this, cx| {
                let Some(review) = this.pending_review_mut(&request_id) else {
                    return;
                };
                match outcome {
                    Ok(()) => {
                        if let Some(responder) = review.responder.take() {
                            let _ = responder.send(approved);
                        }
                        this.edit_review = None;
                        if approved && let Some(selected) = this.selected.clone() {
                            this.reload_note(selected, cx);
                        }
                    }
                    Err(error) => {
                        review.busy = false;
                        review.error = Some(error);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `PendingProposalsBanner`: "N pending edit(s)" with a `Review memo` /
    /// `Review summary` outline button per proposal, above the note body.
    pub(super) fn render_pending_proposals_banner(
        &self,
        preview: &crate::db::NotePreview,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        if preview.pending_proposals.is_empty() {
            return None;
        }
        let theme = self.theme;
        let count = preview.pending_proposals.len();
        let label = if count == 1 {
            "1 pending edit".to_string()
        } else {
            format!("{count} pending edits")
        };
        Some(
            div()
                .flex_shrink_0()
                .px_1()
                .pt_1()
                .pb_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .rounded(px(22.0))
                        .border_1()
                        .border_color(alpha(theme.border, 0.7))
                        .bg(alpha(theme.card, 0.8))
                        .px_3()
                        .py_2()
                        .child(
                            div()
                                .text_size(px(13.0))
                                .line_height(px(20.0))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(theme.foreground)
                                .child(SharedString::from(label)),
                        )
                        .child(div().flex().flex_wrap().justify_end().gap_2().children(
                            preview.pending_proposals.iter().enumerate().map(
                                |(index, (id, kind))| {
                                    // `Button size="sm" variant="outline"`, one per
                                    // proposal (`small_button` keys hover by a static id).
                                    let proposal_id = id.clone();
                                    div()
                                        .id(("review-proposal", index))
                                        .flex()
                                        .h(px(28.0))
                                        .flex_shrink_0()
                                        .items_center()
                                        .px_2()
                                        .rounded(px(8.0))
                                        .bg(theme.background)
                                        .border_1()
                                        .border_color(theme.border)
                                        .shadow_xs()
                                        .tw_text_xs()
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(theme.foreground)
                                        .cursor_pointer()
                                        .hover(move |style| style.bg(theme.accent))
                                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation()
                                        })
                                        .on_click(cx.listener(
                                            move |this, _: &gpui::ClickEvent, _, cx| {
                                                this.open_proposal_review(proposal_id.clone(), cx)
                                            },
                                        ))
                                        .child(if kind == "memo_replace" {
                                            "Review memo"
                                        } else {
                                            "Review summary"
                                        })
                                },
                            ),
                        )),
                )
                .into_any_element(),
        )
    }

    /// `TabContentEdit`
    pub(super) fn render_edit_review(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let notice = |text: &'static str| {
            div()
                .flex()
                .h_full()
                .items_center()
                .justify_center()
                .tw_text_sm()
                .text_color(theme.muted_foreground)
                .child(text)
                .into_any_element()
        };
        let review = match self.edit_review.as_ref() {
            None => return div().into_any_element(),
            Some(EditReview::Loading { .. }) => return notice("Loading edit…"),
            Some(EditReview::Missing) => return notice("This edit is no longer pending."),
            Some(EditReview::Pending(review)) => review,
        };
        let is_memo = review.target == EditTarget::Memo;
        let request_id = review.request_id.clone();
        let busy = review.busy;
        let error = review.error.clone();
        // `useSessionSummary(sessionId)` / `useEnhancedNote(enhancedNoteId)`
        let session_title = self
            .session_rows
            .iter()
            .find(|row| row.id == review.session_id)
            .map(|row| row.title.trim().to_string())
            .filter(|title| !title.is_empty());
        let summary_title = match (&review.target, &self.note) {
            (EditTarget::Summary { enhanced_note_id }, super::Note::Ready { preview, .. })
                if preview.session.id == review.session_id =>
            {
                preview
                    .enhanced
                    .iter()
                    .find(|note| note.id == *enhanced_note_id)
                    .map(|note| note.title.trim().to_string())
                    .filter(|title| !title.is_empty())
            }
            _ => None,
        };
        let file_name = if is_memo { "memo.md" } else { "summary.md" };
        let decline_id = request_id.clone();
        let apply_id = request_id.clone();
        // `border-border flex items-start justify-between gap-3 border-b px-4 py-3`
        let header = div()
            .flex()
            .items_start()
            .justify_between()
            .gap_3()
            .border_b_1()
            .border_color(theme.border)
            .px_4()
            .py_3()
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .line_height(px(19.0))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .child(SharedString::from(
                                session_title.unwrap_or_else(|| "Untitled session".to_string()),
                            )),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .line_height(px(18.0))
                            .text_color(theme.muted_foreground)
                            .child(SharedString::from(if is_memo {
                                "Memo".to_string()
                            } else {
                                summary_title.unwrap_or_else(|| "Summary".to_string())
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .gap_2()
                    .child(self.small_button(
                        SmallButton {
                            id: "edit-review-decline",
                            outline: true,
                            glyph: None,
                            label: "Decline",
                            disabled: busy,
                            on_click: Some(Rc::new(move |this, _, cx| {
                                this.review_edit(&decline_id, false, cx)
                            })),
                        },
                        cx,
                    ))
                    .child(self.small_button(
                        SmallButton {
                            id: "edit-review-apply",
                            outline: false,
                            glyph: None,
                            label: if is_memo {
                                "Apply to memo"
                            } else {
                                "Apply to summary"
                            },
                            disabled: busy,
                            on_click: Some(Rc::new(move |this, _, cx| {
                                this.review_edit(&apply_id, true, cx)
                            })),
                        },
                        cx,
                    )),
            );
        let diff = self.render_unified_diff(
            file_name,
            &review.current_content,
            &review.proposed_content,
            window,
        );
        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .child(header)
            .when_some(error, |column, error| {
                // `border-red-200 bg-red-50 px-4 py-2 text-[13px] text-red-600`
                // (the border colour without a border width).
                column.child(
                    div()
                        .bg(gpui::rgb(0xfef2f2))
                        .px_4()
                        .py_2()
                        .text_size(px(13.0))
                        .line_height(px(20.0))
                        .text_color(gpui::rgb(0xe7000b))
                        .child(SharedString::from(error)),
                )
            })
            .child(
                // `overflow: "scroll"`: lines keep `white-space: pre` and the
                // code scrolls sideways instead of wrapping.
                div()
                    .id("edit-review-diff")
                    .flex_1()
                    .min_h_0()
                    .overflow_scroll()
                    .child(diff),
            )
            .into_any_element()
    }

    /// `MultiFileDiff` (`@pierre/diffs`, unified, `pierre-light` / `-dark`):
    /// the 44px file header (modified-file icon, name, `-N +M`), then 20px
    /// rows of 13px mono text — a 4px change bar, the right-aligned number
    /// column (`2ch` + digits + `1ch`), a 2px gap, and the line with `1ch`
    /// padding — coloured by the theme's markdown scopes, changed words
    /// emphasised, and `No newline at end of file` notes.
    fn render_unified_diff(
        &self,
        file_name: &'static str,
        old: &str,
        new: &str,
        window: &Window,
    ) -> AnyElement {
        use crate::unified_diff::{DiffRow, LineKind};
        let palette = if self.theme.dark {
            crate::unified_diff::DARK
        } else {
            crate::unified_diff::LIGHT
        };
        let dark = self.theme.dark;
        let fg = gpui::rgb(palette.fg);
        let bg = gpui::rgb(palette.bg);
        let addition = gpui::rgb(0x00cab1);
        let deletion = gpui::rgb(0xff2e3f);
        let modified = gpui::rgb(0x009fff);
        // `color-mix(in lab, bg 88% | 91%, base)` (dark: 80% | 85%) and
        // `rgb(from base r g b / 0.15)` (dark: 0.2) over the line, mixed as
        // WebKit paints them.
        let (bg_deletion, bg_deletion_number, bg_addition, bg_addition_number) = if dark {
            (
                gpui::rgb(0x371816),
                gpui::rgb(0x2c1513),
                gpui::rgb(0x172c28),
                gpui::rgb(0x152420),
            )
        } else {
            (
                gpui::rgb(0xffeae6),
                gpui::rgb(0xffefec),
                gpui::rgb(0xeaf9f5),
                gpui::rgb(0xeffbf8),
            )
        };
        let (emphasis_deletion, emphasis_addition) = if dark {
            (gpui::rgb(0x5f1c1e), gpui::rgb(0x124c43))
        } else {
            (gpui::rgb(0xffcecd), gpui::rgb(0xc7f2eb))
        };
        // `--diffs-fg-number`: `color-mix(in lab, fg 65%, bg)`.
        let fg_number = if dark {
            gpui::rgb(0x9d9d9d)
        } else {
            gpui::rgb(0x555555)
        };
        // `--diffs-bg-separator`: `color-mix(in lab, bg 96% | 85%, mixer)`.
        let bg_separator = if dark {
            gpui::rgb(0x292929)
        } else {
            gpui::rgb(0xf3f3f3)
        };
        const FONT_PX: f32 = 13.0;
        const LINE_PX: f32 = 20.0;
        let ch = FONT_PX * 0.6;
        let diff = crate::unified_diff::diff(old, new);
        let number_width = ch * (3 + diff.number_digits) as f32;

        let mut mono = window.text_style();
        mono.font_size = px(FONT_PX).into();
        mono.line_height = px(LINE_PX).into();
        mono.color = fg.into();
        if let Some(family) = &self.diff_font_family {
            mono.font_family = family.clone();
        }

        // `[data-diffs-header='default']`: `min-height: 1lh + 3 * 8px`,
        // `padding-inline: 16px`, the file icon and name left, the counts right.
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .h(px(44.0))
            .px_4()
            .bg(bg)
            .text_size(px(FONT_PX))
            .line_height(px(LINE_PX))
            .text_color(fg)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_w_0()
                    .child(
                        // The `modified` file icon: a ring with a centred dot.
                        div()
                            .flex()
                            .size(px(16.0))
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .border(px(1.5))
                            .border_color(modified)
                            .child(div().size(px(6.0)).rounded_full().bg(modified)),
                    )
                    .child(div().truncate().child(file_name)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(ch))
                    .flex_shrink_0()
                    .when_some(self.diff_font_family.clone(), |meta, family| {
                        meta.font_family(family)
                    })
                    .when(diff.stats.deletions > 0, |meta| {
                        meta.child(
                            div()
                                .text_color(deletion)
                                .child(SharedString::from(format!("-{}", diff.stats.deletions))),
                        )
                    })
                    .when(diff.stats.additions > 0, |meta| {
                        meta.child(
                            div()
                                .text_color(addition)
                                .child(SharedString::from(format!("+{}", diff.stats.additions))),
                        )
                    }),
            );

        let mut body = div()
            .flex()
            .flex_col()
            .min_w_full()
            .bg(bg)
            .pb(px(2.0))
            .child(header);
        if diff.rows.is_empty() {
            body = body.child(
                div()
                    .px_4()
                    .py_3()
                    .text_size(px(12.0))
                    .text_color(self.theme.muted_foreground)
                    .child("No changes"),
            );
        }
        let gutter = |kind: Option<LineKind>, number: Option<usize>| {
            let (number_bg, color) = match kind {
                Some(LineKind::Delete) => (bg_deletion_number, deletion),
                Some(LineKind::Insert) => (bg_addition_number, addition),
                Some(LineKind::Context) => (bg, fg_number),
                // The no-newline row keeps the line background under its gutter.
                None => (gpui::rgba(0x00000000), fg_number),
            };
            div()
                .relative()
                .flex()
                .flex_shrink_0()
                .h(px(LINE_PX))
                .w(px(number_width))
                .bg(number_bg)
                .justify_end()
                .pr(px(ch))
                .text_size(px(FONT_PX))
                .line_height(px(LINE_PX))
                .when_some(self.diff_font_family.clone(), |gutter, family| {
                    gutter.font_family(family)
                })
                .text_color(color)
                .when_some(number, |gutter, number| {
                    gutter.child(SharedString::from(number.to_string()))
                })
                // `[data-indicators='bars']`: the 4px bar at the gutter's left,
                // solid for additions, 2px dotted for deletions.
                .when(matches!(kind, Some(LineKind::Insert)), |gutter| {
                    gutter.child(
                        div()
                            .absolute()
                            .left_0()
                            .top_0()
                            .w(px(4.0))
                            .h(px(LINE_PX))
                            .bg(addition),
                    )
                })
                .when(matches!(kind, Some(LineKind::Delete)), |gutter| {
                    gutter
                        .child(
                            div()
                                .absolute()
                                .left_0()
                                .top_0()
                                .w(px(4.0))
                                .h(px(LINE_PX))
                                .bg(bg_deletion),
                        )
                        .children((0..(LINE_PX as usize / 2)).map(|step| {
                            div()
                                .absolute()
                                .left_0()
                                .top(px(step as f32 * 2.0))
                                .w(px(4.0))
                                .h(px(1.0))
                                .bg(deletion)
                        }))
                })
        };
        for row in diff.rows {
            body = body.child(match row {
                DiffRow::Separator(text) => div()
                    .flex()
                    .items_center()
                    .h(px(32.0))
                    .px(px(ch))
                    .bg(bg_separator)
                    .text_size(px(FONT_PX))
                    .line_height(px(LINE_PX))
                    .text_color(fg_number)
                    .when_some(self.diff_font_family.clone(), |row, family| {
                        row.font_family(family)
                    })
                    .child(SharedString::from(text)),
                DiffRow::Line {
                    kind,
                    number,
                    text,
                    emphasis,
                } => {
                    let (line_bg, emphasis_bg) = match kind {
                        LineKind::Delete => (bg_deletion, emphasis_deletion),
                        LineKind::Insert => (bg_addition, emphasis_addition),
                        LineKind::Context => (bg, bg),
                    };
                    let highlights: Vec<(std::ops::Range<usize>, gpui::HighlightStyle)> =
                        crate::unified_diff::markdown_tokens(&text, &palette)
                            .into_iter()
                            .map(|token| {
                                (
                                    token.range,
                                    gpui::HighlightStyle {
                                        color: Some(gpui::rgb(token.color).into()),
                                        font_style: token.italic.then_some(gpui::FontStyle::Italic),
                                        ..Default::default()
                                    },
                                )
                            })
                            .collect();
                    // `[data-diff-span]`: `border-radius: 3px` boxes the height
                    // of the inline text (17px of the 20px line) behind the
                    // changed words; a cell per character in the mono font.
                    let column = |byte: usize| text[..byte].chars().count() as f32;
                    let emphasis_boxes: Vec<AnyElement> = emphasis
                        .iter()
                        .map(|range| {
                            let start = column(range.start);
                            let end = column(range.end);
                            div()
                                .absolute()
                                .left(px(ch * (1.0 + start) + 0.5))
                                .top(px(1.0))
                                .w(px(ch * (end - start)))
                                .h(px(17.0))
                                .rounded(px(3.0))
                                .bg(emphasis_bg)
                                .into_any_element()
                        })
                        .collect();
                    let content: SharedString = if text.is_empty() {
                        " ".into()
                    } else {
                        text.clone().into()
                    };
                    div()
                        .flex()
                        .w_full()
                        .min_h(px(LINE_PX))
                        .bg(line_bg)
                        .child(gutter(Some(kind), Some(number)))
                        .child(div().w(px(2.0)).flex_shrink_0().bg(bg))
                        .child(
                            div()
                                .relative()
                                .flex_1()
                                .flex_shrink_0()
                                .whitespace_nowrap()
                                .px(px(ch))
                                .text_size(px(FONT_PX))
                                .line_height(px(LINE_PX))
                                .when_some(self.diff_font_family.clone(), |line, family| {
                                    line.font_family(family)
                                })
                                .children(emphasis_boxes)
                                .child(
                                    gpui::StyledText::new(content)
                                        .with_default_highlights(&mono, highlights),
                                ),
                        )
                }
                DiffRow::NoNewline(kind) => {
                    let line_bg = match kind {
                        LineKind::Delete => bg_deletion,
                        LineKind::Insert => bg_addition,
                        LineKind::Context => bg,
                    };
                    div()
                        .flex()
                        .w_full()
                        .h(px(LINE_PX))
                        .bg(line_bg)
                        .child(gutter(None, None))
                        .child(div().w(px(2.0)).flex_shrink_0().bg(bg))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .px(px(ch))
                                .text_size(px(FONT_PX))
                                .line_height(px(LINE_PX))
                                .text_color(alpha(fg, 0.6))
                                .when_some(self.diff_font_family.clone(), |line, family| {
                                    line.font_family(family)
                                })
                                .child("No newline at end of file"),
                        )
                }
            });
        }
        body.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_proposals_auto_decline_when_the_review_closes() {
        assert!(should_auto_decline("chat"));
        assert!(!should_auto_decline("cli"));
        assert!(!should_auto_decline("mcp"));
    }
}
