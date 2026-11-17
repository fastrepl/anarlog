//! `settings/stats`: conversation counts, weekday patterns and the past year's activity,
//! computed from transcripts like shipping's `ACTIVITY_SQL` and `summarizeActivity`.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use desktop_runtime::{CancellationToken, Result, RuntimeHandle, ServiceError};
use gpui::{Context, Hsla, Render, SharedString, Window, div, prelude::*, px, relative};
use serde_json::json;

use crate::ui::theme::{Theme, theme};

use super::{controls, locale, settings};
use crate::ui::a11y::Accessible;

mod badges;

const DEFAULT_USER_ID: &str = "00000000-0000-0000-0000-000000000000";

const ACTIVITY_SQL: &str = "
  SELECT
    transcript.session_id,
    transcript.started_at_ms,
    transcript.created_at,
    CASE WHEN json_valid(session.event_json)
      THEN COALESCE(json_extract(session.event_json, '$.tracking_id') = 'anarlog-onboarding-demo-v1', 0)
      ELSE 0 END AS is_demo,
    MAX(CASE
      WHEN word.type = 'object'
        AND json_type(word.value, '$.end_ms') IN ('integer', 'real')
      THEN MAX(0, json_extract(word.value, '$.end_ms'))
      ELSE 0
    END) AS duration_ms
  FROM transcripts AS transcript
  JOIN sessions AS session ON session.id = transcript.session_id
  JOIN json_each(CASE
    WHEN json_valid(transcript.words_json) THEN
      CASE WHEN json_type(transcript.words_json) = 'array'
        THEN transcript.words_json ELSE '[]' END
    ELSE '[]'
  END) AS word
  WHERE session.deleted_at IS NULL
    AND transcript.deleted_at IS NULL
    AND COALESCE(session.owner_user_id, '') IN (?, '', '00000000-0000-0000-0000-000000000000')
    AND CASE WHEN word.type = 'object'
      THEN LENGTH(TRIM(COALESCE(json_extract(word.value, '$.text'), ''))) > 0
      ELSE 0 END
  GROUP BY transcript.id
";

#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub session_id: String,
    /// Milliseconds since the epoch.
    pub started_at: i64,
    pub duration_ms: i64,
    /// The onboarding welcome demo, which badges do not count.
    pub is_demo: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Range {
    All,
    Days30,
    Days7,
}

#[derive(Debug, Default, PartialEq)]
pub struct Stats {
    pub conversations: usize,
    pub hours: f64,
    pub active_days: usize,
    pub streak: usize,
    /// The heatmap's days from the start of the week a year ago through today.
    pub days: Vec<(NaiveDate, usize)>,
    /// Session counts by weekday (0 = Sunday), in week order.
    pub weekdays: Vec<(u32, usize)>,
    pub median_minutes: Option<f64>,
    pub timed_conversations: usize,
    pub conversation_days: usize,
}

fn week_start(date: NaiveDate, monday: bool) -> NaiveDate {
    let offset = if monday {
        date.weekday().num_days_from_monday()
    } else {
        date.weekday().num_days_from_sunday()
    };
    date - Duration::days(offset.into())
}

