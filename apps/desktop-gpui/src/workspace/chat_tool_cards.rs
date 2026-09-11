//! `chat/components/message/tool/*`: the tool parts inside an assistant
//! bubble — the `Disclosure` shell, the search card for `list_meetings` /
//! `search_meetings`, the edit card for `edit_memo` / `edit_summary`, and
//! the generic card for everything else.

use std::rc::Rc;

use gpui::{AnyElement, ClickEvent, Context, SharedString, div, prelude::*, px};

use super::Workspace;
use super::automations_tab::SmallButton;
use crate::chat::ToolView;
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon};

/// `formatToolName`: underscores to spaces, first letter capitalised.
pub fn format_tool_name(name: &str) -> String {
    let mut text = name.replace('_', " ");
    if let Some(first) = text.get(..1) {
        let upper = first.to_uppercase();
        text.replace_range(..1, &upper);
    }
    text
}

/// `formatSearchInput`: the title's query and the detail lines.
pub fn format_search_input(input: Option<&serde_json::Value>) -> (String, Vec<String>) {
    let Some(input) = input else {
        return ("meetings".to_string(), Vec::new());
    };
    let mut details = Vec::new();
    let raw_query = input
        .get("query")
        .and_then(|q| q.as_str())
        .map(str::trim)
        .unwrap_or_default();
    let title_query = if raw_query.is_empty() {
        "meetings".to_string()
    } else {
        raw_query.to_string()
    };
    if raw_query.is_empty() {
        details.push("Query: none".to_string());
    } else {
        details.push(format!("Query: {raw_query}"));
    }
    if let Some(created_at) = input.get("filters").and_then(|f| f.get("created_at")) {
        match created_at.get("kind").and_then(|k| k.as_str()) {
            Some("relative") => {
                let days = created_at
                    .get("recent_days")
                    .and_then(|d| d.as_i64())
                    .unwrap_or(0);
                details.push(format!("Date: recent {days} day(s), including today"));
            }
            Some("absolute") => {
                let bounds: Vec<String> = ["gte", "lte", "gt", "lt", "eq"]
                    .iter()
                    .filter_map(|key| {
                        created_at
                            .get(key)
                            .and_then(|v| v.as_f64())
                            .map(|ms| format!("{key} {}", locale_string(ms as i64)))
                    })
                    .collect();
                if !bounds.is_empty() {
                    details.push(format!("Date: {}", bounds.join(", ")));
                }
            }
            _ => {}
        }
    }
    if let Some(limit) = input.get("limit").and_then(|l| l.as_i64()) {
        details.push(format!("Limit: {limit}"));
    }
    (title_query, details)
}

/// `new Date(ms).toLocaleString()` in the `en-US` shape, local time.
pub fn locale_string(ms: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)
        .map(|time| {
            time.with_timezone(&chrono::Local)
                .format("%-m/%-d/%Y, %-I:%M:%S %p")
                .to_string()
        })
        .unwrap_or_default()
}

/// A meeting result of `list_meetings` / `search_meetings` (`parseMeetingSearchResults`).
#[derive(Debug, Clone, PartialEq)]
pub struct MeetingResult {
    pub id: String,
    pub title: String,
    pub excerpt: String,
    /// Epoch millis or an ISO string, whichever the output carried.
    pub created_at: Option<String>,
}

pub fn parse_meeting_results(output: Option<&serde_json::Value>) -> Vec<MeetingResult> {
    let Some(output) = output else {
        return Vec::new();
    };
    let results = output
        .get("results")
        .and_then(|r| r.as_array())
        .or_else(|| output.get("meetings").and_then(|m| m.as_array()));
    results
        .into_iter()
        .flatten()
        .filter_map(|result| {
            let id = result.get("id")?.as_str()?.to_string();
            let started_at = result
                .get("started_at")
                .and_then(|s| s.as_str())
                .filter(|s| !s.is_empty());
            let created_at = match started_at {
                Some(started) => Some(started.to_string()),
                None => match result.get("created_at") {
                    Some(serde_json::Value::Number(number)) => {
                        number.as_i64().filter(|n| *n != 0).map(|n| n.to_string())
                    }
                    Some(serde_json::Value::String(text)) if !text.is_empty() => Some(text.clone()),
                    _ => None,
                },
            };
            Some(MeetingResult {
                id,
                title: result
                    .get("title")
                    .and_then(|t| t.as_str())
                    .unwrap_or("Untitled")
                    .to_string(),
                excerpt: result
                    .get("excerpt")
                    .and_then(|e| e.as_str())
                    .unwrap_or_default()
                    .to_string(),
                created_at,
            })
        })
        .collect()
}

