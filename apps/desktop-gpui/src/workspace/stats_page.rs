//! `SettingsStats` (`settings/stats/index.tsx`): the personal activity page
//! with the range group, the metric cards, the year heatmap (tremor
//! `Tracker`), and the milestone panel (tremor `ProgressBar`).

use std::cell::Cell;
use std::rc::Rc;

use chrono::{Datelike, Utc, Weekday};
use gpui::{
    AnyElement, Bounds, ClickEvent, Context, Div, MouseMoveEvent, Pixels, SharedString, Window,
    canvas, div, prelude::*, px,
};

use super::Workspace;
use crate::stats::{ActivityRecord, CONVERSATION_MILESTONES, Range, Summary};
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon};

pub(crate) struct StatsState {
    records: Option<Result<Vec<ActivityRecord>, String>>,
    range: Range,
    /// The heatmap block under the pointer, for its tooltip.
    hovered: Option<usize>,
    /// Bounds of the tracker grid, recorded while painting, so the tooltip
    /// can anchor to the hovered block.
    tracker_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
}

const GAP: f32 = 3.0;
/// The tracker rows stretch to the weekday label column, whose `text-[9px]`
/// spans lay out 13px tall with WebKit's normal line height, so the blocks
/// are `(width / 53) × 13` rather than square.
const ROW_HEIGHT: f32 = 13.0;
const TRACKER_HEIGHT: f32 = ROW_HEIGHT * 7.0 + GAP * 6.0;

impl Workspace {
    /// `useActivity`: (re)load the records for the current user.
    pub(crate) fn ensure_stats(&mut self, cx: &mut Context<Self>) {
        if self.stats.is_none() {
            self.stats = Some(StatsState {
                records: None,
                range: Range::All,
                hovered: None,
                tracker_bounds: Rc::default(),
            });
        }
        self.reload_stats(cx);
    }

