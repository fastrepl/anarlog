//! `SettingsStats` (`settings/stats/index.tsx`): the personal activity page
//! with the range group, the metric cards, the year heatmap (tremor
//! `Tracker`) and the badge collection (`badge-collection.tsx`), plus
//! `SettingsInsights` (`insights.tsx`) over the same records.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;

use chrono::{Datelike, Utc, Weekday};
use gpui::{
    AnyElement, Bounds, ClickEvent, Context, Div, MouseButton, MouseMoveEvent, Pixels,
    SharedString, Window, canvas, div, img, prelude::*, px,
};

use super::Workspace;
use crate::badges::{BadgeProgress, Metric};
use crate::stats::{ActivityRecord, Range, Summary};
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon};

pub(crate) struct StatsState {
    records: Option<Result<Vec<ActivityRecord>, String>>,
    range: Range,
    /// The Insights page's own `DateRangeFilter` (`30d` to start).
    insights_range: Range,
    /// The heatmap block under the pointer, for its tooltip.
    hovered: Option<usize>,
    /// Bounds of the tracker grid, recorded while painting, so the tooltip
    /// can anchor to the hovered block.
    tracker_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// `useCollectedBadges`: badge id → `collectedAt`.
    collected: Option<Result<BTreeMap<&'static str, String>, String>>,
    /// `CollectNewBadges`: the ids being written, so a set is saved once,
    /// and whether the last save failed (`Couldn't save your new badges.`).
    collecting: Option<Vec<&'static str>>,
    collect_failed: bool,
    /// The badge whose detail dialog is open.
    selected_badge: Option<&'static str>,
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
                insights_range: Range::Days30,
                hovered: None,
                tracker_bounds: Rc::default(),
                collected: None,
                collecting: None,
                collect_failed: false,
                selected_badge: None,
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
                this.collect_new_badges(cx);
            })
            .ok();
        })
        .detach();
        self.reload_collected_badges(cx);
    }

    /// `useCollectedBadges(ownerId)` for the signed-out shell's owner.
    fn reload_collected_badges(&mut self, cx: &mut Context<Self>) {
        let task = self
            .store
            .load_collected_badges(crate::db::DEFAULT_USER_ID.to_string());
        cx.spawn(async move |this, cx| {
            let result = match task.await {
                Ok(Ok(rows)) => Ok(crate::badges::parse_collected_badges(&rows)),
                Ok(Err(error)) => Err(error.to_string()),
                Err(error) => Err(error.to_string()),
            };
            this.update(cx, |this, cx| {
                if let Some(stats) = this.stats.as_mut() {
                    stats.collected = Some(result);
                    cx.notify();
                }
                this.collect_new_badges(cx);
            })
            .ok();
        })
        .detach();
    }

    /// `getBadgeProgress` over the loaded records: signed out, with the
    /// onboarding state from `store.json`.
    fn badge_progress(&self, records: &[ActivityRecord]) -> Vec<BadgeProgress> {
        let now = Utc::now();
        let week_start = self.stats_week_start();
        let onboarding_complete = !self.store_file.onboarding_needed();
        match self
            .provider_settings
            .string_setting("timezone", &["general", "timezone"])
            .and_then(|name| name.parse::<chrono_tz::Tz>().ok())
        {
            Some(tz) => crate::badges::badge_progress(
                records,
                false,
                onboarding_complete,
                now,
                &tz,
                week_start,
            ),
            None => crate::badges::badge_progress(
                records,
                false,
                onboarding_complete,
                now,
                &chrono::Local,
                week_start,
            ),
        }
    }

    /// `CollectNewBadges`: once the records and the collection are both in,
    /// every complete badge not yet collected is written.
    fn collect_new_badges(&mut self, cx: &mut Context<Self>) {
        let Some(stats) = self.stats.as_ref() else {
            return;
        };
        let (Some(Ok(records)), Some(Ok(collected))) = (&stats.records, &stats.collected) else {
            return;
        };
        let new_badges: Vec<&'static str> = self
            .badge_progress(records)
            .into_iter()
            .filter(|badge| badge.complete() && !collected.contains_key(badge.badge.id))
            .map(|badge| badge.badge.id)
            .collect();
        if new_badges.is_empty() || stats.collecting.as_ref() == Some(&new_badges) {
            return;
        }
        let Some(stats) = self.stats.as_mut() else {
            return;
        };
        stats.collecting = Some(new_badges.clone());
        stats.collect_failed = false;
        let task = self
            .store
            .collect_badges(crate::db::DEFAULT_USER_ID.to_string(), new_badges);
        cx.spawn(async move |this, cx| {
            let failed = !matches!(task.await, Ok(Ok(())));
            this.update(cx, |this, cx| {
                if let Some(stats) = this.stats.as_mut() {
                    stats.collect_failed = failed;
                    cx.notify();
                }
                if !failed {
                    this.reload_collected_badges(cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// The `Try again` of a failed save.
    fn retry_collect_badges(&mut self, cx: &mut Context<Self>) {
        if let Some(stats) = self.stats.as_mut() {
            stats.collecting = None;
        }
        self.collect_new_badges(cx);
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
            .child(self.render_badge_collection(stats, records, window, cx))
            .child(div().w_full().child(self.muted_paragraph_xs(
                "Includes imported transcripts. Deleted conversations are excluded.",
                window,
            )))
    }

    /// Overview: the heading, the `bg-muted p-1 rounded-lg` range group of
    /// ghost `size="sm"` buttons, and the three `StatCard`s.
    /// `DateRangeFilter`: the `bg-muted p-1 rounded-lg` group of ghost
    /// `size="sm"` buttons.
    fn render_range_filter(
        &self,
        id: &'static str,
        current: Range,
        on_change: fn(&mut StatsState, Range),
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let ranges = [
            (Range::All, "All time"),
            (Range::Days30, "30 days"),
            (Range::Days7, "7 days"),
        ];
        div()
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
                let active = current == range;
                div()
                    .id(SharedString::from(format!("{id}-range-{label}")))
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
                            on_change(stats, range);
                            cx.notify();
                        }
                    }))
                    .child(div().relative().child(label))
            }))
    }

    /// Overview: the heading, the range filter, and the three `StatCard`s.
    fn render_stats_overview(
        &self,
        stats: &StatsState,
        summary: &Summary,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let group =
            self.render_range_filter("stats", stats.range, |stats, range| stats.range = range, cx);

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
        // The last label overflows its column past the region's edge, so the
        // `overflow-x-auto` region shows WebKit's 6px horizontal scrollbar.
        let tracker_width = stats
            .tracker_bounds
            .get()
            .map(|bounds| f32::from(bounds.size.width))
            .unwrap_or(704.0);
        let region_width = tracker_width + 40.0;
        let pitch = (tracker_width - GAP * (columns as f32 - 1.0)) / columns as f32 + GAP;
        let scroll_width = labels
            .iter()
            .map(|(column, label)| {
                let mut style = window.text_style();
                style.font_size = px(10.0).into();
                if let Some(font) = &self.font_family {
                    style.font_family = font.clone();
                }
                let width = window
                    .text_system()
                    .shape_line(
                        SharedString::from(label.clone()),
                        px(10.0),
                        &[style.to_run(label.len())],
                        None,
                    )
                    .width;
                40.0 + *column as f32 * pitch + f32::from(width)
            })
            .fold(region_width, f32::max);
        let horizontal_scrollbar = (scroll_width > region_width + 0.5).then(|| {
            let thumb = region_width * region_width / scroll_width;
            div()
                .h(px(crate::ui::WEBKIT_SCROLLBAR_WIDTH))
                .w_full()
                .child(
                    div()
                        .h_full()
                        .w(px(thumb))
                        .rounded(px(3.0))
                        .bg(theme.scrollbar_thumb),
                )
        });
        let month_labels = div()
            .relative()
            .overflow_hidden()
            .h(px(15.0))
            .mb_2()
            .ml(px(40.0))
            .text_size(px(10.0))
            .line_height(px(15.0))
            .text_color(theme.muted_foreground)
            .child(
                canvas(|_, _, _| (), {
                    let labels = labels.clone();
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
                    .flex()
                    .flex_col()
                    .min_w(px(620.0))
                    .child(
                        div()
                            .pb_1()
                            .flex()
                            .flex_col()
                            .child(month_labels)
                            .child(div().flex().gap_2().child(weekday_labels).child(tracker)),
                    )
                    .children(horizontal_scrollbar),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .tw_text_xs()
                    .text_color(theme.muted_foreground)
                    .child("Every conversation adds to your story.")
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

    /// `BadgeCollection` → `PersonalBadges` → `BadgeGallery`: the header
    /// with the collected count, the emblem grid, the footer, and the failed
    /// save's `Try again` row.
    fn render_badge_collection(
        &self,
        stats: &StatsState,
        records: &[ActivityRecord],
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let collected = match &stats.collected {
            None => {
                return div()
                    .tw_text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Loading your badges…");
            }
            Some(Err(_)) => {
                return div()
                    .tw_text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Couldn't load your badges. Reopen this page to try again.");
            }
            Some(Ok(collected)) => collected,
        };
        let progress = self.badge_progress(records);
        // `grid-cols-2 min-[480px]:grid-cols-3` over the `max-w-3xl` column.
        let columns = if f32::from(window.viewport_size().width) >= 480.0 {
            3
        } else {
            2
        };
        let mut rows: Vec<Div> = Vec::new();
        for chunk in progress.chunks(columns) {
            let mut row = div().flex().gap_3();
            for badge in chunk {
                row = row.child(self.render_badge_card(*badge, collected.get(badge.badge.id), cx));
            }
            for _ in chunk.len()..columns {
                row = row.child(div().flex_1());
            }
            rows.push(row);
        }
        div()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .tw_text_sm()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child("Your badges"),
                            )
                            .child(
                                div()
                                    .tw_text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child("Little steps worth keeping. Collect them at your own pace."),
                            ),
                    )
                    .child(
                        div()
                            .tw_text_xs()
                            .text_color(theme.muted_foreground)
                            .child(SharedString::from(format!(
                                "{} of {} collected",
                                format_number(collected.len() as f64, 0),
                                format_number(progress.len() as f64, 0)
                            ))),
                    ),
            )
            .child(div().flex().flex_col().gap_3().children(rows))
            .child(div().w_full().child(self.muted_paragraph_xs(
                "Collected badges stay yours on this device, even when you delete a note or take a break. Imported transcripts count; the welcome demo doesn't.",
                window,
            )))
            .when(stats.collect_failed, |section| {
                section.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .tw_text_xs()
                        .text_color(theme.muted_foreground)
                        .child("Couldn't save your new badges.")
                        .child(
                            div()
                                .id("badges-retry")
                                .flex()
                                .h(px(32.0))
                                .items_center()
                                .px_3()
                                .rounded_md()
                                .tw_text_sm()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(theme.foreground)
                                .cursor_pointer()
                                .hover(move |style| style.bg(theme.accent))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.retry_collect_badges(cx)
                                }))
                                .child("Try again"),
                        ),
                )
            })
    }

    /// One gallery button: `bg-background border rounded-2xl px-3 py-5
    /// hover:bg-muted`, the 96px emblem, the name and the progress line.
    fn render_badge_card(
        &self,
        badge: BadgeProgress,
        collected_at: Option<&String>,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let id = badge.badge.id;
        let (name, _) = crate::badges::badge_details(id);
        let hovered = self.hovered == Some(id);
        let status = if collected_at.is_some() {
            "Collected".to_string()
        } else {
            badge_progress_label(&badge)
        };
        div().flex_1().flex().child(
            div()
                .id(SharedString::from(format!("badge-{id}")))
                .relative()
                .flex()
                .flex_1()
                .flex_col()
                .items_center()
                .gap_3()
                .rounded(px(16.0))
                .border_1()
                .border_color(theme.border)
                .bg(if hovered {
                    theme.muted
                } else {
                    theme.background
                })
                .px_3()
                .py_5()
                .cursor_pointer()
                .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                    this.set_hovered(id, *hovering, cx);
                }))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(stats) = this.stats.as_mut() {
                        stats.selected_badge = Some(id);
                        cx.notify();
                    }
                }))
                .child(self.render_badge_emblem(id, collected_at.is_some(), false))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_1()
                        .text_center()
                        .child(
                            div()
                                .tw_text_sm()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(theme.foreground)
                                .child(name),
                        )
                        .child(
                            div()
                                .tw_text_xs()
                                .text_color(theme.muted_foreground)
                                .child(SharedString::from(status)),
                        ),
                ),
        )
    }

    /// `BadgeEmblem`: the illustration (`mix-blend-multiply`, inverted and
    /// screened in dark mode — pre-rendered as an alpha mask in the badge
    /// colour), at 30% until collected, with the `Check` pill once it is.
    fn render_badge_emblem(&self, id: &str, collected: bool, large: bool) -> Div {
        let theme = self.theme;
        let size = if large { 144.0 } else { 96.0 };
        let asset = if theme.dark {
            format!("badges/{id}-dark.webp")
        } else {
            format!("badges/{id}.webp")
        };
        div()
            .relative()
            .flex()
            .flex_shrink_0()
            .size(px(size))
            .child(
                img(super::note::embedded(&asset))
                    .size(px(size))
                    .when(!collected, |image| image.opacity(0.3)),
            )
            .when(collected, |emblem| {
                emblem.child(
                    div()
                        .absolute()
                        .right_0()
                        .bottom_0()
                        .flex()
                        .size(px(20.0))
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.background)
                        .child(icon("check", px(12.0), theme.foreground)),
                )
            })
    }

    /// The badge `Dialog`: `DialogContent max-w-sm rounded-2xl` over the
    /// `bg-black/80` overlay, with the large emblem, the centred title and
    /// description, and the collection date or the progress bar.
    pub(super) fn render_badge_dialog(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let stats = self.stats.as_ref()?;
        let id = stats.selected_badge?;
        let records = match &stats.records {
            Some(Ok(records)) => records,
            _ => return None,
        };
        let badge = self
            .badge_progress(records)
            .into_iter()
            .find(|badge| badge.badge.id == id)?;
        let collected_at = stats
            .collected
            .as_ref()
            .and_then(|collected| collected.as_ref().ok())
            .and_then(|collected| collected.get(id))
            .cloned();
        let theme = self.theme;
        let (name, description) = crate::badges::badge_details(id);
        let ratio = (badge.value as f32 / badge.badge.target.max(1) as f32).clamp(0.0, 1.0);
        let card = div()
            .id("badge-dialog")
            .relative()
            .w(px(384.0))
            .flex()
            .flex_col()
            .gap_4()
            .p_6()
            .rounded(px(16.0))
            .border_1()
            .border_color(theme.border)
            .bg(theme.background)
            .shadow(vec![gpui::BoxShadow {
                color: gpui::hsla(0.0, 0.0, 0.0, 0.1),
                offset: gpui::point(px(0.0), px(10.0)),
                blur_radius: px(15.0),
                spread_radius: px(-3.0),
            }])
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .justify_center()
                    .py_2()
                    .child(self.render_badge_emblem(id, collected_at.is_some(), true)),
            )
            .child(
                // `DialogTitle`: `text-lg leading-none font-semibold`.
                div()
                    .text_size(px(18.0))
                    .line_height(px(18.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_center()
                    .text_color(theme.foreground)
                    .child(name),
            )
            .child(
                // `DialogDescription`: a centred `text-sm text-muted-foreground` p.
                div().w_full().child(
                    self.muted_paragraph(description, 14.0, 20.0, window)
                        .centered()
                        .max_width(px(336.0)),
                ),
            )
            .map(|card| match &collected_at {
                Some(collected_at) => card.child(
                    div()
                        .tw_text_xs()
                        .text_center()
                        .text_color(theme.muted_foreground)
                        .child(SharedString::from(format!(
                            "Collected {}",
                            medium_date(collected_at)
                        ))),
                ),
                None => card.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .tw_text_xs()
                                .text_center()
                                .text_color(theme.muted_foreground)
                                .child(SharedString::from(badge_progress_label(&badge))),
                        )
                        .child(
                            // `ProgressBar`: `h-2 bg-muted rounded-full` track,
                            // `bg-foreground/80` fill.
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
                        ),
                ),
            })
            .child(
                // `DialogPrimitive.Close`: `absolute top-4 right-4 opacity-70`.
                div()
                    .id("badge-dialog-close")
                    .absolute()
                    .top_4()
                    .right_4()
                    .flex()
                    .size(px(16.0))
                    .items_center()
                    .justify_center()
                    .rounded(px(2.0))
                    .opacity(0.7)
                    .cursor_pointer()
                    .hover(|style| style.opacity(1.0))
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| this.close_badge_dialog(cx)),
                    )
                    .child(icon("x", px(16.0), theme.foreground)),
            );
        Some(
            gpui::deferred(
                div()
                    .id("badge-dialog-overlay")
                    .occlude()
                    .absolute()
                    .inset_0()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gpui::hsla(0.0, 0.0, 0.0, 0.8))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| this.close_badge_dialog(cx)),
                    )
                    .child(card),
            )
            .with_priority(5)
            .into_any_element(),
        )
    }

    pub(super) fn badge_dialog_open(&self) -> bool {
        self.stats
            .as_ref()
            .is_some_and(|stats| stats.selected_badge.is_some())
    }

    pub(super) fn close_badge_dialog(&mut self, cx: &mut Context<Self>) {
        if let Some(stats) = self.stats.as_mut()
            && stats.selected_badge.take().is_some()
        {
            cx.notify();
        }
    }

    /// `SettingsInsights` (`insights.tsx`): the weekday pattern panel, the
    /// week-at-a-glance bars, and the typical length / per-day cards.
    pub(super) fn render_insights_settings(
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
                .child("Patterns in your captured conversations."),
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
                        .child("Loading your insights…"),
                );
            }
            Some(Err(_)) => {
                return page.child(header).child(
                    div()
                        .tw_text_sm()
                        .text_color(theme.muted_foreground)
                        .child("Couldn't load your insights. Reopen this page to try again."),
                );
            }
            Some(Ok(records)) => records,
        };
        let summary = self.summarize_stats(records, stats.insights_range);
        let weekdays: Vec<(&'static str, usize)> = summary
            .weekday_counts
            .iter()
            .map(|(weekday, count)| (weekday_long(*weekday), *count))
            .collect();
        let peak = weekdays.iter().map(|(_, count)| *count).max().unwrap_or(0);
        let busiest: Vec<&str> = weekdays
            .iter()
            .filter(|(_, count)| *count == peak)
            .map(|(label, _)| *label)
            .collect();
        let has_patterns = summary.conversations >= 5;
        let total = format_number(summary.conversations as f64, 0);
        let peak_count = format_number(peak as f64, 0);
        let share = if summary.conversations > 0 {
            format!(
                "{}%",
                ((peak as f64 / summary.conversations as f64) * 100.0).round()
            )
        } else {
            "0%".to_string()
        };
        let (headline, detail) = if !has_patterns {
            (
                "A little more history will help".to_string(),
                "Capture at least 5 conversations in this period to see patterns.".to_string(),
            )
        } else if busiest.len() == 1 {
            (
                format!("Most conversations: {}", busiest[0]),
                format!(
                    "{peak_count} of {total} conversations ({share}) started on this weekday in the selected period."
                ),
            )
        } else {
            (
                "No single busiest day".to_string(),
                format!(
                    "Your busiest weekdays are tied at {peak_count} conversations each in the selected period."
                ),
            )
        };
        let detail_prose = self.muted_paragraph(&detail, 14.0, 20.0, window);

        // `InsightPanel`: `rounded-[20px] border p-5 flex items-start gap-3`.
        let pattern_panel = div()
            .relative()
            .flex()
            .items_start()
            .gap_3()
            .p_5()
            .child(crate::squircle::squircle(
                crate::squircle::PANEL_RADIUS,
                None,
                Some((1.0, theme.border)),
            ))
            .child(div().relative().mt(px(2.0)).flex_shrink_0().child(icon(
                "chart-line-up",
                px(20.0),
                theme.muted_foreground,
            )))
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .tw_text_base()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child(SharedString::from(headline)),
                    )
                    .child(detail_prose),
            );

        let mut page = page
            .child(header)
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
                            .child("Your conversation patterns"),
                    )
                    .child(self.render_range_filter(
                        "insights",
                        stats.insights_range,
                        |stats, range| stats.insights_range = range,
                        cx,
                    )),
            )
            .child(pattern_panel);

        if summary.conversations > 0 {
            // `Your week at a glance`: `w-24` labels, `h-5 bg-muted rounded-sm`
            // bars filled to the day's share of the peak, `w-10` counts.
            let bars = div()
                .flex()
                .flex_col()
                .gap_3()
                .children(weekdays.iter().map(|(label, count)| {
                    let fill = if peak > 0 {
                        *count as f32 / peak as f32
                    } else {
                        0.0
                    };
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .tw_text_xs()
                        .child(
                            div()
                                .w(px(96.0))
                                .flex_shrink_0()
                                .text_color(theme.muted_foreground)
                                .child(*label),
                        )
                        .child(
                            div()
                                .h(px(20.0))
                                .min_w_0()
                                .flex_1()
                                .overflow_hidden()
                                .rounded(px(2.0))
                                .bg(theme.muted)
                                .child(div().h_full().w(gpui::relative(fill)).rounded(px(2.0)).bg(
                                    alpha(
                                        theme.foreground,
                                        if has_patterns && *count == peak {
                                            0.7
                                        } else {
                                            0.2
                                        },
                                    ),
                                )),
                        )
                        .child(
                            div()
                                .w(px(40.0))
                                .flex_shrink_0()
                                .text_right()
                                .child(SharedString::from(format_number(*count as f64, 0))),
                        )
                }));
            page = page.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_5()
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
                                    .child("Your week at a glance"),
                            )
                            .child(
                                div()
                                    .tw_text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(SharedString::from(format!("Conversations: {total}"))),
                            ),
                    )
                    .child(bars),
            );
            if has_patterns {
                let card = |label: &'static str, value: String, note: String| {
                    div()
                        .relative()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_2()
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
                            div()
                                .relative()
                                .text_size(px(24.0))
                                .line_height(px(32.0))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child(SharedString::from(value)),
                        )
                        .child(
                            div()
                                .relative()
                                .tw_text_xs()
                                .text_color(theme.muted_foreground)
                                .child(SharedString::from(note)),
                        )
                };
                let median = match summary.median_minutes {
                    None => "Not available".to_string(),
                    Some(minutes) => format!("{} min", format_number(minutes, 1)),
                };
                let per_day = if summary.conversation_days > 0 {
                    summary.conversations as f64 / summary.conversation_days as f64
                } else {
                    0.0
                };
                let two_columns = f32::from(window.viewport_size().width) >= 480.0;
                let cards = [
                    card(
                        "Typical conversation length",
                        median,
                        format!(
                            "Median transcribed time. Conversations with timing: {}.",
                            format_number(summary.timed_conversations as f64, 0)
                        ),
                    ),
                    card(
                        "Conversations per active day",
                        format_number(per_day, 1),
                        "Average across the capture days counted above.".to_string(),
                    ),
                ];
                page = page.child(
                    div()
                        .flex()
                        .when(!two_columns, |grid| grid.flex_col())
                        .gap_3()
                        .children(cards),
                );
            }
        }
        page.child(div().w_full().child(self.muted_paragraph_xs(
            "Based on captured conversations, including imported transcripts. Each conversation is counted once, on its first capture day in the selected period, using your calendar timezone. Deleted conversations are excluded.",
            window,
        )))
    }
}