/// `summarizeActivity`.
pub fn summarize(
    records: &[Record],
    now: DateTime<Utc>,
    zone: Tz,
    monday: bool,
    range: Range,
) -> Stats {
    let today = now.with_timezone(&zone).date_naive();
    let cutoff = match range {
        Range::All => None,
        Range::Days30 => Some(today - Duration::days(29)),
        Range::Days7 => Some(today - Duration::days(6)),
    };
    let now_ms = now.timestamp_millis();
    let mut sessions = BTreeSet::new();
    let mut daily: BTreeMap<NaiveDate, BTreeSet<&str>> = BTreeMap::new();
    let mut weeks = BTreeSet::new();
    let mut intervals: HashMap<&str, Vec<(i64, i64)>> = HashMap::new();
    let mut starts: HashMap<&str, DateTime<Tz>> = HashMap::new();
    for record in records {
        if record.started_at > now_ms {
            continue;
        }
        let Some(date) = zone.timestamp_millis_opt(record.started_at).single() else {
            continue;
        };
        let day = date.date_naive();
        daily.entry(day).or_default().insert(&record.session_id);
        weeks.insert(week_start(day, monday));
        if cutoff.is_some_and(|cutoff| day < cutoff) {
            continue;
        }
        sessions.insert(record.session_id.as_str());
        let first = starts.entry(&record.session_id).or_insert(date);
        if date < *first {
            *first = date;
        }
        if record.duration_ms > 0 {
            let end = (record.started_at + record.duration_ms).min(now_ms);
            intervals
                .entry(&record.session_id)
                .or_default()
                .push((record.started_at, end));
        }
    }
    let mut total_ms = 0;
    let mut durations = Vec::new();
    for spans in intervals.values_mut() {
        spans.sort();
        let (mut previous_end, mut session_ms) = (0, 0);
        for (start, end) in spans.iter() {
            session_ms += (end - start.max(&previous_end)).max(0);
            previous_end = previous_end.max(*end);
        }
        total_ms += session_ms;
        if session_ms > 0 {
            durations.push(session_ms);
        }
    }
    durations.sort();
    let middle = durations.len() / 2;
    let median_minutes = (!durations.is_empty()).then(|| {
        let median = if durations.len() % 2 == 1 {
            durations[middle] as f64
        } else {
            (durations[middle - 1] + durations[middle]) as f64 / 2.
        };
        median / 60_000.
    });
    let first_weekday = u32::from(monday);
    let mut weekdays: Vec<(u32, usize)> = (0..7).map(|i| ((i + first_weekday) % 7, 0)).collect();
    for date in starts.values() {
        let weekday = date.weekday().num_days_from_sunday();
        weekdays[((weekday + 7 - first_weekday) % 7) as usize].1 += 1;
    }
    let mut week = week_start(today, monday);
    if !weeks.contains(&week) {
        week -= Duration::days(7);
    }
    let mut streak = 0;
    while weeks.contains(&week) {
        streak += 1;
        week -= Duration::days(7);
    }
    let mut days = Vec::new();
    let mut day = week_start(today - Duration::days(364), monday);
    while day <= today {
        days.push((day, daily.get(&day).map_or(0, BTreeSet::len)));
        day += Duration::days(1);
    }
    Stats {
        conversations: sessions.len(),
        hours: total_ms as f64 / 3_600_000.,
        active_days: daily
            .keys()
            .filter(|day| cutoff.is_none_or(|cutoff| **day >= cutoff))
            .count(),
        streak,
        days,
        weekdays,
        median_minutes,
        timed_conversations: durations.len(),
        conversation_days: starts
            .values()
            .map(|date| date.date_naive())
            .collect::<BTreeSet<_>>()
            .len(),
    }
}

/// Formats like `Intl.NumberFormat` with at most one fraction digit.
fn number(value: f64) -> String {
    let rounded = (value * 10.).round() / 10.;
    if rounded.fract() == 0. {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded:.1}")
    }
}

const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

pub struct InsightsView {
    runtime: RuntimeHandle,
    user_id: String,
    records: Option<Result<Vec<Record>>>,
    zone: Tz,
    monday: bool,
    range: Range,
    cancel: CancellationToken,
    collection: Option<Result<badges::Collection>>,
    /// Saving newly earned badges failed; shipping offers "Try again".
    collect_failed: bool,
    selected_badge: Option<&'static str>,
    dialog_focus: gpui::FocusHandle,
}