    pub(crate) fn reload_stats(&mut self, cx: &mut Context<Self>) {
        if self.stats.is_none() {
            return;
        }
        let task = self.store.load_activity();
        cx.spawn(async move |this, cx| {
            let result = match task.await {
                Ok(Ok(records)) => Ok(records),
                Ok(Err(error)) => Err(error.to_string()),
                Err(error) => Err(error.to_string()),
            };
            this.update(cx, |this, cx| {
                if let Some(stats) = this.stats.as_mut() {
                    stats.records = Some(result);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn stats_week_start(&self) -> Weekday {
        // `useWeekStartsOn`: the setting, else the system locale's start
        // (Sunday for the shell's en-US formatting).
        match self
            .provider_settings
            .string_setting("week_start", &["general", "week_start"])
            .as_deref()
        {
            Some("monday") => Weekday::Mon,
            _ => Weekday::Sun,
        }
    }

    fn summarize_stats(&self, records: &[ActivityRecord], range: Range) -> Summary {
        let now = Utc::now();
        let week_start = self.stats_week_start();
        // `useTimezone`: the `timezone` setting when it names a zone.
        match self
            .provider_settings
            .string_setting("timezone", &["general", "timezone"])
            .and_then(|name| name.parse::<chrono_tz::Tz>().ok())
        {
            Some(tz) => crate::stats::summarize(records, now, &tz, week_start, range),
            None => crate::stats::summarize(records, now, &chrono::Local, week_start, range),
        }
    }

    /// `mx-auto w-full max-w-3xl flex-col gap-8`
    pub(super) fn render_stats_settings(
        &self,
        title: Div,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let page = div()
            .flex()
            .flex_col()
            .w_full()
            .max_w(px(768.0))
            .mx_auto()
            .gap_8();
        let header = div().flex().flex_col().gap_2().child(title).child(
            div()
                .tw_text_sm()
                .text_color(theme.muted_foreground)
                .child("Your conversation history."),
        );
        let Some(stats) = self.stats.as_ref() else {
            return page.child(header);
        };
        let records = match &stats.records {
            None => {
                return page.child(header).child(
                    div()
                        .tw_text_sm()
                        .text_color(theme.muted_foreground)
                        .child("Loading your stats…"),
                );
            }
            Some(Err(_)) => {
                return page.child(header).child(
                    div()
                        .tw_text_sm()
                        .text_color(theme.muted_foreground)
                        .child("Couldn't load your stats. Reopen this page to try again."),
                );
            }
            Some(Ok(records)) => records,
        };
        let summary = self.summarize_stats(records, stats.range);

        page.child(header)
            .child(self.render_stats_overview(stats, &summary, cx))
            .child(self.render_stats_activity(stats, &summary, window, cx))
            .child(self.render_stats_milestones(&summary))
            .child(
                div()
                    .tw_text_xs()
                    .text_color(theme.muted_foreground)
                    .child("Includes imported transcripts. Deleted conversations are excluded."),
            )
    }

    /// Overview: the heading, the `bg-muted p-1 rounded-lg` range group of
    /// ghost `size="sm"` buttons, and the three `StatCard`s.
    fn render_stats_overview(
        &self,
        stats: &StatsState,
        summary: &Summary,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let ranges = [
            (Range::All, "All time"),
            (Range::Days30, "30 days"),
            (Range::Days7, "7 days"),
        ];
        let group = div()
            .relative()
            .flex()
            .gap_1()
            .p_1()
            .child(crate::squircle::squircle(
                crate::squircle::CONTROL_RADIUS,
                Some(theme.muted),
                None,
            ))
            .children(ranges.into_iter().map(|(range, label)| {
                let active = stats.range == range;
                div()
                    .id(SharedString::from(format!("stats-range-{label}")))
                    .relative()
                    .flex()
                    .h(px(28.0))
                    .items_center()
                    .justify_center()
                    .px_3()
                    .tw_text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .cursor_pointer()
                    .when(active, |button| {
                        button
                            .text_color(theme.foreground)
                            .child(crate::squircle::squircle(
                                crate::squircle::CONTROL_RADIUS,
                                Some(theme.background),
                                None,
                            ))
                            .shadow(vec![gpui::BoxShadow {
                                color: gpui::hsla(0.0, 0.0, 0.0, 0.05),
                                offset: gpui::point(px(0.0), px(1.0)),
                                blur_radius: px(2.0),
                                spread_radius: px(0.0),
                            }])
                    })
                    .when(!active, |button| {
                        button
                            .text_color(theme.muted_foreground)
                            .hover(move |style| style.text_color(theme.foreground))
                    })
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(stats) = this.stats.as_mut() {
                            stats.range = range;
                            cx.notify();
                        }
                    }))
                    .child(div().relative().child(label))
            }));

        let metrics = [
            (
                "Conversations",
                format_number(summary.conversations as f64, 0),
            ),
            (
                "Hours transcribed",
                format_number((summary.hours * 10.0).round() / 10.0, 1),
            ),
            ("Active days", format_number(summary.active_days as f64, 0)),
        ];
        div()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .tw_text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child("Overview"),
                    )
                    .child(group),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .children(metrics.into_iter().map(|(label, value)| {
                        // `StatCard`: `rounded-[20px] border p-4`.
                        div()
                            .relative()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .p_4()
                            .child(crate::squircle::squircle(
                                crate::squircle::PANEL_RADIUS,
                                None,
                                Some((1.0, theme.border)),
                            ))
                            .child(
                                div()
                                    .relative()
                                    .tw_text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(label),
                            )
                            .child(
                                // `mt-2 text-3xl font-medium tracking-tight tabular-nums`
                                div()
                                    .relative()
                                    .mt_2()
                                    .text_size(px(30.0))
                                    .line_height(px(36.0))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child(SharedString::from(value)),
                            )
                    })),
            )
    }

    /// The past-year heatmap: month labels, weekday labels, the tracker grid
    /// with tooltips, and the legend.
    fn render_stats_activity(
        &self,
        stats: &StatsState,
        summary: &Summary,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let colors = [
            theme.muted,
            alpha(theme.foreground, 0.2),
            alpha(theme.foreground, 0.4),
            alpha(theme.foreground, 0.6),
            alpha(theme.foreground, 0.8),
        ];
        let days = &summary.days;
        let columns = days.len().div_ceil(7);
        let tracker_bounds = stats.tracker_bounds.clone();

        // Month labels: one `1fr` column per week, labelled where the month
        // changes (`text-[10px]`, overflowing its column).
        let month_labels = div()
            .relative()
            .h(px(15.0))
            .mb_2()
            .ml(px(40.0))
            .text_size(px(10.0))
            .line_height(px(15.0))
            .text_color(theme.muted_foreground)
            .child(
                canvas(|_, _, _| (), {
                    let labels: Vec<(usize, String)> = (0..columns)
                        .filter_map(|column| {
                            let day = days.get(column * 7)?;
                            let previous = column
                                .checked_sub(1)
                                .and_then(|c| days.get(c * 7))
                                .map(|d| d.date.month());
                            (previous != Some(day.date.month()))
                                .then(|| (column, month_short(day.date.month())))
                        })
                        .collect();
                    let font = self.font_family.clone();
                    move |bounds, _, window, cx| {
                        let pitch = (f32::from(bounds.size.width) - GAP * (columns as f32 - 1.0))
                            / columns as f32
                            + GAP;
                        for (column, label) in &labels {
                            let mut style = window.text_style();
                            style.font_size = px(10.0).into();
                            style.color = theme.muted_foreground.into();
                            if let Some(font) = &font {
                                style.font_family = font.clone();
                            }
                            let run = style.to_run(label.len());
                            let line = window.text_system().shape_line(
                                SharedString::from(label.clone()),
                                px(10.0),
                                &[run],
                                None,
                            );
                            let origin = gpui::point(
                                bounds.left() + px(*column as f32 * pitch),
                                bounds.top(),
                            );
                            line.paint(origin, px(15.0), window, cx).ok();
                        }
                    }
                })
                .size_full(),
            );

        let weekday_labels = div()
            .flex()
            .flex_col()
            .w(px(32.0))
            .flex_shrink_0()
            .gap(px(GAP))
            .text_size(px(9.0))
            .line_height(px(ROW_HEIGHT))
            .text_color(theme.muted_foreground)
            .children((0..7).map(|row| {
                let label = days
                    .get(row)
                    .filter(|_| row % 2 == 1)
                    .map(|day| weekday_short(day.date.weekday()))
                    .unwrap_or("");
                div().h(px(ROW_HEIGHT)).flex().items_center().child(label)
            }));

        let hovered = stats.hovered;
        let tracker = div()
            .id("stats-tracker")
            .relative()
            .flex_1()
            .min_w_0()
            .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                let Some(stats) = this.stats.as_mut() else {
                    return;
                };
                let Some(bounds) = stats.tracker_bounds.get() else {
                    return;
                };
                let pitch_x = (f32::from(bounds.size.width) - GAP * (columns as f32 - 1.0))
                    / columns as f32
                    + GAP;
                let pitch_y = ROW_HEIGHT + GAP;
                let x = f32::from(event.position.x - bounds.left());
                let y = f32::from(event.position.y - bounds.top());
                let next = if bounds.contains(&event.position) {
                    let column = (x / pitch_x).floor() as usize;
                    let row = (y / pitch_y).floor() as usize;
                    let index = column * 7 + row;
                    (row < 7 && x % pitch_x < pitch_x - GAP && y % pitch_y < pitch_y - GAP)
                        .then_some(index)
                } else {
                    None
                };
                if stats.hovered != next {
                    stats.hovered = next;
                    cx.notify();
                }
            }))
            .child(
                canvas(move |bounds, _, _| tracker_bounds.set(Some(bounds)), {
                    let counts: Vec<usize> = days.iter().map(|day| day.count).collect();
                    move |bounds, _, window, _| {
                        let pitch = (f32::from(bounds.size.width) - GAP * (columns as f32 - 1.0))
                            / columns as f32
                            + GAP;
                        let cell = pitch - GAP;
                        for (index, count) in counts.iter().enumerate() {
                            let column = index / 7;
                            let row = index % 7;
                            let mut color = colors[(*count).min(4)];
                            if hovered == Some(index) {
                                color.a *= 0.6;
                            }
                            window.paint_quad(
                                gpui::fill(
                                    Bounds::new(
                                        gpui::point(
                                            bounds.left() + px(column as f32 * pitch),
                                            bounds.top() + px(row as f32 * (ROW_HEIGHT + GAP)),
                                        ),
                                        gpui::size(px(cell), px(ROW_HEIGHT)),
                                    ),
                                    color,
                                )
                                .corner_radii(px(3.0)),
                            );
                        }
                    }
                })
                .w_full()
                .h(px(TRACKER_HEIGHT)),
            )
            .children(hovered.and_then(|index| {
                let day = days.get(index)?;
                Some(self.render_stats_tooltip(stats, index, columns, day, window))
            }));

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .tw_text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child("Activity over the past year"),
                    )
                    .child(div().tw_text_xs().text_color(theme.muted_foreground).child(
                        SharedString::from(format!("Weekly streak: {}", summary.streak)),
                    )),
            )
            .child(
                div()
                    .pb_1()
                    .flex()
                    .flex_col()
                    .min_w(px(620.0))
                    .child(month_labels)
                    .child(div().flex().gap_2().child(weekday_labels).child(tracker)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .tw_text_xs()
                    .text_color(theme.muted_foreground)
                    .child("Capture a conversation each week to keep your streak going.")
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child("Less")
                            .children(
                                colors
                                    .into_iter()
                                    .map(|color| div().size(px(10.0)).rounded(px(2.0)).bg(color)),
                            )
                            .child("More"),
                    ),
            )
    }

    /// The HoverCard tooltip (`side="top" sideOffset={10}`): `bg-foreground
    /// text-background rounded-md px-2 py-1 text-xs shadow-md`.
    fn render_stats_tooltip(
        &self,
        stats: &StatsState,
        index: usize,
        columns: usize,
        day: &crate::stats::Day,
        window: &Window,
    ) -> AnyElement {
        let theme = self.theme;
        let Some(bounds) = stats.tracker_bounds.get() else {
            return div().into_any_element();
        };
        let pitch =
            (f32::from(bounds.size.width) - GAP * (columns as f32 - 1.0)) / columns as f32 + GAP;
        let cell = pitch - GAP;
        let column = index / 7;
        let row = index % 7;
        let center_x = column as f32 * pitch + cell / 2.0;
        let top = row as f32 * (ROW_HEIGHT + GAP);
        let text = format!("{}. Conversations: {}", long_date(day.date), day.count);
        // `align="center"`: measure the label to centre the card on the block.
        let mut style = window.text_style();
        style.font_size = px(12.0).into();
        if let Some(font) = &self.font_family {
            style.font_family = font.clone();
        }
        let width = window
            .text_system()
            .shape_line(
                SharedString::from(text.clone()),
                px(12.0),
                &[style.to_run(text.len())],
                None,
            )
            .width
            + px(16.0);
        div()
            .absolute()
            .left(px(center_x) - width / 2.0)
            .top(px(top - 10.0))
            .child(
                gpui::deferred(
                    gpui::anchored()
                        .anchor(gpui::Corner::BottomLeft)
                        .snap_to_window_with_margin(px(8.0))
                        .child(
                            div()
                                .relative()
                                .px_2()
                                .py_1()
                                .tw_text_xs()
                                .text_color(theme.background)
                                .whitespace_nowrap()
                                .child(crate::squircle::squircle(6.0, Some(theme.foreground), None))
                                .shadow_md()
                                .child(div().relative().child(SharedString::from(text))),
                        ),
                )
                .with_priority(2),
            )
            .into_any_element()
    }

    /// `MilestonePanel`: `rounded-[20px] border p-5 gap-5` with the trophy
    /// header, the `ProgressBar`, and the milestone badges.
    fn render_stats_milestones(&self, summary: &Summary) -> Div {
        let theme = self.theme;
        let total = summary.total_conversations as u64;
        let next = summary.next_milestone.max(1);
        let ratio = (total as f32 / next as f32).clamp(0.0, 1.0);
        div()
            .relative()
            .flex()
            .flex_col()
            .gap_5()
            .p_5()
            .child(crate::squircle::squircle(
                crate::squircle::PANEL_RADIUS,
                None,
                Some((1.0, theme.border)),
            ))
            .child(
                div()
                    .relative()
                    .flex()
                    .items_start()
                    .gap_3()
                    .child(div().mt(px(2.0)).flex_shrink_0().child(icon(
                        "trophy",
                        px(20.0),
                        theme.muted_foreground,
                    )))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .tw_text_sm()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child(if total == 0 {
                                        "Capture your first conversation"
                                    } else {
                                        "Next milestone"
                                    }),
                            )
                            .child(div().tw_text_xs().text_color(theme.muted_foreground).child(
                                SharedString::from(format!(
                                    "{} of {} conversations captured",
                                    total,
                                    format_number(next as f64, 0)
                                )),
                            )),
                    ),
            )
            .child(
                // `ProgressBar`: `h-2 bg-muted rounded-full` track, `bg-foreground/80` fill.
                div()
                    .relative()
                    .flex()
                    .w_full()
                    .h(px(8.0))
                    .rounded(px(4.0))
                    .overflow_hidden()
                    .bg(theme.muted)
                    .child(
                        div()
                            .h_full()
                            .w(gpui::relative(ratio))
                            .rounded(px(4.0))
                            .bg(alpha(theme.foreground, 0.8)),
                    ),
            )
            .child(div().relative().flex().flex_wrap().gap_2().children(
                CONVERSATION_MILESTONES.into_iter().map(|target| {
                    let reached = total >= target;
                    // `MilestoneBadge`: `rounded-lg border px-2.5 py-1 text-xs`
                    // on the chip squircle.
                    div()
                        .relative()
                        .flex()
                        .items_center()
                        .gap_1()
                        .px(px(10.0))
                        .py_1()
                        .tw_text_xs()
                        .text_color(if reached {
                            theme.foreground
                        } else {
                            theme.muted_foreground
                        })
                        .child(crate::squircle::squircle(
                            6.0,
                            reached.then_some(theme.muted),
                            Some((1.0, theme.border)),
                        ))
                        .when(reached, |badge| {
                            badge.child(div().relative().child(icon(
                                "check",
                                px(12.0),
                                theme.foreground,
                            )))
                        })
                        .child(
                            div()
                                .relative()
                                .child(SharedString::from(format_number(target as f64, 0))),
                        )
                }),
            ))
    }
}