impl Workspace {
    /// A `p` at `text-xs text-muted-foreground`: 12px / 16px, wrapped pretty
    /// like every paragraph in the app.
    fn muted_paragraph_xs(&self, text: &str, window: &Window) -> crate::prose_text::ProseText {
        self.muted_paragraph(text, 12.0, 16.0, window)
    }

    fn muted_paragraph(
        &self,
        text: &str,
        font_px: f32,
        line: f32,
        window: &Window,
    ) -> crate::prose_text::ProseText {
        let mut style = window.text_style();
        style.font_size = px(font_px).into();
        style.color = self.theme.muted_foreground.into();
        if let Some(font) = &self.font_family {
            style.font_family = font.clone();
        }
        crate::prose_text::ProseText::new(
            text.to_string(),
            vec![style.to_run(text.len())],
            px(font_px),
            px(line),
        )
        .pretty()
    }
}

/// `progressLabel` per metric.
fn badge_progress_label(badge: &BadgeProgress) -> String {
    let value = format_number(badge.value as f64, 0);
    let target = format_number(badge.badge.target as f64, 0);
    match badge.badge.metric {
        Metric::Conversations => format!("{value} / {target} conversations"),
        Metric::Weeks => format!("{value} / {target} active weeks"),
        Metric::Signup => "Create your account".to_string(),
        Metric::Onboarding => "Complete onboarding".to_string(),
    }
}

/// `Intl.DateTimeFormat("en-US", { dateStyle: "medium" })` of an ISO stamp,
/// in the local zone.
fn medium_date(iso: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(iso) {
        Ok(date) => {
            let local = date.with_timezone(&chrono::Local);
            format!(
                "{} {}, {}",
                month_short(local.month()),
                local.day(),
                local.year()
            )
        }
        Err(_) => iso.to_string(),
    }
}

fn weekday_long(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
        Weekday::Sun => "Sunday",
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
