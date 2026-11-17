//! Shipping's chat tool rendering (`chat/components/message/{normal,shared}.tsx` and
//! `message/tool/{edit-summary,search-meetings,generic,shared}.tsx`): the Activity
//! disclosure with flat search and generic rows, and the memo/summary edit cards.

use std::sync::Arc;

use gpui::{
    AnyElement, Context, Div, SharedString, Transformation, Window, div, percentage, prelude::*,
    px, rgb,
};
use serde_json::Value;

use super::{
    ChatView, markdown,
    store::{Display, ToolPart},
    view::{icon, spinner},
};
use crate::{
    meeting::ai_view::AiViewServices,
    ui::theme::{MEDIUM, Theme},
    workspace::{edit_review, navigation::Route},
};

/// Shipping's `formatToolName`.
fn tool_title(name: &str) -> String {
    let mut text = name.replace('_', " ");
    if let Some(first) = text.get(..1) {
        text.replace_range(..1, &first.to_uppercase());
    }
    text
}

fn local_time(value: &Value) -> Option<String> {
    let at = match value {
        Value::Number(ms) => chrono::DateTime::from_timestamp_millis(ms.as_f64()? as i64)?,
        Value::String(text) => chrono::DateTime::parse_from_rfc3339(text)
            .ok()?
            .with_timezone(&chrono::Utc),
        _ => return None,
    };
    (at.timestamp_millis() != 0).then(|| {
        at.with_timezone(&chrono::Local)
            .format("%-m/%-d/%Y, %-I:%M:%S %p")
            .to_string()
    })
}

