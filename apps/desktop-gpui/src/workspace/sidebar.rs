//! Left sidebar: `SidebarTimelineChrome` header row plus the `TimelineView`
//! buckets (`apps/desktop/src/sidebar/timeline/{buckets,item,realtime}.tsx`).

use chrono::{Local, Utc};
use gpui::{
    AnyElement, ClickEvent, Context, Div, MouseButton, SharedString, div, list, prelude::*, px,
};

use super::{SIDEBAR_WIDTH, Sessions, SidebarRow, Workspace};
use crate::theme::alpha;
use crate::timeline::{self, Bucket, IndicatorPlacement, Precision};
use crate::ui::{TailwindText as _, icon};

impl Workspace {
    pub(super) fn render_sidebar(&self, cx: &Context<Self>) -> Div {
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
                    .child(self.tracked_chrome_button("search", cx).child(icon(
                        "search",
                        px(15.0),
                        self.chrome_icon_color("search"),
                    )))
                    .child(self.tracked_chrome_button("new-note", cx).child(icon(
                        "note-edit",
                        px(15.0),
                        self.chrome_icon_color("new-note"),
                    )))
                    .child(self.tracked_chrome_button("sort-notes", cx).child(icon(
                        "filter",
                        px(15.0),
                        self.chrome_icon_color("sort-notes"),
                    ))),
            );

        let body = match &self.sessions {
            Sessions::Loading => div().flex_1(),
            Sessions::Failed(error) => div()
                .flex_1()
                .px_3()
                .py_4()
                .tw_text_sm()
                .text_color(theme.destructive)
                .child(SharedString::from(error.clone())),
            Sessions::Ready(_) => div().flex_1().min_h_0().child(
                list(
                    self.list_state.clone(),
                    cx.processor(|this, index: usize, _window, cx| {
                        this.render_sidebar_row(index, cx)
                    }),
                )
                .size_full(),
            ),
        };

        div()
            .flex()
            .flex_col()
            .h_full()
            .w(px(SIDEBAR_WIDTH))
            .flex_shrink_0()
            .gap_1()
            .overflow_hidden()
            .child(header)
            .child(body)
    }

    fn render_sidebar_row(&self, index: usize, cx: &Context<Self>) -> AnyElement {
        let Sessions::Ready(timeline) = &self.sessions else {
            return div().into_any_element();
        };
        match self.rows.get(index) {
            Some(SidebarRow::Header { bucket }) => {
                let bucket = &timeline.buckets[*bucket];
                let items_len = bucket.items.len();
                // Every item of a Today bucket that has no future items is in
                // the past, so the line sits directly under the header.
                let line_here = bucket.label == "Today"
                    && matches!(
                        timeline::indicator_placement(&bucket.items, Utc::now()),
                        IndicatorPlacement::Before { index: 0 }
                    )
                    && items_len > 0;
                self.render_bucket_header(bucket)
                    .when(line_here, |header| {
                        header.child(self.render_current_time_line())
                    })
                    .into_any_element()
            }
            Some(SidebarRow::Session { bucket, item }) => {
                let bucket = &timeline.buckets[*bucket];
                let placement = if bucket.label == "Today" {
                    Some(timeline::indicator_placement(&bucket.items, Utc::now()))
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

    /// `CurrentTimeIndicator`: a 1px `bg-red-500/85` rule.
    fn render_current_time_line(&self) -> Div {
        div()
            .relative()
            .h(px(1.0))
            .w_full()
            .bg(alpha(self.theme.red, 0.85))
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
        let selected = self.selected.as_deref() == Some(item.id.as_str());
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
        let folder = timeline::folder_label(&item.folder_id);

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
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                    this.select(session_id.clone(), cx);
                }))
                .child(
                    div()
                        .flex()
                        .min_w_0()
                        .items_start()
                        .gap_2()
                        .child(
                            // `flex-grow` with an auto basis: a 0% basis would make
                            // GPUI cache the nowrap title at zero width.
                            div()
                                .flex()
                                .flex_grow()
                                .min_w_0()
                                .flex_col()
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
                                    .pt(px(2.0))
                                    .text_color(theme.muted_foreground)
                                    .child(icon("lock", px(14.0), theme.muted_foreground)),
                            )
                        }),
                ),
        )
    }
}