impl InsightsView {
    pub fn new(runtime: RuntimeHandle, user_id: String, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            runtime,
            user_id,
            records: None,
            zone: locale::system_timezone().parse().unwrap_or(Tz::UTC),
            monday: locale::system_week_start() == "monday",
            range: Range::All,
            cancel: CancellationToken::new(),
            collection: None,
            collect_failed: false,
            selected_badge: None,
            dialog_focus: cx.focus_handle(),
        };
        this.load(cx);
        this.load_badges(cx);
        this
    }

    fn signed_up(&self) -> bool {
        self.user_id != DEFAULT_USER_ID
    }

    fn badge_progress(&self) -> Option<Vec<u32>> {
        let records = self.records.as_ref()?.as_ref().ok()?;
        let collection = self.collection.as_ref()?.as_ref().ok()?;
        Some(badges::progress(
            records,
            self.signed_up(),
            collection.onboarding_complete,
            Utc::now(),
            self.zone,
            self.monday,
        ))
    }

    fn load_badges(&mut self, cx: &mut Context<Self>) {
        let runtime = self.runtime.clone();
        let owner = self.user_id.clone();
        let art = cx
            .background_executor()
            .spawn(async { badges::decode_emblems() });
        cx.spawn(async move |this, cx| {
            let collection = badges::load(&runtime, owner).await;
            let _ = this.update(cx, |this, cx| {
                this.collection = Some(collection);
                this.collect_new(cx);
                cx.notify();
            });
            art.await;
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }

    /// Shipping's `CollectNewBadges`: badges that reached their target are saved as collected.
    fn collect_new(&mut self, cx: &mut Context<Self>) {
        let Some(progress) = self.badge_progress() else {
            return;
        };
        let Some(Ok(collection)) = &self.collection else {
            return;
        };
        let ids: Vec<&'static str> = badges::BADGES
            .iter()
            .zip(progress)
            .filter(|(badge, value)| {
                *value >= badge.target && !collection.collected.contains_key(badge.id)
            })
            .map(|(badge, _)| badge.id)
            .collect();
        if ids.is_empty() {
            return;
        }
        self.collect_failed = false;
        let runtime = self.runtime.clone();
        let owner = self.user_id.clone();
        cx.spawn(async move |this, cx| {
            let saved = badges::collect(&runtime, owner.clone(), ids).await;
            let reloaded = match saved {
                Ok(()) => Some(badges::load(&runtime, owner).await),
                Err(_) => None,
            };
            let _ = this.update(cx, |this, cx| {
                match reloaded {
                    Some(collection) => this.collection = Some(collection),
                    None => this.collect_failed = true,
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let user = self.user_id.clone();
        let records = self
            .runtime
            .read(self.cancel.clone(), move |services| async move {
                services
                    .executor
                    .execute(ACTIVITY_SQL.into(), vec![json!(user)])
                    .await
                    .map_err(|error| ServiceError::Failed(error.to_string().into()))
            });
        let preferences = settings::watch(&self.runtime);
        cx.spawn(async move |this, cx| {
            let snapshot = match preferences {
                Ok(reply) => match reply.receive().await {
                    Ok(watch) => {
                        let snapshot = settings::decode(&watch.snapshots.borrow().rows).ok();
                        let _ = watch.unsubscribe().await;
                        snapshot
                    }
                    Err(_) => None,
                },
                Err(_) => None,
            };
            let rows = match records {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let records = rows.map(|rows| {
                rows.iter()
                    .filter_map(|row| {
                        let started = row["started_at_ms"].as_i64().unwrap_or(0);
                        let started = if started > 0 {
                            started
                        } else {
                            DateTime::parse_from_rfc3339(row["created_at"].as_str()?)
                                .ok()?
                                .timestamp_millis()
                        };
                        Some(Record {
                            session_id: row["session_id"].as_str()?.to_owned(),
                            started_at: started,
                            duration_ms: row["duration_ms"].as_f64().unwrap_or(0.) as i64,
                            is_demo: row["is_demo"].as_i64().unwrap_or(0) != 0,
                        })
                    })
                    .collect()
            });
            let _ = this.update(cx, |this, cx| {
                if let Some(snapshot) = snapshot {
                    let text = |key: &str| {
                        snapshot
                            .get(key)
                            .and_then(|setting| setting.value.as_str())
                            .unwrap_or_default()
                            .to_owned()
                    };
                    if let Ok(zone) = text("timezone").parse() {
                        this.zone = zone;
                    }
                    match text("week_start").as_str() {
                        "monday" => this.monday = true,
                        "sunday" => this.monday = false,
                        _ => {}
                    }
                }
                this.records = Some(records);
                this.collect_new(cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn range_filter(&self, colors: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        div().flex().gap_1().p_1().bg(colors.muted).children(
            [
                (Range::All, "All time"),
                (Range::Days30, "30 days"),
                (Range::Days7, "7 days"),
            ]
            .into_iter()
            .map(|(range, label)| {
                let selected = range == self.range;
                div()
                    .id(label)
                    .px_3()
                    .py(px(6.))
                    .text_xs()
                    .line_height(px(16.))
                    .font_weight(crate::ui::theme::MEDIUM)
                    .cursor_pointer()
                    .when(selected, |view| {
                        view.bg(colors.background)
                            .text_color(colors.foreground)
                            .shadow_xs()
                    })
                    .when(!selected, |view| {
                        view.text_color(colors.muted_foreground)
                            .hover(|style| style.text_color(colors.foreground))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.range = range;
                        cx.notify();
                    }))
                    .child(label)
            }),
        )
    }
}

fn heading(text: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_sm()
        .line_height(px(20.))
        .font_weight(crate::ui::theme::MEDIUM)
        .child(text.into())
}

fn panel(colors: &Theme) -> gpui::Div {
    div().border_1().border_color(colors.border)
}

fn patterns(stats: &Stats, colors: &Theme) -> gpui::Div {
    let peak = stats
        .weekdays
        .iter()
        .map(|(_, count)| *count)
        .max()
        .unwrap_or(0);
    let busiest = stats
        .weekdays
        .iter()
        .filter(|(_, count)| *count == peak)
        .collect::<Vec<_>>();
    let has_patterns = stats.conversations >= 5;
    let total = stats.conversations;
    let (title, body) = if !has_patterns {
        (
            "A little more history will help".to_owned(),
            "Capture at least 5 conversations in this period to see patterns.".to_owned(),
        )
    } else if busiest.len() == 1 {
        (
            format!("Most conversations: {}", WEEKDAYS[busiest[0].0 as usize]),
            format!(
                "{peak} of {total} conversations ({}%) started on this weekday in the selected period.",
                (peak as f64 / total as f64 * 100.).round()
            ),
        )
    } else {
        (
            "No single busiest day".to_owned(),
            format!(
                "Your busiest weekdays are tied at {peak} conversations each in the selected period."
            ),
        )
    };
    let small = |text: String| {
        div()
            .text_xs()
            .line_height(px(16.))
            .text_color(colors.muted_foreground)
            .child(text)
    };
    let figure = |label: &'static str, value: String, note: String| {
        panel(colors)
            .flex_1()
            .p_4()
            .flex()
            .flex_col()
            .gap_2()
            .child(small(label.into()))
            .child(
                div()
                    .text_2xl()
                    .line_height(px(32.))
                    .font_weight(crate::ui::theme::MEDIUM)
                    .child(value),
            )
            .child(small(note))
    };
    div()
        .flex()
        .flex_col()
        .gap_8()
        .child(heading("Your conversation patterns"))
        .child(
            panel(colors)
                .p_5()
                .flex()
                .items_start()
                .gap_3()
                .child(
                    controls::icon("product/ChartLineData01Icon.svg", 20., colors.muted_foreground)
                        .mt(px(2.)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .text_base()
                                .line_height(px(24.))
                                .font_weight(crate::ui::theme::MEDIUM)
                                .child(title),
                        )
                        .child(
                            div()
                                .text_sm()
                                .line_height(px(20.))
                                .text_color(colors.muted_foreground)
                                .child(body),
                        ),
                ),
        )
        .when(total > 0, |view| {
            view.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_5()
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_baseline()
                            .justify_between()
                            .gap_2()
                            .child(heading("Your week at a glance"))
                            .child(small(format!("Conversations: {total}"))),
                    )
                    .child(div().flex().flex_col().gap_3().children(stats.weekdays.iter().map(
                        |(weekday, count)| {
                            let strong = has_patterns && *count == peak;
                            div()
                                .flex()
                                .items_center()
                                .gap_3()
                                .text_xs()
                                .line_height(px(16.))
                                .child(
                                    div()
                                        .w(px(96.))
                                        .flex_shrink_0()
                                        .text_color(colors.muted_foreground)
                                        .child(WEEKDAYS[*weekday as usize]),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .h(px(20.))
                                        .overflow_hidden()
                                        .bg(colors.muted)
                                        .child(
                                            div()
                                                .h_full()
                                                .w(relative(*count as f32 / peak.max(1) as f32))
                                                .bg(colors.foreground.opacity(if strong { 0.7 } else { 0.2 })),
                                        ),
                                )
                                .child(div().w(px(40.)).flex_shrink_0().text_right().child(count.to_string()))
                        },
                    ))),
            )
            .when(has_patterns, |view| {
                view.child(
                    div()
                        .flex()
                        .gap_3()
                        .child(figure(
                            "Typical conversation length",
                            stats
                                .median_minutes
                                .map_or("Not available".into(), |minutes| {
                                    format!("{} min", number(minutes))
                                }),
                            format!(
                                "Median transcribed time. Conversations with timing: {}.",
                                stats.timed_conversations
                            ),
                        ))
                        .child(figure(
                            "Conversations per active day",
                            number(total as f64 / stats.conversation_days.max(1) as f64),
                            "Average across the capture days counted above.".into(),
                        )),
                )
            })
        })
        .child(small(
            "Based on captured conversations, including imported transcripts. Each conversation is counted once, on its first capture day in the selected period, using your calendar timezone. Deleted conversations are excluded.".into(),
        ))
}

/// The tremor `Tracker` heatmap: one column per week, one square per day.
fn heatmap(stats: &Stats, monday: bool, colors: &Theme) -> gpui::Div {
    let shades: [Hsla; 5] = [
        colors.muted,
        colors.foreground.opacity(0.2),
        colors.foreground.opacity(0.4),
        colors.foreground.opacity(0.6),
        colors.foreground.opacity(0.8),
    ];
    let weeks = stats.days.chunks(7).collect::<Vec<_>>();
    let short = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let first = u32::from(monday);
    let mut previous_month = None;
    let months = weeks.iter().map(|week| {
        let month = week[0].0.month();
        let label = (previous_month != Some(month)).then(|| week[0].0.format("%b").to_string());
        previous_month = Some(month);
        div()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .whitespace_nowrap()
            .child(label.unwrap_or_default())
    });
    div()
        .child(
            div()
                .mb_2()
                .ml(px(40.))
                .flex()
                .gap(px(3.))
                .text_size(px(10.))
                .text_color(colors.muted_foreground)
                .children(months),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(
                    div()
                        .w(px(32.))
                        .flex_shrink_0()
                        .flex()
                        .flex_col()
                        .gap(px(3.))
                        .text_size(px(9.))
                        .text_color(colors.muted_foreground)
                        .children((0..7).map(|row| {
                            div().flex_1().flex().items_center().child(if row % 2 == 1 {
                                short[((row + first) % 7) as usize]
                            } else {
                                ""
                            })
                        })),
                )
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .gap(px(3.))
                        .children(weeks.iter().map(|week| {
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap(px(3.))
                                .children(week.iter().map(|(_, count)| {
                                    div().w_full().h(px(10.)).bg(shades[(*count).min(4)])
                                }))
                        })),
                ),
        )
}

impl InsightsView {
    /// `BadgeCollection`.
    fn badge_collection(&self, colors: &Theme, cx: &mut Context<Self>) -> gpui::AnyElement {
        let small = |text: SharedString| {
            div()
                .text_xs()
                .line_height(px(16.))
                .text_color(colors.muted_foreground)
                .child(text)
        };
        let collection = match &self.collection {
            None => {
                return div()
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child("Loading your badges…")
                    .into_any_element();
            }
            Some(Err(_)) => {
                return div()
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child("Couldn't load your badges. Reopen this page to try again.")
                    .into_any_element();
            }
            Some(Ok(collection)) => collection,
        };
        let Some(progress) = self.badge_progress() else {
            return div().into_any_element();
        };
        let cards = badges::BADGES.iter().zip(progress).map(|(badge, value)| {
            let collected = collection.collected.contains_key(badge.id);
            let id = badge.id;
            div()
                .id(SharedString::from(format!("badge-{id}")))
                .size_full()
                .px_3()
                .py_5()
                .flex()
                .flex_col()
                .items_center()
                .gap_3()
                .border_1()
                .border_color(colors.border)
                .bg(colors.background)
                .cursor_pointer()
                .hover(|style| style.bg(colors.muted))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.selected_badge = Some(id);
                    this.dialog_focus.focus(window, cx);
                    cx.notify();
                }))
                .child(badges::emblem(id, collected, false, colors))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_1()
                        .text_center()
                        .child(
                            div()
                                .text_sm()
                                .line_height(px(20.))
                                .font_weight(crate::ui::theme::MEDIUM)
                                .child(badge.name),
                        )
                        .child(small(if collected {
                            "Collected".into()
                        } else {
                            badges::progress_label(badge, value)
                        })),
                )
        });
        div()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_baseline()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(heading("Your badges"))
                            .child(small(
                                "Little steps worth keeping. Collect them at your own pace.".into(),
                            )),
                    )
                    .child(small(
                        format!(
                            "{} of {} collected",
                            collection.collected.len(),
                            badges::BADGES.len()
                        )
                        .into(),
                    )),
            )
            .child(div().grid().grid_cols(3).gap_3().children(cards))
            .child(small(
                "Collected badges stay yours on this device, even when you delete a note or take a break. Imported transcripts count; the welcome demo doesn't.".into(),
            ))
            .when(self.collect_failed, |view| {
                view.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .text_xs()
                        .text_color(colors.muted_foreground)
                        .child("Couldn't save your new badges.")
                        .child(
                            div()
                                .id("badges-retry")
                                .a11y_button("Try again")
                                .h_7()
                                .px_2()
                                .flex()
                                .items_center()
                                .font_weight(crate::ui::theme::MEDIUM)
                                .text_color(colors.foreground)
                                .cursor_pointer()
                                .hover(|style| style.bg(colors.accent))
                                .on_click(cx.listener(|this, _, _, cx| this.collect_new(cx)))
                                .child("Try again"),
                        ),
                )
            })
            .into_any_element()
    }

    /// The badge `Dialog`: artwork, name, description, then the collected date or progress.
    fn badge_dialog(&self, window: &Window, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let id = self.selected_badge?;
        let colors = theme(window);
        let Some(Ok(collection)) = &self.collection else {
            return None;
        };
        let index = badges::BADGES.iter().position(|badge| badge.id == id)?;
        let badge = &badges::BADGES[index];
        let value = self.badge_progress()?.get(index).copied().unwrap_or(0);
        let collected = collection.collected.get(id).copied();
        let small = |text: SharedString| {
            div()
                .text_xs()
                .line_height(px(16.))
                .text_color(colors.muted_foreground)
                .text_center()
                .child(text)
        };
        let status = match collected {
            Some(at) => small(
                format!(
                    "Collected {}",
                    at.with_timezone(&chrono::Local).format("%b %-d, %Y")
                )
                .into(),
            ),
            None => div()
                .flex()
                .flex_col()
                .gap_3()
                .child(small(badges::progress_label(badge, value)))
                .child(
                    div()
                        .h_2()
                        .w_full()
                        .overflow_hidden()
                        .bg(colors.muted)
                        .child(
                            div()
                                .h_full()
                                .w(relative(value as f32 / badge.target.max(1) as f32))
                                .bg(colors.foreground.opacity(0.8)),
                        ),
                ),
        };
        let size = window.viewport_size();
        Some(
            gpui::deferred(
                gpui::anchored()
                    .position(gpui::point(px(0.), px(0.)))
                    .child(
                        div()
                            .id("badge-dialog-backdrop")
                            .w(size.width)
                            .h(size.height)
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(gpui::black().opacity(0.8))
                            .occlude()
                            .track_focus(&self.dialog_focus)
                            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                                if event.keystroke.key == "escape" {
                                    this.selected_badge = None;
                                    cx.stop_propagation();
                                    cx.notify();
                                }
                            }))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.selected_badge = None;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .id("badge-dialog")
                                    .relative()
                                    .w(px(384.))
                                    .p_6()
                                    .flex()
                                    .flex_col()
                                    .gap_4()
                                    .border_1()
                                    .border_color(colors.border)
                                    .bg(colors.background)
                                    .text_color(colors.foreground)
                                    .shadow_lg()
                                    .on_click(|_, _, cx| cx.stop_propagation())
                                    .child(
                                        div()
                                            .id("badge-dialog-close")
                                            .absolute()
                                            .top_4()
                                            .right_4()
                                            .opacity(0.7)
                                            .hover(|style| style.opacity(1.))
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.selected_badge = None;
                                                cx.notify();
                                            }))
                                            .child(controls::icon(
                                                "product/XIcon.svg",
                                                16.,
                                                colors.foreground,
                                            )),
                                    )
                                    .child(div().py_2().flex().justify_center().child(
                                        badges::emblem(id, collected.is_some(), true, &colors),
                                    ))
                                    .child(
                                        div()
                                            .text_center()
                                            .text_lg()
                                            .line_height(px(18.))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .child(badge.name),
                                    )
                                    .child(
                                        div()
                                            .text_center()
                                            .text_sm()
                                            .line_height(px(20.))
                                            .text_color(colors.muted_foreground)
                                            .child(badge.description),
                                    )
                                    .child(status),
                            ),
                    ),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }
}