/// `formatSearchInput`: the searched text and the detail lines under the title.
fn search_input(input: &Value) -> (String, Vec<String>) {
    let query = input["query"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_owned();
    let mut details = vec![if query.is_empty() {
        "Query: none".to_owned()
    } else {
        format!("Query: {query}")
    }];
    let created = &input["filters"]["created_at"];
    match created["kind"].as_str() {
        Some("relative") => details.push(format!(
            "Date: recent {} day(s), including today",
            created["recent_days"]
        )),
        Some("absolute") => {
            let bounds = ["gte", "lte", "gt", "lt", "eq"]
                .iter()
                .filter_map(|key| local_time(&created[*key]).map(|at| format!("{key} {at}")))
                .collect::<Vec<_>>();
            if !bounds.is_empty() {
                details.push(format!("Date: {}", bounds.join(", ")));
            }
        }
        _ => {}
    }
    if let Some(limit) = input["limit"].as_i64() {
        details.push(format!("Limit: {limit}"));
    }
    let title = if query.is_empty() {
        "meetings".to_owned()
    } else {
        query
    };
    if input.is_null() {
        return ("meetings".into(), Vec::new());
    }
    (title, details)
}

/// `parseMeetingSearchResults`.
fn search_results(output: &Value) -> Vec<(String, String, String, Value)> {
    output["results"]
        .as_array()
        .or_else(|| output["meetings"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|result| {
            let id = result["id"].as_str()?.to_owned();
            let started = result["started_at"]
                .as_str()
                .filter(|at| !at.is_empty())
                .map(|at| Value::String(at.into()));
            Some((
                id,
                result["title"].as_str().unwrap_or("Untitled").to_owned(),
                result["excerpt"].as_str().unwrap_or_default().to_owned(),
                started.unwrap_or_else(|| result["created_at"].clone()),
            ))
        })
        .collect()
}

fn search_tool(name: &str) -> bool {
    matches!(
        name,
        "list_meetings" | "search_meetings" | "search_sessions"
    )
}

fn pretty(output: &Value) -> Option<String> {
    match output {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        Value::Object(object) if object.contains_key("contextText") => {
            let mut rest = object.clone();
            rest.remove("contextText");
            serde_json::to_string_pretty(&rest).ok()
        }
        value => serde_json::to_string_pretty(value).ok(),
    }
}

impl ChatView {
    /// Shipping's "Activity" disclosure: reasoning and every tool but the edit cards.
    pub(super) fn activity(
        &self,
        id: &str,
        parts: &[Display],
        streaming: bool,
        colors: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let parts = parts
            .iter()
            .filter(|part| match part {
                Display::Reasoning(text) => !text.trim().is_empty(),
                Display::Tool(tool) => !tool.edit(),
                Display::Text(_) => false,
            })
            .collect::<Vec<_>>();
        if parts.is_empty() {
            return None;
        }
        let open = self.expanded.contains(id);
        let running = streaming
            || parts
                .iter()
                .any(|part| matches!(part, Display::Tool(tool) if tool.running()));
        let toggle = id.to_owned();
        let muted = colors.muted_foreground;
        Some(
            div()
                .my_2()
                .px_2()
                .py_1()
                .border_1()
                .border_color(colors.border)
                .child(
                    div()
                        .id(SharedString::from(format!("chat-activity-{id}")))
                        .flex()
                        .items_center()
                        .gap_2()
                        .cursor_pointer()
                        .text_xs()
                        .line_height(px(16.))
                        .text_color(muted)
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.toggle_activity(&toggle, cx)),
                        )
                        .child(if running {
                            spinner("chat-activity-spinner", 12., muted).into_any_element()
                        } else {
                            icon("chat/BrainIcon.svg", 12., muted).into_any_element()
                        })
                        .child(
                            div()
                                .flex_1()
                                .truncate()
                                .when(open, |view| view.font_weight(MEDIUM))
                                .child("Activity"),
                        )
                        .child(
                            icon("chat/ArrowRight01Icon.svg", 12., muted).when(open, |icon| {
                                icon.with_transformation(Transformation::rotate(percentage(0.25)))
                            }),
                        ),
                )
                .when(open, |view| {
                    let last = parts.len() - 1;
                    view.child(
                        div()
                            .mt_1()
                            .px_1()
                            .pt_2()
                            .border_t_1()
                            .border_color(colors.border)
                            .flex()
                            .flex_col()
                            .children(parts.into_iter().enumerate().map(|(index, part)| {
                                let row = match part {
                                    Display::Reasoning(text) => div()
                                        .flex()
                                        .items_start()
                                        .gap_2()
                                        .child(if streaming && index == last {
                                            spinner("chat-reasoning-spinner", 12., muted)
                                                .into_any_element()
                                        } else {
                                            icon("chat/BrainIcon.svg", 12., muted)
                                                .mt(px(2.))
                                                .into_any_element()
                                        })
                                        .child(
                                            div()
                                                .min_w_0()
                                                .flex_1()
                                                .text_xs()
                                                .line_height(px(16.))
                                                .text_color(muted)
                                                .child(SharedString::from(text.clone())),
                                        ),
                                    Display::Tool(tool) if search_tool(&tool.name) => {
                                        self.search_row(tool, colors, window, cx)
                                    }
                                    Display::Tool(tool) => self.generic_row(tool, colors),
                                    Display::Text(_) => div(),
                                };
                                // Shipping's `my-1.5` rows collapse into a 6px gap.
                                row.when(index > 0, |row| row.mt(px(6.)))
                            })),
                    )
                })
                .into_any_element(),
        )
    }

    /// A flat `Disclosure` row: icon (or spinner while running) and the title.
    fn flat_title(
        &self,
        id: &str,
        path: &'static str,
        title: String,
        running: bool,
        colors: &Theme,
    ) -> Div {
        let muted = colors.muted_foreground;
        div()
            .flex()
            .items_center()
            .gap_2()
            .text_xs()
            .line_height(px(16.))
            .text_color(muted)
            .child(if running {
                spinner(
                    SharedString::from(format!("chat-tool-spinner-{id}")),
                    12.,
                    muted,
                )
                .into_any_element()
            } else {
                icon(path, 12., muted).into_any_element()
            })
            .child(div().min_w_0().flex_1().truncate().child(title))
    }

    /// Shipping's `ToolGeneric` inside the activity list.
    fn generic_row(&self, tool: &ToolPart, colors: &Theme) -> Div {
        let name = tool_title(&tool.name);
        if tool.running() {
            return div().child(self.flat_title(
                &tool.id,
                "chat/Wrench01Icon.svg",
                format!("Running {name}…"),
                true,
                colors,
            ));
        }
        let title = if tool.failed() {
            format!("{name} failed")
        } else {
            name
        };
        let muted = colors.muted_foreground;
        let inputs = tool
            .input
            .as_object()
            .map(|input| {
                input
                    .iter()
                    .map(|(key, value)| {
                        let value = value
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| value.to_string());
                        div()
                            .flex()
                            .flex_wrap()
                            .child(div().font_weight(MEDIUM).child(format!("{key}: ")))
                            .child(value)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let output = tool.done().then(|| pretty(&tool.output)).flatten();
        div()
            .child(self.flat_title(&tool.id, "chat/Wrench01Icon.svg", title, false, colors))
            .child(
                div()
                    .mt_1()
                    .pl_5()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .text_xs()
                    .line_height(px(16.))
                    .text_color(muted)
                    .when(!inputs.is_empty(), |view| {
                        view.child(div().flex().flex_col().gap_1().children(inputs))
                    })
                    .when(tool.failed(), |view| {
                        view.child(
                            div().text_color(rgb(0xef4444)).child(
                                tool.error.clone().unwrap_or_else(|| "Unknown error".into()),
                            ),
                        )
                    })
                    .children(output.map(|output| div().child(output))),
            )
    }

    /// Shipping's `ToolSearchMeetings`: the searched text, details and a result carousel.
    fn search_row(
        &self,
        tool: &ToolPart,
        colors: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let (query, details) = search_input(&tool.input);
        let title = match tool.state.as_str() {
            "input-streaming" => "Preparing search...".to_owned(),
            "input-available" => format!("Searching for: {query}"),
            "output-available" => format!("Searched for: {query}"),
            "output-error" if !tool.input.is_null() => format!("Search failed: {query}"),
            "output-error" => "Search failed".to_owned(),
            _ => "Search".to_owned(),
        };
        let muted = colors.muted_foreground;
        let detail_lines = (!details.is_empty()).then(|| {
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .text_size(px(11.))
                .line_height(px(16.5))
                .text_color(muted)
                .children(details.iter().map(|detail| div().child(detail.clone())))
        });
        let body = match tool.state.as_str() {
            "output-available" => {
                let results = search_results(&tool.output);
                let content = div().flex().flex_col().gap_2().children(detail_lines);
                Some(if results.is_empty() {
                    content.child(
                        div()
                            .py_2()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .line_height(px(16.))
                            .text_color(muted)
                            .child("No results found"),
                    )
                } else {
                    content.child(self.carousel(&tool.id, results, colors, window, cx))
                })
            }
            "output-error" => Some(
                div()
                    .text_sm()
                    .line_height(px(20.))
                    .text_color(rgb(0xef4444))
                    .child(format!("Error: {}", tool.error.clone().unwrap_or_default())),
            ),
            _ => detail_lines,
        };
        div()
            .child(self.flat_title(
                &tool.id,
                "chat/Search01Icon.svg",
                title,
                tool.running(),
                colors,
            ))
            .children(body.map(|body| div().mt_1().pl_5().child(body)))
    }

    /// The results carousel: one card per view below `sm`, two from `sm`, three from `lg`.
    fn carousel(
        &self,
        id: &str,
        results: Vec<(String, String, String, Value)>,
        colors: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let width = f32::from(window.viewport_size().width);
        let per_view = if width >= 1024. {
            3
        } else if width >= 640. {
            2
        } else {
            1
        };
        let max_start = results.len().saturating_sub(per_view);
        let start = self.carousels.get(id).copied().unwrap_or(0).min(max_start);
        let muted = colors.muted_foreground;
        let arrow = |path: &'static str, left: bool, target: usize| {
            let key = id.to_owned();
            div()
                .id(SharedString::from(format!(
                    "chat-carousel-{}-{id}",
                    if left { "previous" } else { "next" }
                )))
                .absolute()
                .top_0()
                .bottom_0()
                .when(left, |view| view.left(px(-16.)))
                .when(!left, |view| view.right(px(-16.)))
                .flex()
                .items_center()
                .child(
                    div()
                        .size_6()
                        .flex()
                        .items_center()
                        .justify_center()
                        .border_1()
                        .border_color(colors.input)
                        .bg(colors.muted)
                        .shadow_xs()
                        .cursor_pointer()
                        .hover(|style| style.bg(colors.accent))
                        .child(icon(path, 16., colors.foreground)),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.carousels.insert(key.clone(), target);
                    cx.notify();
                }))
        };
        div()
            .relative()
            .mx(px(-4.))
            .child(
                div()
                    .overflow_hidden()
                    .child(div().flex().ml(px(-8.)).children(
                        results.into_iter().skip(start).take(per_view).map(
                            |(session, title, excerpt, created)| {
                                let date = local_time(&created);
                                let open = session.clone();
                                div()
                                    .flex_basis(gpui::relative(1. / per_view as f32))
                                    .flex_shrink_0()
                                    .min_w_0()
                                    .pl_1()
                                    .child(
                                        div()
                                            .id(SharedString::from(format!(
                                                "chat-search-result-{session}"
                                            )))
                                            .h_full()
                                            .border_1()
                                            .border_color(colors.border)
                                            .bg(colors.muted)
                                            .shadow_xs()
                                            .px_2()
                                            .py(px(2.))
                                            .cursor_pointer()
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                let route =
                                                    Route::Session(desktop_runtime::SessionId(
                                                        open.as_str().into(),
                                                    ));
                                                let _ =
                                                    this.workspace.update(cx, |workspace, cx| {
                                                        workspace.open_route(route, true, cx)
                                                    });
                                            }))
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap_1()
                                                    .text_xs()
                                                    .line_height(px(16.))
                                                    .child(
                                                        div().truncate().font_weight(MEDIUM).child(
                                                            if title.is_empty() {
                                                                "Untitled".to_owned()
                                                            } else {
                                                                title
                                                            },
                                                        ),
                                                    )
                                                    .children(date.map(|date| {
                                                        div()
                                                            .text_size(px(11.))
                                                            .line_height(px(16.5))
                                                            .text_color(muted)
                                                            .child(date)
                                                    }))
                                                    .child(
                                                        div()
                                                            .text_color(muted)
                                                            .line_clamp(3)
                                                            .child(if excerpt.is_empty() {
                                                                "No excerpt available".to_owned()
                                                            } else {
                                                                excerpt
                                                            }),
                                                    ),
                                            ),
                                    )
                            },
                        ),
                    )),
            )
            .when(start > 0, |view| {
                view.child(arrow("chat/ArrowLeft02Icon.svg", true, start - 1))
            })
            .when(start < max_start, |view| {
                view.child(arrow("chat/ArrowRight02Icon.svg", false, start + 1))
            })
    }

    /// Shipping's `ToolEditMemo` / `ToolEditSummary` card with its review actions.
    pub(super) fn edit_card(
        &self,
        tool: &ToolPart,
        style: &markdown::Style,
        colors: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let memo = tool.name == "edit_memo";
        let status = tool.output["status"].as_str().filter(|_| tool.done());
        let (running, failed) = (tool.running(), tool.failed());
        let label = match (memo, running, failed, status) {
            (true, true, ..) => "Edit memo — review tab opened",
            (false, true, ..) => "Edit summary — review tab opened",
            (true, _, true, _) => "Memo edit failed",
            (false, _, true, _) => "Summary edit failed",
            (true, _, _, Some("applied")) => "Memo updated",
            (false, _, _, Some("applied")) => "Summary updated",
            (true, _, _, Some("declined")) => "Memo edit declined",
            (false, _, _, Some("declined")) => "Summary edit declined",
            (true, ..) => "Edit memo",
            (false, ..) => "Edit summary",
        };
        let red = |value: u32| -> gpui::Hsla { rgb(value).into() };
        let icon_color = if failed {
            red(0xef4444)
        } else if status == Some("applied") {
            rgb(0x10b981).into()
        } else {
            colors.muted_foreground
        };
        let content = tool.input["content"].as_str().unwrap_or_default();
        let pending = cx
            .try_global::<AiViewServices>()
            .is_some_and(|services| services.approvals.is_pending(&tool.id));
        let footer_error = |text: String| {
            div()
                .flex()
                .items_center()
                .gap_2()
                .border_t_1()
                .border_color(rgb(0xfecaca))
                .bg(rgb(0xfef2f2))
                .px(px(14.))
                .py(px(10.))
                .child(icon("chat/CancelCircleIcon.svg", 16., red(0xef4444)))
                .child(
                    div()
                        .text_size(px(13.))
                        .line_height(px(19.5))
                        .text_color(rgb(0xdc2626))
                        .child(text),
                )
        };
        let error = (status == Some("error")).then(|| {
            tool.output["message"]
                .as_str()
                .unwrap_or("Unknown error")
                .to_owned()
        });
        let candidates = tool.output["candidates"]
            .as_array()
            .filter(|candidates| !memo && !candidates.is_empty())
            .cloned();
        let preview_style = markdown::Style {
            colors: Theme {
                foreground: colors.muted_foreground,
                ..*colors
            },
            ..style.clone()
        };
        let id: Arc<str> = tool.id.as_str().into();
        div()
            .my(px(10.))
            .overflow_hidden()
            .border_1()
            .border_color(if failed {
                rgb(0xfecaca).into()
            } else {
                colors.border.opacity(0.8)
            })
            // GPUI paints a shadow under the whole box; the chat's surface keeps it outside.
            .bg(self.surface(window))
            .shadow_sm()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(14.))
                    .py_2()
                    .text_size(px(13.))
                    .line_height(px(19.5))
                    .map(|header| {
                        if failed {
                            header.bg(rgb(0xfef2f2)).text_color(rgb(0xb91c1c))
                        } else {
                            header
                                .bg(colors.muted.opacity(0.8))
                                .text_color(colors.muted_foreground)
                        }
                    })
                    .child(if running {
                        spinner(
                            SharedString::from(format!("chat-edit-spinner-{}", tool.id)),
                            16.,
                            colors.muted_foreground,
                        )
                        .into_any_element()
                    } else {
                        icon("chat/PencilIcon.svg", 16., icon_color).into_any_element()
                    })
                    .child(div().font_weight(MEDIUM).child(label)),
            )
            .when(!content.is_empty(), |card| {
                let blocks = markdown::parse(content);
                card.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .px(px(14.))
                        .py(px(10.))
                        .child(
                            div()
                                .border_1()
                                .border_color(colors.border.opacity(0.8))
                                .bg(colors.card)
                                .child(
                                    div()
                                        .id(SharedString::from(format!(
                                            "chat-edit-preview-{}",
                                            tool.id
                                        )))
                                        .max_h(px(256.))
                                        .overflow_y_scroll()
                                        .px_3()
                                        .py(px(10.))
                                        .text_color(colors.muted_foreground)
                                        .child(markdown::render_preview(
                                            &blocks,
                                            &preview_style,
                                            &format!("chat-edit-md-{}", tool.id),
                                        )),
                                ),
                        ),
                )
            })
            .when(failed, |card| {
                card.child(footer_error(
                    tool.error.clone().unwrap_or_else(|| "Unknown error".into()),
                ))
            })
            .when_some(error, |card, error| {
                card.child(footer_error(error))
                    .when_some(candidates, |card, candidates| {
                        card.child(
                            div().px(px(14.)).pb(px(10.)).child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .border_1()
                                    .border_color(colors.border)
                                    .bg(colors.muted)
                                    .p_2()
                                    .text_size(px(12.))
                                    .line_height(px(16.))
                                    .text_color(colors.muted_foreground)
                                    .children(candidates.iter().map(|candidate| {
                                        format!(
                                            "{} ({})",
                                            candidate["title"].as_str().unwrap_or_default(),
                                            candidate["enhancedNoteId"]
                                                .as_str()
                                                .unwrap_or_default()
                                        )
                                    })),
                            ),
                        )
                    })
            })
            .when(pending, |card| {
                let (decline, apply) = (id.clone(), id.clone());
                card.child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .border_t_1()
                        .border_color(colors.border.opacity(0.8))
                        .px(px(14.))
                        .py(px(10.))
                        .child(
                            edit_review::button(
                                format!("chat-edit-decline-{id}"),
                                "Decline",
                                false,
                                true,
                                colors,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| this.review_edit(decline.clone(), false, cx),
                            )),
                        )
                        .child(
                            edit_review::button(
                                format!("chat-edit-apply-{id}"),
                                if memo {
                                    "Apply to memo"
                                } else {
                                    "Apply to summary"
                                },
                                true,
                                true,
                                colors,
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| this.review_edit(apply.clone(), true, cx),
                            )),
                        ),
                )
            })
            .into_any_element()
    }

    /// The card's Decline / Apply. Like shipping's, a failure leaves the edit pending.
    fn review_edit(&mut self, id: Arc<str>, approved: bool, cx: &mut Context<Self>) {
        edit_review::decide(
            self.runtime.clone(),
            self.workspace.clone(),
            id,
            approved,
            cx,
        )
        .detach();
        cx.notify();
    }
}