/// `Intl.NumberFormat("en-US")` with grouping separators.
fn format_number(value: f64, decimals: usize) -> String {
    let negative = value < 0.0;
    let value = value.abs();
    let whole = value.trunc() as u64;
    let mut digits = whole.to_string();
    let mut grouped = String::new();
    while digits.len() > 3 {
        let tail = digits.split_off(digits.len() - 3);
        grouped = format!(",{tail}{grouped}");
    }
    grouped = format!("{digits}{grouped}");
    if decimals > 0 {
        let fraction = value.fract();
        if fraction > 0.0 {
            let scaled = (fraction * 10f64.powi(decimals as i32)).round() as u64;
            if scaled > 0 {
                grouped = format!("{grouped}.{scaled}");
            }
        }
    }
    if negative {
        format!("-{grouped}")
    } else {
        grouped
    }
}

fn month_short(month: u32) -> String {
    [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][(month as usize).saturating_sub(1).min(11)]
    .to_string()
}

fn weekday_short(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Mon => "Mon",
        Weekday::Tue => "Tue",
        Weekday::Wed => "Wed",
        Weekday::Thu => "Thu",
        Weekday::Fri => "Fri",
        Weekday::Sat => "Sat",
        Weekday::Sun => "Sun",
    }
}

/// `Intl.DateTimeFormat(locale, { dateStyle: "long" })` in en-US.
fn long_date(date: chrono::NaiveDate) -> String {
    let month = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ][(date.month() as usize).saturating_sub(1).min(11)];
    format!("{month} {}, {}", date.day(), date.year())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_group_and_keep_one_decimal_like_intl() {
        assert_eq!(format_number(0.0, 0), "0");
        assert_eq!(format_number(1234.0, 0), "1,234");
        assert_eq!(format_number(3.5, 1), "3.5");
        assert_eq!(format_number(2.0, 1), "2");
        assert_eq!(format_number(1000000.0, 0), "1,000,000");
        assert_eq!(
            long_date(chrono::NaiveDate::from_ymd_opt(2026, 9, 4).unwrap()),
            "September 4, 2026"
        );
    }
}