impl Render for InsightsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let page = div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_8()
            .child(controls::page_title("Your insights"));
        let records = match &self.records {
            None => {
                return page.child(
                    div()
                        .text_sm()
                        .text_color(colors.muted_foreground)
                        .child("Loading your insights…"),
                );
            }
            Some(Err(_)) => {
                return page.child(
                    div()
                        .text_sm()
                        .text_color(colors.muted_foreground)
                        .child("Couldn't load your insights. Reopen this page to try again."),
                );
            }
            Some(Ok(records)) => records,
        };
        let stats = summarize(records, Utc::now(), self.zone, self.monday, self.range);
        let small = |text: String| {
            div()
                .text_xs()
                .line_height(px(16.))
                .text_color(colors.muted_foreground)
                .child(text)
        };
        let metrics = [
            ("Conversations", stats.conversations.to_string()),
            ("Hours transcribed", number(stats.hours)),
            ("Active days", stats.active_days.to_string()),
        ];
        page.child(
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
                        .child(heading("Overview"))
                        .child(self.range_filter(&colors, cx)),
                )
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .children(metrics.into_iter().map(|(label, value)| {
                            panel(&colors)
                                .flex_1()
                                .p_4()
                                .child(small(label.into()))
                                .child(
                                    div()
                                        .mt_2()
                                        .text_3xl()
                                        .line_height(px(36.))
                                        .font_weight(crate::ui::theme::MEDIUM)
                                        .child(value),
                                )
                        })),
                ),
        )
        .child(patterns(&stats, &colors))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_4()
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_baseline()
                        .justify_between()
                        .gap_2()
                        .child(heading("Activity over the past year"))
                        .child(small(format!("Weekly streak: {}", stats.streak))),
                )
                .child(heatmap(&stats, self.monday, &colors))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .text_xs()
                        .text_color(colors.muted_foreground)
                        .child("Every conversation adds to your story.")
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child("Less")
                                .children([0., 0.2, 0.4, 0.6, 0.8].into_iter().map(|alpha| {
                                    div().size(px(10.)).bg(if alpha == 0. {
                                        colors.muted
                                    } else {
                                        colors.foreground.opacity(alpha)
                                    })
                                }))
                                .child("More"),
                        ),
                ),
        )
        .child(self.badge_collection(&colors, cx))
        .child(small(
            "Includes imported transcripts. Deleted conversations are excluded.".into(),
        ))
        .children(self.badge_dialog(window, cx))
    }
}