/// The result card's date: epoch millis or an ISO string through
/// `toLocaleString()`.
pub fn result_date_label(created_at: &str) -> Option<String> {
    if let Ok(ms) = created_at.parse::<i64>() {
        return Some(locale_string(ms));
    }
    chrono::DateTime::parse_from_rfc3339(created_at)
        .ok()
        .map(|time| locale_string(time.timestamp_millis()))
}

/// `formatOutputText`: the output as pretty JSON without `contextText`.
pub fn format_output_text(output: Option<&serde_json::Value>) -> Option<String> {
    let output = output?;
    match output {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Null => None,
        serde_json::Value::Object(map) => {
            let mut rest = map.clone();
            rest.remove(crate::chat_tools::CONTEXT_TEXT_FIELD);
            serde_json::to_string_pretty(&serde_json::Value::Object(rest)).ok()
        }
        other => serde_json::to_string_pretty(other).ok(),
    }
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

impl Workspace {
    /// The tool part's card; `key` scopes the element ids.
    pub(super) fn render_tool_part(
        &self,
        tool: &ToolView<'_>,
        key: (usize, usize),
        renderer: &super::document_view::DocumentRenderer,
        cx: &Context<Self>,
    ) -> AnyElement {
        if matches!(tool.name, "edit_memo" | "edit_summary") {
            return self.render_edit_tool_card(tool, key, renderer, cx);
        }
        let running = matches!(tool.state, "input-streaming" | "input-available");
        let failed = tool.state == "output-error";
        let done = tool.state == "output-available";
        let call_id = tool.call_id.to_string();
        let open = self.chat.open_tools.contains(&call_id);
        let search_card = matches!(
            tool.name,
            "list_meetings" | "search_meetings" | "search_sessions"
        );
        let (glyph, title) = if search_card {
            let (title_query, _) = format_search_input(tool.input);
            let title = match tool.state {
                "input-streaming" => "Preparing search...".to_string(),
                "input-available" => format!("Searching for: {title_query}"),
                "output-available" => format!("Searched for: {title_query}"),
                "output-error" => {
                    if tool.input.is_some() {
                        format!("Search failed: {title_query}")
                    } else {
                        "Search failed".to_string()
                    }
                }
                _ => "Search".to_string(),
            };
            ("magnifying-glass", title)
        } else {
            let name = format_tool_name(tool.name);
            let title = if failed {
                format!("{name} failed")
            } else if done {
                name
            } else {
                format!("Running {name}…")
            };
            ("wrench", title)
        };
        // The generic card is a spinner-only summary while running.
        let disabled = if search_card {
            running
        } else {
            !(done || failed)
        };
        let body: Option<AnyElement> = if search_card {
            Some(self.render_search_tool_body(tool, key, cx))
        } else if done || failed {
            Some(self.render_generic_tool_body(tool))
        } else {
            None
        };
        self.render_disclosure(key, glyph, title, disabled, open, call_id, body, cx)
    }

    /// `Disclosure`: `my-2 rounded-md border px-2 py-1`, the summary row with
    /// the spinner or icon (a `"spinner"` glyph shows the spinner on an
    /// enabled disclosure, like the running `Activity` row), the title, the
    /// caret; the body under a top border while open.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_disclosure(
        &self,
        key: (usize, usize),
        glyph: &'static str,
        title: String,
        disabled: bool,
        open: bool,
        call_id: String,
        body: Option<AnyElement>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let open = open && !disabled;
        let summary = div()
            .id(("chat-tool-summary", key.0 * 1000 + key.1))
            .flex()
            .w_full()
            .items_center()
            .gap_2()
            .tw_text_xs()
            .text_color(theme.muted_foreground)
            .when(!disabled, |summary| summary.cursor_pointer())
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if disabled {
                    return;
                }
                if !this.chat.open_tools.remove(&call_id) {
                    this.chat.open_tools.insert(call_id.clone());
                }
                cx.notify();
            }))
            .child(if disabled || glyph == "spinner" {
                crate::ui::spinner(
                    ("chat-tool-spinner", key.0 * 1000 + key.1),
                    px(12.0),
                    theme.muted_foreground,
                )
                .into_any_element()
            } else {
                icon(glyph, px(12.0), theme.muted_foreground).into_any_element()
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .when(open, |title| title.font_weight(gpui::FontWeight::MEDIUM))
                    .child(SharedString::from(title)),
            )
            .child(div().flex_shrink_0().child(icon(
                if open { "caret-down" } else { "caret-right" },
                px(12.0),
                theme.muted_foreground,
            )));
        div()
            .my_2()
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.border)
            .px_2()
            .py_1()
            .child(summary)
            .when(open, |details| {
                details.child(
                    div()
                        .mt_1()
                        .border_t_1()
                        .border_color(theme.border)
                        .px_1()
                        .pt_2()
                        .children(body),
                )
            })
            .into_any_element()
    }

    /// `ToolSearchMeetings`' body: the detail lines, then the result card
    /// carousel (one `basis-full` card per view at the panel's width, the
    /// arrows at its edges), `No results found`, or the error.
    fn render_search_tool_body(
        &self,
        tool: &ToolView<'_>,
        key: (usize, usize),
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let (_, details) = format_search_input(tool.input);
        let details_block = (!details.is_empty()).then(|| {
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .text_size(px(11.0))
                .line_height(px(16.0))
                .text_color(theme.muted_foreground)
                .children(details.into_iter().map(SharedString::from))
        });
        match tool.state {
            "output-available" => {
                let results = parse_meeting_results(tool.output);
                if results.is_empty() {
                    return div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .children(details_block)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .py_2()
                                .tw_text_xs()
                                .text_color(theme.muted_foreground)
                                .child("No results found"),
                        )
                        .into_any_element();
                }
                let call_id = tool.call_id.to_string();
                let page = self
                    .chat
                    .tool_pages
                    .get(&call_id)
                    .copied()
                    .unwrap_or(0)
                    .min(results.len().saturating_sub(1));
                // `basis-full`: one card per view at the panel's width.
                let visible: Vec<MeetingResult> =
                    results.iter().skip(page).take(1).cloned().collect();
                let can_prev = page > 0;
                let can_next = page + 1 < results.len();
                let arrow = |id: &'static str, glyph: &'static str, enabled: bool, delta: isize| {
                    let call_id = call_id.clone();
                    div()
                        .id((id, key.0 * 1000 + key.1))
                        .absolute()
                        .top(px(0.0))
                        .bottom(px(0.0))
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .flex()
                                .size(px(24.0))
                                .items_center()
                                .justify_center()
                                .rounded_full()
                                .border_1()
                                .border_color(theme.border)
                                .bg(theme.muted)
                                .when(!enabled, |arrow| arrow.opacity(0.5))
                                .when(enabled, |arrow| {
                                    arrow
                                        .cursor_pointer()
                                        .hover(move |style| style.bg(theme.accent))
                                })
                                .child(icon(glyph, px(12.0), theme.foreground)),
                        )
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if !enabled {
                                return;
                            }
                            let current =
                                this.chat.tool_pages.get(&call_id).copied().unwrap_or(0) as isize;
                            this.chat
                                .tool_pages
                                .insert(call_id.clone(), (current + delta).max(0) as usize);
                            cx.notify();
                        }))
                };
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(details_block)
                    .child(
                        div()
                            .relative()
                            .mx(px(-4.0))
                            .child(div().flex().w_full().gap(px(4.0)).px(px(4.0)).children(
                                visible.into_iter().enumerate().map(|(offset, result)| {
                                    let session_id = result.id.clone();
                                    let date =
                                        result.created_at.as_deref().and_then(result_date_label);
                                    // `Card bg-muted` with `px-2 py-0.5`: title, date, excerpt.
                                    div()
                                        .id((
                                            "chat-tool-meeting",
                                            key.0 * 1000 + key.1 * 10 + offset,
                                        ))
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .flex_1()
                                        .min_w_0()
                                        .rounded(px(12.0))
                                        .border_1()
                                        .border_color(theme.border)
                                        .bg(theme.muted)
                                        .px_2()
                                        .py(px(2.0))
                                        .tw_text_xs()
                                        .cursor_pointer()
                                        .on_click(cx.listener(
                                            move |this, _: &ClickEvent, _, cx| {
                                                this.open_new(session_id.clone(), cx);
                                            },
                                        ))
                                        .child(
                                            div()
                                                .truncate()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .text_color(theme.foreground)
                                                .child(SharedString::from(
                                                    if result.title.is_empty() {
                                                        "Untitled".to_string()
                                                    } else {
                                                        result.title.clone()
                                                    },
                                                )),
                                        )
                                        .children(date.map(|date| {
                                            div()
                                                .text_size(px(11.0))
                                                .line_height(px(16.0))
                                                .text_color(theme.muted_foreground)
                                                .child(SharedString::from(date))
                                        }))
                                        .child(
                                            div()
                                                .text_color(theme.muted_foreground)
                                                .line_clamp(3)
                                                .child(SharedString::from(
                                                    if result.excerpt.is_empty() {
                                                        "No excerpt available".to_string()
                                                    } else {
                                                        result.excerpt.clone()
                                                    },
                                                )),
                                        )
                                }),
                            ))
                            .when(can_prev, |carousel| {
                                carousel.child(
                                    arrow("chat-tool-prev", "arrow-left", true, -1).left(px(-16.0)),
                                )
                            })
                            .when(can_next, |carousel| {
                                carousel.child(
                                    arrow("chat-tool-next", "arrow-right", true, 1)
                                        .right(px(-16.0)),
                                )
                            }),
                    )
                    .into_any_element()
            }
            "output-error" => div()
                .tw_text_sm()
                .text_color(gpui::rgb(0xfb2c36))
                .child(SharedString::from(format!(
                    "Error: {}",
                    tool.error_text.unwrap_or_default()
                )))
                .into_any_element(),
            _ => div().children(details_block).into_any_element(),
        }
    }

    /// `ToolEditMemo` / `ToolEditSummary` (`defineTool`): the `ToolCard`
    /// (`my-2.5 rounded-xl border shadow-sm`) with the `ToolCardHeader`
    /// (spinner while running, the pencil in emerald once applied, the label
    /// per state), the `MarkdownPreview` of the proposed content, the error
    /// footer (and the summary candidates), and `Decline` / `Apply to …`
    /// while the proposal waits on this review.
    fn render_edit_tool_card(
        &self,
        tool: &ToolView<'_>,
        key: (usize, usize),
        renderer: &super::document_view::DocumentRenderer,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let is_memo = tool.name == "edit_memo";
        let running = matches!(tool.state, "input-streaming" | "input-available");
        let failed = tool.state == "output-error";
        let done = tool.state == "output-available";
        let status = done.then(|| tool.output?.get("status")?.as_str()).flatten();
        let message = tool
            .output
            .and_then(|output| output.get("message"))
            .and_then(|message| message.as_str());
        let candidates: Vec<String> = tool
            .output
            .and_then(|output| output.get("candidates"))
            .and_then(|candidates| candidates.as_array())
            .map(|candidates| {
                candidates
                    .iter()
                    .map(|candidate| {
                        format!(
                            "{} ({})",
                            candidate
                                .get("title")
                                .and_then(|t| t.as_str())
                                .unwrap_or_default(),
                            candidate
                                .get("enhancedNoteId")
                                .and_then(|id| id.as_str())
                                .unwrap_or_default()
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let target = if is_memo { "memo" } else { "summary" };
        let label = if running {
            format!("Edit {target} — review tab opened")
        } else if failed {
            format!("{} edit failed", capitalize(target))
        } else {
            match status {
                Some("applied") => format!("{} updated", capitalize(target)),
                Some("declined") => format!("{} edit declined", capitalize(target)),
                _ => format!("Edit {target}"),
            }
        };
        let applied = status == Some("applied");
        // `EditActions`: only while the tool is waiting on this review.
        let pending = matches!(
            &self.edit_review,
            Some(super::edit_review::EditReview::Pending(review))
                if review.request_id == tool.call_id && review.responder.is_some()
        );
        let red_200 = gpui::rgb(0xffc9c9);
        let red_50 = gpui::rgb(0xfef2f2);
        let red_500 = gpui::rgb(0xfb2c36);
        let red_600 = gpui::rgb(0xe7000b);
        let red_700 = gpui::rgb(0xc10007);
        let emerald_500 = gpui::rgb(0x00bc7d);
        let header_icon: AnyElement = if running {
            crate::ui::spinner(
                ("chat-tool-spinner", key.0 * 1000 + key.1),
                px(16.0),
                theme.muted_foreground,
            )
            .into_any_element()
        } else {
            icon(
                "pencil",
                px(16.0),
                if failed {
                    red_500
                } else if applied {
                    emerald_500
                } else {
                    theme.muted_foreground
                },
            )
            .into_any_element()
        };
        // `flex items-center gap-2.5 px-3.5 py-2 text-[13px]`
        let header = div()
            .flex()
            .items_center()
            .gap(px(10.0))
            .px(px(14.0))
            .py_2()
            .text_size(px(13.0))
            .line_height(px(18.0))
            .map(|header| {
                if failed {
                    header.bg(red_50).text_color(red_700)
                } else {
                    header
                        .bg(alpha(theme.muted, 0.8))
                        .text_color(theme.muted_foreground)
                }
            })
            .child(header_icon)
            .child(
                div()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(SharedString::from(label)),
            );
        let content = tool
            .input
            .and_then(|input| input.get("content"))
            .and_then(|content| content.as_str())
            .filter(|content| !content.is_empty());
        // `ToolCardBody` (`px-3.5 py-2.5`) > `MarkdownPreview`: `rounded-lg
        // border bg-card` around `max-h-64 overflow-y-auto px-3 py-2.5`.
        let body = content.map(|content| {
            let blocks = crate::document::from_body("markdown", content);
            div().px(px(14.0)).py(px(10.0)).child(
                div()
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(alpha(theme.border, 0.8))
                    .bg(theme.card)
                    .overflow_hidden()
                    .child(
                        div()
                            .id(("chat-edit-preview", key.0 * 1000 + key.1))
                            .max_h(px(256.0))
                            .overflow_y_scroll()
                            .px_3()
                            .py(px(10.0))
                            .flex()
                            .flex_col()
                            .children(renderer.preview_blocks(&blocks, theme.muted_foreground)),
                    ),
            )
        });
        let error_footer = |text: String| {
            // `ToolCardFooterError`
            div()
                .flex()
                .items_center()
                .gap_2()
                .border_t_1()
                .border_color(red_200)
                .bg(red_50)
                .px(px(14.0))
                .py(px(10.0))
                .child(icon("x-circle", px(16.0), red_500))
                .child(
                    div()
                        .text_size(px(13.0))
                        .line_height(px(18.0))
                        .text_color(red_600)
                        .child(SharedString::from(text)),
                )
        };
        let call_id = tool.call_id.to_string();
        let decline_id = call_id.clone();
        let apply_id = call_id;
        // The card is transparent in the web view; GPUI paints the shadow
        // under the element, so it takes the panel's surface colour.
        let surface = if self.chat_in_right_panel() {
            theme.card
        } else if theme.dark {
            gpui::rgb(0x202020)
        } else {
            gpui::rgb(0xf4f4f5)
        };
        div()
            .my(px(10.0))
            .rounded(px(12.0))
            .bg(surface)
            .border_1()
            .border_color(if failed {
                red_200
            } else {
                alpha(theme.border, 0.8)
            })
            .overflow_hidden()
            // Tailwind v4 `shadow-sm`.
            .shadow(vec![
                gpui::BoxShadow {
                    color: gpui::hsla(0.0, 0.0, 0.0, 0.1),
                    offset: gpui::point(px(0.0), px(1.0)),
                    blur_radius: px(3.0),
                    spread_radius: px(0.0),
                },
                gpui::BoxShadow {
                    color: gpui::hsla(0.0, 0.0, 0.0, 0.1),
                    offset: gpui::point(px(0.0), px(1.0)),
                    blur_radius: px(2.0),
                    spread_radius: px(-1.0),
                },
            ])
            .child(header)
            .children(body)
            .when(status == Some("error"), |card| {
                card.child(error_footer(message.unwrap_or("Unknown error").to_string()))
                    .when(!candidates.is_empty(), |card| {
                        // The summary candidates under the error.
                        card.child(
                            div().px(px(14.0)).pb(px(10.0)).child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .rounded(px(6.0))
                                    .border_1()
                                    .border_color(theme.border)
                                    .bg(theme.muted)
                                    .p_2()
                                    .text_size(px(12.0))
                                    .text_color(theme.muted_foreground)
                                    .children(candidates.into_iter().map(|candidate| {
                                        div().child(SharedString::from(candidate))
                                    })),
                            ),
                        )
                    })
            })
            .when(failed, |card| {
                card.child(error_footer(
                    tool.error_text.unwrap_or("Unknown error").to_string(),
                ))
            })
            .when(pending, |card| {
                // `border-border/80 flex justify-end gap-2 border-t px-3.5 py-2.5`
                card.child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .border_t_1()
                        .border_color(alpha(theme.border, 0.8))
                        .px(px(14.0))
                        .py(px(10.0))
                        .child(self.small_button(
                            SmallButton {
                                id: "chat-edit-decline",
                                outline: true,
                                glyph: None,
                                label: "Decline",
                                disabled: false,
                                on_click: Some(Rc::new(move |this, _, cx| {
                                    this.review_edit(&decline_id, false, cx)
                                })),
                            },
                            cx,
                        ))
                        .child(self.small_button(
                            SmallButton {
                                id: "chat-edit-apply",
                                outline: false,
                                glyph: None,
                                label: if is_memo {
                                    "Apply to memo"
                                } else {
                                    "Apply to summary"
                                },
                                disabled: false,
                                on_click: Some(Rc::new(move |this, _, cx| {
                                    this.review_edit(&apply_id, true, cx)
                                })),
                            },
                            cx,
                        )),
                )
            })
            .into_any_element()
    }

    /// `ToolGeneric`'s body: the `key: value` input lines, the error, the
    /// output text.
    fn render_generic_tool_body(&self, tool: &ToolView<'_>) -> AnyElement {
        let theme = self.theme;
        let failed = tool.state == "output-error";
        let output_text = (!failed).then(|| format_output_text(tool.output)).flatten();
        let inputs: Vec<(String, String)> = tool
            .input
            .and_then(|input| input.as_object())
            .map(|object| {
                object
                    .iter()
                    .map(|(key, value)| {
                        let value = match value {
                            serde_json::Value::String(text) => text.clone(),
                            other => other.to_string(),
                        };
                        (key.clone(), value)
                    })
                    .collect()
            })
            .unwrap_or_default();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .when(!inputs.is_empty(), |body| {
                body.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .tw_text_xs()
                        .text_color(theme.muted_foreground)
                        .children(inputs.into_iter().map(|(key, value)| {
                            div()
                                .child(
                                    div()
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .child(SharedString::from(format!("{key}: "))),
                                )
                                .child(SharedString::from(value))
                                .flex()
                                .flex_wrap()
                        })),
                )
            })
            .when(failed, |body| {
                body.child(div().tw_text_xs().text_color(gpui::rgb(0xfb2c36)).child(
                    SharedString::from(tool.error_text.unwrap_or("Unknown error").to_string()),
                ))
            })
            .children(output_text.map(|text| {
                div()
                    .tw_text_xs()
                    .text_color(theme.muted_foreground)
                    .whitespace_normal()
                    .child(SharedString::from(text))
            }))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_names_and_search_titles_format_like_the_frontend() {
        assert_eq!(
            format_tool_name("search_meeting_content"),
            "Search meeting content"
        );
        let (title, details) = format_search_input(Some(&serde_json::json!({
            "query": " release ",
            "filters": { "created_at": { "kind": "relative", "recent_days": 7 } },
            "limit": 5
        })));
        assert_eq!(title, "release");
        assert_eq!(
            details,
            [
                "Query: release",
                "Date: recent 7 day(s), including today",
                "Limit: 5"
            ]
        );
        let (title, details) = format_search_input(Some(&serde_json::json!({ "limit": 3 })));
        assert_eq!(title, "meetings");
        assert_eq!(details, ["Query: none", "Limit: 3"]);
        assert_eq!(format_search_input(None).0, "meetings");
    }

    #[test]
    fn meeting_results_parse_both_shapes() {
        let results = parse_meeting_results(Some(&serde_json::json!({
            "meetings": [
                { "id": "a", "title": "Weekly", "started_at": "2026-09-07T10:00:00Z", "created_at": "2026-09-06T00:00:00Z" },
                { "id": "b", "title": "", "started_at": "", "created_at": "2026-09-07T09:25:15.387Z" },
                { "title": "no id" }
            ]
        })));
        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].created_at.as_deref(),
            Some("2026-09-07T10:00:00Z")
        );
        assert_eq!(
            results[1].created_at.as_deref(),
            Some("2026-09-07T09:25:15.387Z")
        );
        let results = parse_meeting_results(Some(&serde_json::json!({
            "results": [{ "id": "c", "title": "Hit", "excerpt": "…", "score": 1.5, "created_at": 1_788_775_200_000i64 }]
        })));
        assert_eq!(results[0].created_at.as_deref(), Some("1788775200000"));
        assert!(result_date_label("1788775200000").is_some());
        assert_eq!(
            format_output_text(Some(&serde_json::json!({ "a": 1, "contextText": "x" }))).unwrap(),
            "{\n  \"a\": 1\n}"
        );
    }
}
