//! Left sidebar: `SidebarTimelineChrome` header row plus the `TimelineView`
//! buckets (`apps/desktop/src/sidebar/timeline/{buckets,item,realtime}.tsx`).

use chrono::{Local, Utc};
use gpui::{
    AnyElement, ClickEvent, Context, Div, MouseButton, Pixels, SharedString, Stateful, Window, div,
    list, prelude::*, px,
};

use super::{Sessions, SidebarRow, Workspace};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnchorVisibility {
    Visible,
    Above,
    Below,
}
use crate::theme::alpha;
use crate::timeline::{self, Bucket, IndicatorPlacement, ItemKind, Precision};
use crate::ui::{TailwindText as _, icon};

impl Workspace {
    pub(super) fn render_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;

        // `flex h-9 shrink-0 items-start pt-[9px] pr-1 pl-2`; on Windows/Linux the
        // sidebar toggle lives in the title bar and a `size-7` spacer keeps the
        // row aligned.
        let header = div()
            .flex()
            .h(px(36.0))
            .flex_shrink_0()
            .items_start()
            .pt(px(9.0))
            .pr_1()
            .pl_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(div().size(px(28.0)).flex_shrink_0())
                    .child(
                        self.tracked_chrome_button("search", cx)
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.open_note_dialog(window, cx)
                            }))
                            .child(icon("search", px(15.0), self.chrome_icon_color("search"))),
                    )
                    .child(
                        self.tracked_chrome_button("new-note", cx)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.new_note(cx)))
                            .child(icon(
                                "note-edit",
                                px(15.0),
                                self.chrome_icon_color("new-note"),
                            )),
                    )
                    .child(
                        // `!isDefaultView && "bg-accent text-foreground"`
                        self.tracked_chrome_button("sort-notes", cx)
                            .when(
                                !self.is_default_notes_view() && self.hovered != Some("sort-notes"),
                                |button| {
                                    button.child(crate::squircle::squircle(
                                        crate::squircle::CONTROL_RADIUS,
                                        Some(theme.accent),
                                        None,
                                    ))
                                },
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.toggle_filter_menu(cx)
                            }))
                            .child(icon(
                                "filter",
                                px(15.0),
                                if self.is_default_notes_view() {
                                    self.chrome_icon_color("sort-notes")
                                } else {
                                    theme.foreground
                                },
                            )),
                    ),
            );

        if matches!(self.sessions, Sessions::Ready(_)) {
            self.apply_anchor_scroll(window, cx);
        }
        let body = match &self.sessions {
            Sessions::Loading => div().flex_1(),
            Sessions::Failed(error) => div()
                .flex_1()
                .px_3()
                .py_4()
                .tw_text_sm()
                .text_color(theme.destructive)
                .child(SharedString::from(error.clone())),
            Sessions::Ready(timeline) => {
                let anchor = self.anchor_visibility();
                // `showOpenCalendarChip`: only while scrolled to the top.
                let scroll_top = self.list_state.logical_scroll_top();
                let at_top = scroll_top.item_ix == 0 && scroll_top.offset_in_item <= px(0.0);
                let show_chip = timeline.has_more_future_items && at_top;
                // `showTopNowChip` / the bottom `TimelineNowChip`.
                let show_top_now = anchor == Some(AnchorVisibility::Above);
                let show_bottom_now = anchor == Some(AnchorVisibility::Below);
                // `data-sidebar-timeline-bottom-fade` while `!isScrolledToBottom`.
                let max_offset = self.list_state.max_offset_for_scrollbar().height;
                let scrolled = -self.list_state.scroll_px_offset_for_scrollbar().y;
                let show_bottom_fade = max_offset > px(0.0) && scrolled < max_offset - px(1.0);
                let sticky_header = self.sticky_header(timeline);
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        list(
                            self.list_state.clone(),
                            cx.processor(|this, index: usize, _window, cx| {
                                this.render_sidebar_row(index, cx)
                            }),
                        )
                        .size_full(),
                    )
                    .when_some(sticky_header, |container, (bucket, top)| {
                        container.child(
                            div()
                                .absolute()
                                .top(top)
                                .left_0()
                                .right_0()
                                .child(self.render_bucket_header(&timeline.buckets[bucket])),
                        )
                    })
                    .when(show_bottom_fade, |container| {
                        container.child(
                            div()
                                .absolute()
                                .bottom_0()
                                .left_0()
                                .right_0()
                                .h(px(28.0))
                                .bg(gpui::linear_gradient(
                                    180.0,
                                    gpui::linear_color_stop(alpha(theme.background, 0.0), 0.0),
                                    gpui::linear_color_stop(theme.background, 1.0),
                                )),
                        )
                    })
                    .when(show_top_now, |container| {
                        container.child(
                            div()
                                .absolute()
                                .top_1()
                                .left_0()
                                .right_0()
                                .flex()
                                .justify_center()
                                .child(self.render_now_chip(true, cx)),
                        )
                    })
                    .when(show_bottom_now, |container| {
                        container.child(
                            div()
                                .absolute()
                                .bottom_2()
                                .left_0()
                                .right_0()
                                .flex()
                                .justify_center()
                                .child(self.render_now_chip(false, cx)),
                        )
                    })
                    .when(show_chip, |container| {
                        container.child(
                            // Chip stack at `top-1` when chips overlap the header row.
                            div()
                                .absolute()
                                .top_1()
                                .left_0()
                                .right_0()
                                .flex()
                                .justify_center()
                                .child(self.render_open_calendar_chip(cx)),
                        )
                    })
            }
        };

        div()
            .flex()
            .flex_col()
            .h_full()
            .w(px(self.sidebar_width))
            .flex_shrink_0()
            .gap_1()
            .overflow_hidden()
            .child(header)
            .child(body)
    }

    /// Where the current-time line is relative to the list viewport
    /// (`isAnchorVisible` / `isScrolledPastAnchor`).
    fn anchor_visibility(&self) -> Option<AnchorVisibility> {
        let (row, at_bottom) = self.anchor_row()?;
        let viewport = self.list_state.viewport_bounds();
        if viewport.size.height <= px(0.0) {
            return None;
        }
        match self.list_state.bounds_for_item(row) {
            Some(bounds) => {
                let y = if at_bottom {
                    bounds.bottom()
                } else {
                    bounds.top()
                };
                if y < viewport.top() {
                    Some(AnchorVisibility::Above)
                } else if y > viewport.bottom() {
                    Some(AnchorVisibility::Below)
                } else {
                    Some(AnchorVisibility::Visible)
                }
            }
            None => {
                if row < self.list_state.logical_scroll_top().item_ix {
                    Some(AnchorVisibility::Above)
                } else {
                    Some(AnchorVisibility::Below)
                }
            }
        }
    }

    /// `scrollToAnchor({ viewportRatio })` over two frames: bring the row into
    /// the viewport first so it gets measured, then place the line at the
    /// requested ratio. The launch scroll uses 0.15, the chips 0.5.
    fn apply_anchor_scroll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((row, at_bottom)) = self.anchor_row() else {
            return;
        };
        let viewport = self.list_state.viewport_bounds();
        if viewport.size.height <= px(0.0) {
            return;
        }
        if !self.anchor_scrolled_once {
            self.anchor_scrolled_once = true;
            self.anchor_scroll = Some(0.15);
        }
        let Some(ratio) = self.anchor_scroll else {
            return;
        };
        match self.list_state.bounds_for_item(row) {
            Some(bounds) => {
                let y = if at_bottom {
                    bounds.bottom()
                } else {
                    bounds.top()
                };
                let target = viewport.top() + viewport.size.height * ratio;
                self.list_state.scroll_by(y - target);
                self.anchor_scroll = None;
                // The chips read the clamped scroll offset, which the list
                // settles while painting; re-evaluate them on the next frame.
                let this = cx.entity();
                window.on_next_frame(move |_, cx| this.update(cx, |_, cx| cx.notify()));
            }
            None => {
                self.list_state.scroll_to(gpui::ListOffset {
                    item_ix: row,
                    offset_in_item: px(0.0),
                });
                // The row is measured by this frame; finish on the next one.
                let this = cx.entity();
                window.on_next_frame(move |_, cx| this.update(cx, |_, cx| cx.notify()));
            }
        }
    }

    pub(crate) fn scroll_to_now(&mut self, cx: &mut Context<Self>) {
        self.anchor_scroll = Some(0.5);
        cx.notify();
    }

    /// `TimelineNowChip`: `h-6 rounded-full border border-border bg-card
    /// text-xs font-semibold shadow-md px-2.5 gap-1`, an arrow on the side
    /// the line is, and the yellow sun.
    fn render_now_chip(&self, up: bool, cx: &Context<Self>) -> Stateful<Div> {
        let theme = self.theme;
        let arrow = icon(
            if up { "arrow-up" } else { "arrow-down" },
            px(12.0),
            theme.foreground,
        );
        let hovered = self.hovered == Some("now-chip");
        div()
            .id("now-chip")
            .relative()
            .h(px(24.0))
            .flex()
            .items_center()
            .gap_1()
            .px(px(10.0))
            // `Button variant="outline" size="sm"`: the control squircle.
            .child(crate::squircle::squircle(
                crate::squircle::CONTROL_RADIUS,
                Some(if hovered { theme.accent } else { theme.card }),
                Some((1.0, theme.border)),
            ))
            // The shadow keeps a plain radius; only the fill is a squircle.
            .rounded(px(8.0))
            .shadow_md()
            .tw_text_xs()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(theme.foreground)
            .cursor_pointer()
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.set_hovered("now-chip", *hovered, cx);
            }))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.scroll_to_now(cx)))
            .when(up, |chip| chip.child(arrow))
            .child(icon("sun", px(13.0), gpui::rgb(0xfacc15)))
            .child("Now")
            .when(!up, |chip| {
                chip.child(icon("arrow-down", px(12.0), theme.foreground))
            })
    }

    /// CSS `sticky` bucket headers: while a bucket's own header has scrolled
    /// off, its copy pins to the top of the list and is pushed away by the next
    /// bucket's header. Returns the bucket and the overlay's top offset.
    fn sticky_header(&self, timeline: &crate::timeline::Timeline) -> Option<(usize, Pixels)> {
        const HEADER_HEIGHT: f32 = 28.0;
        let scroll_top = self.list_state.logical_scroll_top();
        let bucket = match self.rows.get(scroll_top.item_ix)? {
            SidebarRow::Spacer => return None,
            SidebarRow::Header { bucket } => {
                if scroll_top.offset_in_item <= px(0.0) {
                    return None;
                }
                *bucket
            }
            SidebarRow::Session { bucket, .. } => *bucket,
        };
        timeline.buckets.get(bucket)?;
        let next_header = self
            .rows
            .iter()
            .position(|row| matches!(row, SidebarRow::Header { bucket: b } if *b == bucket + 1));
        let viewport_top = self.list_state.viewport_bounds().origin.y;
        let top = next_header
            .and_then(|ix| self.list_state.bounds_for_item(ix))
            .map(|bounds| f32::from(bounds.origin.y - viewport_top) - HEADER_HEIGHT)
            .unwrap_or(0.0)
            .min(0.0);
        Some((bucket, px(top)))
    }

    /// `ResizableHandle`: the 4px gap between the panels doubles as a
    /// `cursor-ew-resize` drag handle with an 8px hit area.
    pub(super) fn render_sidebar_handle(&self, cx: &Context<Self>) -> Stateful<Div> {
        div()
            .id("sidebar-resize-handle")
            .relative()
            .w(px(4.0))
            .h_full()
            .flex_shrink_0()
            .cursor_col_resize()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(-2.0))
                    .w(px(8.0)),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.begin_sidebar_drag(event.position.x, cx);
                }),
            )
    }

    /// `TimelineTopChip`: `h-6 rounded-full border border-border bg-card
    /// text-muted-foreground text-xs font-medium px-2.5 gap-1 shadow-xs`.
    fn render_open_calendar_chip(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let color = self.chrome_icon_color("open-calendar");
        div().child(
            self.tracked_chrome_button("open-calendar", cx)
                .h(px(24.0))
                .w_auto()
                .px(px(10.0))
                .gap_1()
                .child(crate::squircle::squircle(
                    crate::squircle::CONTROL_RADIUS,
                    Some(if self.hovered == Some("open-calendar") {
                        theme.accent
                    } else {
                        theme.card
                    }),
                    Some((1.0, theme.border)),
                ))
                .rounded(px(8.0))
                .shadow_xs()
                .tw_text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(color)
                .child(icon("calendar-dots", px(12.0), color))
                // `openNew({ type: "calendar" })`
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_calendar(cx)))
                .child("Open calendar"),
        )
    }

    fn render_sidebar_row(&self, index: usize, cx: &Context<Self>) -> AnyElement {
        let Sessions::Ready(timeline) = &self.sessions else {
            return div().into_any_element();
        };
        match self.rows.get(index) {
            // `topChipsOverlapHeader` is set in this layout, so the spacer is `h-9`.
            Some(SidebarRow::Spacer) => div().h(px(36.0)).flex_shrink_0().into_any_element(),
            Some(SidebarRow::Header { bucket }) => {
                let bucket = &timeline.buckets[*bucket];
                let items_len = bucket.items.len();
                // Every item of a Today bucket that has no future items is in
                // the past, so the line sits directly under the header.
                let line_here = bucket.label == "Today"
                    && matches!(
                        timeline::indicator_placement(&bucket.items, Utc::now(), self.sort_order),
                        IndicatorPlacement::Before { index: 0 }
                    )
                    && items_len > 0;
                div()
                    .child(self.render_bucket_header(bucket))
                    .when(line_here, |wrap| {
                        wrap.child(self.render_current_time_line())
                    })
                    .into_any_element()
            }
            Some(SidebarRow::Session { bucket, item }) => {
                let bucket = &timeline.buckets[*bucket];
                let placement = if bucket.label == "Today" {
                    Some(timeline::indicator_placement(
                        &bucket.items,
                        Utc::now(),
                        self.sort_order,
                    ))
                } else {
                    None
                };
                let row =
                    self.render_session_row(index, &bucket.items[*item], bucket.precision, cx);
                let line_before = matches!(placement, Some(IndicatorPlacement::Before { index: at }) if at == *item && at > 0);
                let line_after = *item + 1 == bucket.items.len()
                    && matches!(placement, Some(IndicatorPlacement::After));
                div()
                    .when(line_before, |wrap| {
                        wrap.child(self.render_current_time_line())
                    })
                    .child(row)
                    .when(line_after, |wrap| {
                        wrap.child(self.render_current_time_line())
                    })
                    .into_any_element()
            }
            None => div().into_any_element(),
        }
    }

    /// `bg-background pt-0 pr-1 pb-1 pl-3` with a `text-base font-bold` label.
    fn render_bucket_header(&self, bucket: &Bucket) -> Div {
        div().pr_1().pb_1().pl_3().bg(self.theme.background).child(
            div()
                .tw_text_base()
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(self.theme.foreground)
                .child(SharedString::from(bucket.label.clone())),
        )
    }

    /// `CurrentTimeIndicator` (seam variant): a zero-height anchor whose 1px
    /// `bg-red-500/85` rule is shifted up half a pixel (`-translate-y-1/2`), so
    /// it straddles the seam between two rows like the web app's does.
    fn render_current_time_line(&self) -> Div {
        div().relative().h(px(0.0)).w_full().child(
            div()
                .absolute()
                .top(px(-0.5))
                .left_0()
                .right_0()
                .h(px(1.0))
                .bg(alpha(self.theme.red, 0.85)),
        )
    }

    /// `ItemBase`: `w-full rounded-lg px-3 py-2`, `bg-accent` when selected,
    /// `hover:bg-accent/50` otherwise; title `text-sm`, time `font-mono text-xs`.
    fn render_session_row(
        &self,
        index: usize,
        item: &timeline::Item,
        precision: Precision,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let session_id = item.id.clone();
        let is_event = item.kind == ItemKind::Event;
        let selected = !is_event && self.selected.as_deref() == Some(item.id.as_str());
        let title: SharedString = if item.title.is_empty() {
            "Untitled".into()
        } else {
            item.title.clone().into()
        };
        let time = SharedString::from(timeline::format_display_time(
            item.timestamp,
            precision,
            Utc::now(),
            &Local,
        ));
        // `sidebar_show_folder` (Appearance › Notes list) hides the folder line.
        let show_folder = self.provider_settings.bool_setting(
            "sidebar_show_folder",
            &["general", "sidebar_show_folder"],
            true,
        );
        let folder = timeline::folder_label(&item.folder_id).filter(|_| show_folder);

        div().child(
            div()
                .id(index)
                .w_full()
                .rounded_lg()
                .px_3()
                .py_2()
                .cursor_pointer()
                .when(selected, |row| row.bg(theme.accent))
                .when(!selected, |row| {
                    row.hover(move |style| style.bg(alpha(theme.accent, 0.5)))
                })
                // `muted = isTimelineItemInFuture(item)` -> `opacity-65`.
                .when(item.timestamp > Utc::now(), |row| row.opacity(0.65))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                    if is_event {
                        this.open_event(session_id.clone(), cx);
                    } else {
                        this.select(session_id.clone(), cx);
                    }
                }))
                .child(
                    div()
                        .flex()
                        .min_w_0()
                        .items_center()
                        .gap_2()
                        .child(
                            // `flex-grow` with an auto basis: a 0% basis would make
                            // GPUI cache the nowrap title at zero width.
                            div()
                                .flex()
                                .flex_grow()
                                .min_w_0()
                                .flex_col()
                                .gap(px(2.0))
                                .when_some(folder, |column, folder| {
                                    column.child(
                                        div()
                                            .flex()
                                            .min_w_0()
                                            .items_center()
                                            .gap_1()
                                            .tw_text_11()
                                            .text_color(theme.muted_foreground)
                                            .child(icon("folder", px(12.0), theme.muted_foreground))
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .truncate()
                                                    .child(SharedString::from(folder)),
                                            ),
                                    )
                                })
                                .child(div().min_w_0().truncate().tw_text_sm().child(title))
                                .child(
                                    div()
                                        .tw_text_xs()
                                        .text_color(theme.muted_foreground)
                                        .when_some(self.mono_font_family.clone(), |time, family| {
                                            time.font_family(family)
                                        })
                                        .child(time),
                                ),
                        )
                        .when(item.locked, |row| {
                            row.child(
                                div()
                                    .flex_shrink_0()
                                    .text_color(theme.muted_foreground)
                                    .child(icon("lock", px(14.0), theme.muted_foreground)),
                            )
                        }),
                ),
        )
    }
}