impl Drop for InsightsView {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(day: &str, hour: u32) -> i64 {
        let date: NaiveDate = day.parse().unwrap();
        Utc.from_utc_datetime(&date.and_hms_opt(hour, 0, 0).unwrap())
            .timestamp_millis()
    }

    #[test]
    fn activity_counts_sessions_once_and_merges_overlapping_transcripts() {
        let now = Utc.timestamp_millis_opt(at("2026-11-17", 12)).unwrap();
        let record = |id: &str, day: &str, minutes: i64| Record {
            session_id: id.into(),
            started_at: at(day, 9),
            duration_ms: minutes * 60_000,
            is_demo: false,
        };
        let records = [
            // Two transcripts of one meeting overlap; its time counts once.
            record("a", "2026-11-16", 30),
            record("a", "2026-11-16", 20),
            record("b", "2026-11-09", 60),
            record("c", "2026-10-01", 0),
        ];
        let all = summarize(&records, now, Tz::UTC, false, Range::All);
        assert_eq!(all.conversations, 3);
        assert_eq!(all.hours, 1.5);
        assert_eq!(all.active_days, 3);
        assert_eq!(all.streak, 2);
        assert_eq!(all.median_minutes, Some(45.));
        assert_eq!(all.weekdays[1], (1, 2));
        assert_eq!(all.days.last(), Some(&("2026-11-17".parse().unwrap(), 0)));
        let week = summarize(&records, now, Tz::UTC, true, Range::Days7);
        assert_eq!((week.conversations, week.active_days), (1, 1));
        assert_eq!(week.weekdays[0], (1, 1));
    }
}
