//! Shipping's "Meeting info" panel (`outer-header/metadata/**`): the note date editor, or the
//! calendar event's details with its Join link, and the participants with contact search.

use std::sync::Arc;

use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use desktop_runtime::{CancellationToken, Result, RuntimeHandle, SessionId};
use gpui::{
    Context, Entity, EventEmitter, Hsla, KeyDownEvent, Render, SharedString, Subscription, Task,
    Window, div, prelude::*, px, rgb,
};
use serde_json::{Value, json};
use sqlx::SqliteConnection;

use super::super::mutations::failure;
use crate::ui::a11y::Accessible;
use crate::{
    application::toasts,
    meeting::widgets::spinner,
    ui::{
        input::{InputEvent, TextInput},
        theme::{MEDIUM, Theme, theme},
    },
};

/// The panel content's width: shipping's `w-72` submenu less its chrome, borders and `p-4`.
const CONTENT_WIDTH: f32 = 248.;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Clock {
    pub use_24_hour_time: bool,
    pub timezone: Option<Tz>,
}

impl Clock {
    /// date-fns `MMM d, yyyy` plus shipping's `useTimeFormat`.
    fn format(&self, at: DateTime<Utc>, date: bool) -> String {
        let time = if self.use_24_hour_time {
            "%H:%M"
        } else {
            "%-I:%M %p"
        };
        let pattern = if date {
            format!("%b %-d, %Y {time}")
        } else {
            time.to_owned()
        };
        match self.timezone {
            Some(zone) => at.with_timezone(&zone).format(&pattern).to_string(),
            None => at.with_timezone(&Local).format(&pattern).to_string(),
        }
    }

    fn day(&self, at: DateTime<Utc>) -> chrono::NaiveDate {
        match self.timezone {
            Some(zone) => at.with_timezone(&zone).date_naive(),
            None => at.with_timezone(&Local).date_naive(),
        }
    }
}

fn parse_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|date| date.with_timezone(&Utc))
        .ok()
}

/// Shipping's `formatEventDateTime`: the end drops its date on the same day.
fn event_time(start: &str, end: &str, clock: Clock) -> String {
    let Some(start) = parse_time(start) else {
        return String::new();
    };
    let first = clock.format(start, true);
    match parse_time(end) {
        Some(end) => {
            let same_day = clock.day(start) == clock.day(end);
            format!("{first} to {}", clock.format(end, !same_day))
        }
        None => first,
    }
}

/// Reads the `datetime-local` value the editor shows (`yyyy-MM-ddTHH:mm`, a space also works)
/// as a local time.
fn parse_local(value: &str) -> Option<DateTime<Utc>> {
    let value = value.trim();
    let naive = ["%Y-%m-%dT%H:%M", "%Y-%m-%d %H:%M", "%Y-%m-%dT%H:%M:%S"]
        .iter()
        .find_map(|format| NaiveDateTime::parse_from_str(value, format).ok())?;
    Local
        .from_local_datetime(&naive)
        .earliest()
        .map(|local| local.with_timezone(&Utc))
}

/// The meeting link's host without `www.`.
fn link_domain(link: &str) -> Option<String> {
    url::Url::parse(link)
        .ok()?
        .host_str()
        .map(|host| host.replacen("www.", "", 1))
}

/// The description split into text and URLs, trailing punctuation kept out of each link.
fn description_parts(description: &str) -> Vec<(String, bool)> {
    let pattern = regex::Regex::new(r#"(?i)https?://[^\s<>"'`]+"#).expect("constant expression");
    let mut parts = Vec::new();
    let mut last = 0;
    for found in pattern.find_iter(description) {
        if found.start() > last {
            parts.push((description[last..found.start()].to_owned(), false));
        }
        let mut url = found.as_str();
        while !url.is_empty() && url::Url::parse(url).is_err() {
            if !url.ends_with(['.', ',', '!', '?', ';', ':', ')', ']', '}']) {
                break;
            }
            url = &url[..url.len() - 1];
        }
        if url.is_empty() || url::Url::parse(url).is_err() {
            parts.push((found.as_str().to_owned(), false));
        } else {
            parts.push((url.to_owned(), true));
            if url.len() < found.as_str().len() {
                parts.push((found.as_str()[url.len()..].to_owned(), false));
            }
        }
        last = found.end();
    }
    if last < description.len() {
        parts.push((description[last..].to_owned(), false));
    }
    parts
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Event {
    title: String,
    started_at: String,
    ended_at: String,
    location: String,
    meeting_link: String,
    description: String,
}

impl Event {
    fn parse(raw: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(raw).ok()?;
        let text = |key: &str| value[key].as_str().unwrap_or_default().to_owned();
        value.is_object().then(|| Self {
            title: text("title"),
            started_at: text("started_at"),
            ended_at: text("ended_at"),
            location: text("location"),
            meeting_link: text("meeting_link"),
            description: text("description"),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Participant {
    id: Arc<str>,
    human: Arc<str>,
    name: Arc<str>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Candidate {
    id: Arc<str>,
    name: Arc<str>,
    email: Arc<str>,
    phone: Arc<str>,
    job_title: Arc<str>,
}

/// Shipping's dropdown: contacts not yet added whose name, email or phone contains the input,
/// led by an `Add "…"` row unless a contact already has exactly that name.
pub(super) fn options(input: &str, humans: &[Candidate], added: &[Arc<str>]) -> Vec<Candidate> {
    let search = input.to_lowercase();
    let mut matches: Vec<Candidate> = humans
        .iter()
        .filter(|human| !added.contains(&human.id))
        .filter(|human| {
            [&human.name, &human.email, &human.phone]
                .iter()
                .any(|field| field.to_lowercase().contains(&search))
        })
        .cloned()
        .collect();
    let trimmed = input.trim();
    if !trimmed.is_empty()
        && !matches
            .iter()
            .any(|human| human.name.to_lowercase() == search)
    {
        matches.insert(
            0,
            Candidate {
                id: "".into(),
                name: trimmed.into(),
                email: "".into(),
                phone: "".into(),
                job_title: "".into(),
            },
        );
    }
    matches
}

pub enum MeetingInfoEvent {
    OpenContact(Arc<str>),
    /// The note's date was saved, which also moved its revision.
    DateChanged,
}

struct Pending {
    key: usize,
    human: Option<Arc<str>>,
    name: Arc<str>,
}

pub struct MeetingInfo {
    runtime: RuntimeHandle,
    session: SessionId,
    clock: Clock,
    created_at: Arc<str>,
    /// Shown between saving a date and the watch catching up, so the old date never flashes.
    pending_created_at: Option<Arc<str>>,
    event: Option<Event>,
    participants: Vec<Participant>,
    humans: Vec<Candidate>,
    input: Entity<TextInput>,
    dropdown: bool,
    selected: usize,
    pending: Vec<Pending>,
    next_pending: usize,
    date: Option<(Entity<TextInput>, Subscription)>,
    date_error: Option<&'static str>,
    message: Option<String>,
    /// The participant whose contact is being enhanced from the event.
    enhancing: Option<Arc<str>>,
    cancel: CancellationToken,
    _watch: Option<Task<()>>,
    _input: Subscription,
}

impl EventEmitter<MeetingInfoEvent> for MeetingInfo {}

impl MeetingInfo {
    pub fn new(
        runtime: RuntimeHandle,
        session: SessionId,
        clock: Clock,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            TextInput::new("Add participants", cx)
                .inline()
                .label("Participants")
        });
        let subscription = cx.subscribe(&input, |this, _, event, cx| match event {
            InputEvent::Changed => {
                this.dropdown = true;
                this.selected = 0;
                cx.notify();
            }
            InputEvent::Submitted => this.add_selected(cx),
            InputEvent::Rejected => {}
        });
        let mut this = Self {
            runtime,
            session,
            clock,
            created_at: "".into(),
            pending_created_at: None,
            event: None,
            participants: Vec::new(),
            humans: Vec::new(),
            input,
            dropdown: false,
            selected: 0,
            pending: Vec::new(),
            next_pending: 0,
            date: None,
            date_error: None,
            message: None,
            enhancing: None,
            cancel: CancellationToken::new(),
            _watch: None,
            _input: subscription,
        };
        this.watch(cx);
        this.load_humans(cx);
        this
    }

    pub fn set_clock(&mut self, clock: Clock, cx: &mut Context<Self>) {
        self.clock = clock;
        cx.notify();
    }

    fn watch(&mut self, cx: &mut Context<Self>) {
        let reply = self.runtime.watch_query(
            "SELECT s.created_at, s.event_json,
               (SELECT json_group_array(json_object('id', p.id, 'human_id', p.human_id,
                   'source', p.source, 'name', p.name))
                FROM (SELECT participant.id, participant.human_id, participant.source,
                        COALESCE(NULLIF(human.name, ''), participant.display_name) AS name,
                        COALESCE(NULLIF(human.email, ''), participant.email) AS email
                      FROM session_participants AS participant
                      LEFT JOIN humans AS human
                        ON human.id = participant.human_id AND human.deleted_at IS NULL
                      WHERE participant.session_id = s.id AND participant.deleted_at IS NULL
                      ORDER BY name, email, participant.id) p) AS participants
             FROM sessions s WHERE s.id = ? AND s.deleted_at IS NULL"
                .into(),
            vec![json!(self.session)],
        );
        self._watch = Some(cx.spawn(async move |this, cx| {
            let Ok(mut watch) = async { reply?.receive().await }.await else {
                return;
            };
            loop {
                let rows = watch.snapshots.borrow_and_update().rows.clone();
                let terminal = watch
                    .terminal_error()
                    .or_else(|| watch.errors.try_recv().ok());
                if this
                    .update(cx, |this, cx| {
                        if let Some(error) = &terminal {
                            this.message = Some(format!("Meeting info unavailable: {error}"));
                        } else if let Some(row) = rows.first() {
                            this.apply(row);
                        }
                        cx.notify();
                    })
                    .is_err()
                    || terminal.is_some()
                    || watch.snapshots.changed().await.is_err()
                {
                    break;
                }
            }
            let _ = watch.unsubscribe().await;
        }));
    }

    fn apply(&mut self, row: &Value) {
        let created = row["created_at"].as_str().unwrap_or_default();
        if self.pending_created_at.as_deref() == Some(created) {
            self.pending_created_at = None;
        }
        self.created_at = created.into();
        self.event = row["event_json"].as_str().and_then(Event::parse);
        let rows: Vec<Value> = row["participants"]
            .as_str()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();
        self.participants = rows
            .iter()
            .filter(|row| row["source"] != "excluded")
            .filter_map(|row| {
                Some(Participant {
                    id: row["id"].as_str()?.into(),
                    human: row["human_id"].as_str().unwrap_or_default().into(),
                    name: row["name"].as_str().unwrap_or_default().trim().into(),
                })
            })
            .collect();
        let added: Vec<Arc<str>> = self.participants.iter().map(|p| p.human.clone()).collect();
        self.pending.retain(|pending| {
            pending
                .human
                .as_ref()
                .is_none_or(|human| !added.contains(human))
        });
    }

    fn load_humans(&mut self, cx: &mut Context<Self>) {
        let reply = self
            .runtime
            .read(self.cancel.clone(), |services| async move {
                services
                .executor
                .execute(
                    "SELECT id, name, email, phone, job_title FROM humans WHERE deleted_at IS NULL
                     ORDER BY name, email, id LIMIT 5000"
                        .into(),
                    vec![],
                )
                .await
                .map_err(failure)
            });
        cx.spawn(async move |this, cx| {
            if let Ok(rows) = async { reply?.receive().await }.await {
                let _ = this.update(cx, |this, cx| {
                    let text = |row: &Value, key: &str| -> Arc<str> {
                        row[key].as_str().unwrap_or_default().into()
                    };
                    this.humans = rows
                        .iter()
                        .map(|row| Candidate {
                            id: text(row, "id"),
                            name: text(row, "name"),
                            email: text(row, "email"),
                            phone: text(row, "phone"),
                            job_title: text(row, "job_title"),
                        })
                        .collect();
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn options(&self, cx: &gpui::App) -> Vec<Candidate> {
        let added: Vec<Arc<str>> = self.participants.iter().map(|p| p.human.clone()).collect();
        options(&self.input.read(cx).buffer.text, &self.humans, &added)
    }

    fn add_selected(&mut self, cx: &mut Context<Self>) {
        if self.input.read(cx).buffer.text.trim().is_empty() {
            return;
        }
        let options = self.options(cx);
        if let Some(option) = options.get(self.selected.min(options.len().saturating_sub(1))) {
            self.add(option.clone(), cx);
        }
    }

    fn reset_input(&mut self, cx: &mut Context<Self>) {
        self.input
            .update(cx, |input, cx| input.set_text(String::new(), cx));
        self.dropdown = false;
        self.selected = 0;
        cx.notify();
    }

    fn add(&mut self, option: Candidate, cx: &mut Context<Self>) {
        let key = self.next_pending;
        self.next_pending += 1;
        let new = option.id.is_empty();
        self.pending.push(Pending {
            key,
            human: (!new).then(|| option.id.clone()),
            name: option.name.clone(),
        });
        self.reset_input(cx);
        let reply = add_participant(
            &self.runtime,
            self.session.clone(),
            (!new).then(|| option.id.clone()),
            option.name.clone(),
        );
        cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(human) => {
                        if let Some(pending) = this.pending.iter_mut().find(|p| p.key == key) {
                            pending.human = Some(human);
                        }
                        if new {
                            this.load_humans(cx);
                        }
                    }
                    Err(error) => {
                        this.message = Some(format!("Could not add participant: {error}"));
                    }
                }
                // Shipping keeps the pending chip until the watch lists the person, or drops it
                // after a failure.
                let added: Vec<Arc<str>> =
                    this.participants.iter().map(|p| p.human.clone()).collect();
                this.pending.retain(|pending| {
                    pending.key != key
                        || pending
                            .human
                            .as_ref()
                            .is_some_and(|human| !added.contains(human))
                });
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Shipping's `useEventContactEnhancement` with its result toasts.
    fn enhance(&mut self, human: Arc<str>, cx: &mut Context<Self>) {
        if self.enhancing.is_some() {
            return;
        }
        self.enhancing = Some(human.clone());
        cx.notify();
        let runtime = self.runtime.clone();
        let session = self.session.clone();
        cx.spawn(async move |this, cx| {
            let result = super::event_contacts::enhance(&runtime, session, human.to_string()).await;
            let _ = this.update(cx, |this, cx| {
                this.enhancing = None;
                let toast = match result {
                    Ok(outcome) => outcome
                        .toast()
                        .id(format!("event-contact-enhancement-{human}")),
                    Err(_) => toasts::Toast::error("Could not enhance contact")
                        .id("event-contact-enhancement"),
                };
                toasts::show(toast, cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn remove(&mut self, participant: Participant, cx: &mut Context<Self>) {
        self.participants.retain(|p| p.id != participant.id);
        let reply = remove_participant(
            &self.runtime,
            self.session.clone(),
            participant.id,
            participant.human,
        );
        cx.spawn(async move |this, cx| {
            if let Err(error) = async { reply?.receive().await }.await {
                let _ = this.update(cx, |this, cx| {
                    this.message = Some(format!("Could not remove participant: {error}"));
                    cx.notify();
                });
            }
        })
        .detach();
        cx.notify();
    }

    fn edit_date(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = parse_time(self.shown_created_at())
            .map(|at| {
                at.with_timezone(&Local)
                    .format("%Y-%m-%d %H:%M")
                    .to_string()
            })
            .unwrap_or_default();
        let input = cx.new(|cx| {
            let mut input = TextInput::new("", cx).inline().label("Date");
            input.set_text(value, cx);
            input
        });
        let subscription = cx.subscribe(&input, |this, _, event, cx| match event {
            InputEvent::Submitted => this.save_date(cx),
            _ => {
                this.validate_date(cx);
                cx.notify();
            }
        });
        cx.focus_view(&input, window);
        self.date = Some((input, subscription));
        self.date_error = None;
        cx.notify();
    }

    fn validate_date(&mut self, cx: &gpui::App) -> Option<DateTime<Utc>> {
        let text = self
            .date
            .as_ref()
            .map(|(input, _)| input.read(cx).buffer.text.clone())
            .unwrap_or_default();
        let parsed = parse_local(&text);
        self.date_error = if text.trim().is_empty() {
            Some("Date and time are required")
        } else if parsed.is_none() {
            Some("Enter a valid date and time")
        } else {
            None
        };
        parsed
    }

    fn save_date(&mut self, cx: &mut Context<Self>) {
        let Some(at) = self.validate_date(cx) else {
            cx.notify();
            return;
        };
        let value: Arc<str> = at
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
            .into();
        self.date = None;
        self.pending_created_at = Some(value.clone());
        let session = self.session.clone();
        let reply = self.runtime.submit(move |services| async move {
            let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            services
                .executor
                .execute(
                    "UPDATE sessions SET created_at = ?, updated_at = ? WHERE id = ? AND deleted_at IS NULL"
                        .into(),
                    vec![json!(value), json!(now), json!(session)],
                )
                .await
                .map_err(failure)?;
            Ok(())
        });
        cx.spawn(async move |this, cx| {
            let saved = async { reply?.receive().await }.await;
            let _ = this.update(cx, |this, cx| match saved {
                Ok(()) => cx.emit(MeetingInfoEvent::DateChanged),
                Err(_) => {
                    this.pending_created_at = None;
                    this.message = Some("Could not update the note date.".into());
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    fn shown_created_at(&self) -> &str {
        self.pending_created_at
            .as_deref()
            .unwrap_or(&self.created_at)
    }

    fn input_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let empty = self.input.read(cx).buffer.text.is_empty();
        let count = self.options(cx).len();
        match event.keystroke.key.as_str() {
            "down" => self.selected = (self.selected + 1).min(count.saturating_sub(1)),
            "up" => self.selected = self.selected.saturating_sub(1),
            "escape" if !empty || self.dropdown => self.reset_input(cx),
            "tab" if !empty => self.add_selected(cx),
            "backspace" if empty => {
                if let Some(last) = self.participants.last().cloned() {
                    self.remove(last, cx);
                }
            }
            _ => return,
        }
        let _ = window;
        cx.stop_propagation();
        cx.notify();
    }

    fn date_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "escape" && self.date.is_some() {
            self.date = None;
            cx.stop_propagation();
            cx.notify();
        }
    }

    fn date_editor(&self, colors: Theme, cx: &mut Context<Self>) -> gpui::AnyElement {
        let muted = colors.muted_foreground;
        if let Some((input, _)) = &self.date {
            let red: Hsla = rgb(0xdc2626).into();
            let dark = colors.background.l < 0.5;
            return div()
                .flex()
                .flex_col()
                .gap_2()
                .capture_key_down(cx.listener(Self::date_key))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .h(px(28.))
                                .w_full()
                                .flex()
                                .items_center()
                                .text_sm()
                                .line_height(px(20.))
                                .child(input.clone()),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_shrink_0()
                                .items_center()
                                .justify_end()
                                .child(
                                    round_button("cancel-date-edit", "editor/XIcon.svg", colors)
                                        .a11y_button("Cancel date edit")
                                        .hover(move |style| {
                                            style
                                                .bg(rgb(if dark { 0x450a0a } else { 0xfef2f2 }))
                                                .text_color(rgb(if dark {
                                                    0xfca5a5
                                                } else {
                                                    0xdc2626
                                                }))
                                        })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.date = None;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    round_button("save-date", "editor/CheckIcon.svg", colors)
                                        .a11y_button("Save date")
                                        .when(self.date_error.is_some(), |button| {
                                            button.opacity(0.5)
                                        })
                                        .hover(move |style| {
                                            style
                                                .bg(rgb(if dark { 0x052e16 } else { 0xf0fdf4 }))
                                                .text_color(rgb(if dark {
                                                    0x86efac
                                                } else {
                                                    0x16a34a
                                                }))
                                        })
                                        .on_click(cx.listener(|this, _, _, cx| this.save_date(cx))),
                                ),
                        ),
                )
                .when_some(self.date_error, |view, error| {
                    view.child(
                        div()
                            .id("date-error")
                            .a11y_status(error, true)
                            .text_xs()
                            .text_color(red)
                            .child(error),
                    )
                })
                .into_any_element();
        }
        let date = parse_time(self.shown_created_at())
            .map(|at| self.clock.format(at, true))
            .unwrap_or_else(|| "Unknown date".into());
        div()
            .h(px(28.))
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                div()
                    .id("meeting-date")
                    .a11y_text(date.clone())
                    .flex_1()
                    .min_w_0()
                    .text_sm()
                    .line_height(px(20.))
                    .text_color(muted)
                    .child(date),
            )
            .child(
                round_button("edit-date", "workspace/PencilIcon.svg", colors)
                    .a11y_button("Edit date")
                    .on_click(cx.listener(|this, _, window, cx| this.edit_date(window, cx))),
            )
            .into_any_element()
    }

    fn event_display(
        &self,
        event: &Event,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let muted = colors.muted_foreground;
        let location = (!event.location.is_empty() && url::Url::parse(&event.location).is_err())
            .then(|| event.location.clone());
        let link = (!event.meeting_link.is_empty()).then(|| event.meeting_link.clone());
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .min_w_0()
                    .text_base()
                    .font_weight(MEDIUM)
                    .text_color(colors.foreground)
                    .child(if event.title.is_empty() {
                        "Untitled Event".to_owned()
                    } else {
                        event.title.clone()
                    }),
            )
            .child(separator(colors))
            .when_some(location, |view, location| {
                view.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_sm()
                        .line_height(px(20.))
                        .text_color(muted)
                        .child(icon("workspace/MapPinIcon.svg", 16., muted))
                        .child(div().min_w_0().child(location)),
                )
            })
            .when_some(link, |view, link| {
                let domain = link_domain(&link).unwrap_or_else(|| "Meeting link".into());
                view.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .min_w_0()
                                .items_center()
                                .gap_2()
                                .text_sm()
                                .line_height(px(20.))
                                .text_color(muted)
                                .child(icon("workspace/Video01Icon.svg", 16., muted))
                                .child(div().min_w_0().truncate().child(domain)),
                        )
                        .child(
                            div()
                                .id("join-meeting")
                                .a11y_link("Join")
                                .h(px(28.))
                                .px_2()
                                .flex()
                                .flex_shrink_0()
                                .items_center()
                                .justify_center()
                                .bg(colors.primary)
                                .text_color(colors.primary_foreground)
                                .text_xs()
                                .font_weight(MEDIUM)
                                .shadow_xs()
                                .cursor_pointer()
                                .hover(move |style| style.bg(colors.primary.opacity(0.9)))
                                .child("Join")
                                .on_click(move |_, _, cx| cx.open_url(&link)),
                        ),
                )
            })
            .when(!event.started_at.is_empty(), |view| {
                view.child(
                    div()
                        .text_sm()
                        .line_height(px(20.))
                        .text_color(muted)
                        .child(event_time(&event.started_at, &event.ended_at, self.clock)),
                )
            })
            .child(self.participants_view(colors, cx))
            .when(!event.description.is_empty(), |view| {
                view.child(separator(colors))
                    .child(description(&event.description, colors))
            })
            .into_any_element()
    }

    fn participants_view(&self, colors: Theme, cx: &mut Context<Self>) -> gpui::AnyElement {
        let options = self.options(cx);
        let typed = !self.input.read(cx).buffer.text.trim().is_empty();
        let selected = self.selected.min(options.len().saturating_sub(1));
        let chip_bg = colors.foreground.opacity(0.1);
        // Shipping's `showEnhancementButtons`: the event has a title or description.
        let enhanceable = self.event.as_ref().is_some_and(|event| {
            !event.title.trim().is_empty() || !event.description.trim().is_empty()
        });
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(separator(colors))
            .child(
                div()
                    .relative()
                    .capture_key_down(cx.listener(Self::input_key))
                    .child(
                        div()
                            .id("participants-field")
                            .min_h(px(38.))
                            .w_full()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_2()
                            .cursor_text()
                            .on_click(cx.listener(|this, _, window, cx| {
                                cx.focus_view(&this.input, window);
                                this.dropdown = true;
                            }))
                            .children(self.participants.iter().filter(|p| !p.name.is_empty()).map(
                                |participant| {
                                    let open = participant.human.clone();
                                    let removed = participant.clone();
                                    let enhancing =
                                        self.enhancing.as_ref() == Some(&participant.human);
                                    div()
                                        .id(SharedString::from(format!(
                                            "participant:{}",
                                            participant.id
                                        )))
                                        .a11y_button(participant.name.clone())
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .px_2()
                                        .py(px(2.))
                                        .bg(chip_bg)
                                        .hover(move |style| {
                                            style.bg(colors.foreground.opacity(0.15))
                                        })
                                        .text_xs()
                                        .line_height(px(16.))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(colors.secondary_foreground)
                                        .cursor_pointer()
                                        .when(enhancing, |chip| {
                                            chip.shadow(vec![gpui::BoxShadow {
                                                color: colors.ring.opacity(0.2),
                                                offset: gpui::point(px(0.), px(0.)),
                                                blur_radius: px(0.),
                                                spread_radius: px(1.),
                                                inset: false,
                                            }])
                                        })
                                        .on_click(cx.listener(move |_, _, _, cx| {
                                            if !open.is_empty() {
                                                cx.emit(MeetingInfoEvent::OpenContact(open.clone()))
                                            }
                                        }))
                                        .child(SharedString::from(participant.name.clone()))
                                        .when(
                                            enhanceable && !participant.human.is_empty(),
                                            |chip| {
                                                chip.child(self.enhance_button(
                                                    participant,
                                                    enhancing,
                                                    colors,
                                                    cx,
                                                ))
                                            },
                                        )
                                        .child(
                                            div()
                                                .id(SharedString::from(format!(
                                                    "remove-participant:{}",
                                                    participant.id
                                                )))
                                                .a11y_button(format!("Remove {}", participant.name))
                                                .ml(px(2.))
                                                .size(px(12.))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    cx.stop_propagation();
                                                    this.remove(removed.clone(), cx);
                                                }))
                                                .child(icon(
                                                    "editor/XIcon.svg",
                                                    10.,
                                                    colors.secondary_foreground,
                                                )),
                                        )
                                },
                            ))
                            .children(
                                self.pending
                                    .iter()
                                    .filter(|pending| {
                                        pending.human.as_ref().is_none_or(|human| {
                                            !self.participants.iter().any(|p| &p.human == human)
                                        })
                                    })
                                    .map(|pending| {
                                        div()
                                            .id(("pending-participant", pending.key))
                                            .a11y_text(format!("Adding {}", pending.name))
                                            .flex()
                                            .items_center()
                                            .gap_1()
                                            .px_2()
                                            .py(px(2.))
                                            .bg(chip_bg)
                                            .opacity(0.6)
                                            .text_xs()
                                            .line_height(px(16.))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(colors.secondary_foreground)
                                            .child(SharedString::from(pending.name.clone()))
                                            .child(spinner(
                                                SharedString::from(format!(
                                                    "pending-spinner-{}",
                                                    pending.key
                                                )),
                                                10.,
                                                colors.secondary_foreground,
                                            ))
                                    }),
                            )
                            .child(
                                div()
                                    .min_w(px(120.))
                                    .flex_1()
                                    .text_sm()
                                    .line_height(px(20.))
                                    .child(self.input.clone()),
                            ),
                    )
                    .when(self.dropdown && typed && !options.is_empty(), |view| {
                        view.child(
                            div().absolute().top_full().left_0().child(
                                gpui::deferred(
                                    gpui::anchored()
                                        .snap_to_window()
                                        .child(self.dropdown_view(&options, selected, colors, cx)),
                                )
                                .with_priority(3),
                            ),
                        )
                    }),
            )
            .into_any_element()
    }

    /// Shipping's `EnhanceContactButton`: a sparkle, or a spinner while it runs.
    fn enhance_button(
        &self,
        participant: &Participant,
        enhancing: bool,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let human = participant.human.clone();
        let disabled = self.enhancing.is_some();
        div()
            .id(SharedString::from(format!(
                "enhance-participant:{}",
                participant.id
            )))
            .a11y_button(format!("Enhance contact {}", participant.name))
            .a11y_disabled(disabled && !enhancing)
            .group("enhance-contact")
            .ml(px(2.))
            .size(px(14.))
            .flex()
            .items_center()
            .justify_center()
            .when(disabled && !enhancing, |button| button.opacity(0.5))
            .when(!disabled, |button| {
                button
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.enhance(human.clone(), cx);
                    }))
            })
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(if enhancing {
                spinner(
                    SharedString::from(format!("enhancing-{}", participant.id)),
                    10.,
                    colors.muted_foreground,
                )
                .into_any_element()
            } else {
                gpui::svg()
                    .path("editor/SparkleIcon.svg")
                    .size(px(10.))
                    .text_color(colors.muted_foreground)
                    .when(!disabled, |icon| {
                        icon.group_hover("enhance-contact", move |style| {
                            style.text_color(colors.foreground)
                        })
                    })
                    .into_any_element()
            })
    }

    fn dropdown_view(
        &self,
        options: &[Candidate],
        selected: usize,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("participant-dropdown")
            .a11y_group(gpui::Role::ListBox, "Contacts")
            .mt_1()
            .w(px(CONTENT_WIDTH))
            .occlude()
            .overflow_hidden()
            .border_1()
            .border_color(colors.border)
            .bg(colors.popover)
            .shadow_md()
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .id("participant-options")
                    .max_h(px(200.))
                    .overflow_y_scroll()
                    .py_1()
                    .children(options.iter().enumerate().map(|(index, option)| {
                        let chosen = option.clone();
                        let active = index == selected;
                        div()
                            .id(("participant-option", index))
                            .a11y_option(
                                if option.id.is_empty() {
                                    format!("Add \"{}\"", option.name)
                                } else if option.job_title.is_empty() {
                                    option.name.to_string()
                                } else {
                                    format!("{}, {}", option.name, option.job_title)
                                },
                                active,
                            )
                            .w_full()
                            .px_3()
                            .py(px(6.))
                            .text_sm()
                            .line_height(px(20.))
                            .line_height(px(20.))
                            .text_color(colors.popover_foreground)
                            .cursor_pointer()
                            .when_else(
                                active,
                                |row| row.bg(colors.muted),
                                |row| row.hover(move |style| style.bg(colors.accent)),
                            )
                            .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                                if *hovering {
                                    this.selected = index;
                                    cx.notify();
                                }
                            }))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.add(chosen.clone(), cx);
                                cx.focus_view(&this.input, window);
                            }))
                            .child(
                                div()
                                    .flex()
                                    .w_full()
                                    .items_center()
                                    .justify_between()
                                    .child(if option.id.is_empty() {
                                        div()
                                            .flex()
                                            .child("Add \"")
                                            .child(
                                                div()
                                                    .font_weight(MEDIUM)
                                                    .child(SharedString::from(option.name.clone())),
                                            )
                                            .child("\"")
                                    } else {
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .font_weight(MEDIUM)
                                                    .child(SharedString::from(option.name.clone())),
                                            )
                                            .when(!option.job_title.is_empty(), |row| {
                                                row.child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(colors.muted_foreground)
                                                        .child(SharedString::from(
                                                            option.job_title.clone(),
                                                        )),
                                                )
                                            })
                                    })
                                    .when(active, |row| {
                                        row.child(icon(
                                            "workspace/ArrowTurnBackwardIcon.svg",
                                            12.,
                                            colors.muted_foreground,
                                        ))
                                    }),
                            )
                    })),
            )
    }
}

impl Drop for MeetingInfo {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl Render for MeetingInfo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let event = self.event.clone();
        div()
            .id("meeting-info")
            .max_h(window.viewport_size().height * 0.8)
            .overflow_y_scroll()
            .border_1()
            .border_color(colors.app_floating_border)
            .bg(colors.app_floating_panel)
            .text_color(colors.popover_foreground)
            .child(
                div()
                    .flex()
                    .min_w_0()
                    .flex_col()
                    .gap_4()
                    .p_4()
                    .map(|view| match &event {
                        Some(event) => view.child(self.event_display(event, colors, cx)),
                        None => view
                            .child(self.date_editor(colors, cx))
                            .child(self.participants_view(colors, cx)),
                    })
                    .when_some(self.message.clone(), |view, message| {
                        view.child(
                            div()
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child(message),
                        )
                    }),
            )
    }
}

fn separator(colors: Theme) -> gpui::Div {
    div().h(px(1.)).bg(colors.accent)
}

fn round_button(id: &'static str, path: &'static str, colors: Theme) -> gpui::Stateful<gpui::Div> {
    let muted = colors.muted_foreground;
    div()
        .id(id)
        .group(id)
        .size(px(28.))
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .text_color(muted)
        .cursor_pointer()
        .hover(move |style| style.bg(colors.accent).text_color(colors.foreground))
        .child(
            icon(path, 16., muted)
                .group_hover(id, move |style| style.text_color(colors.foreground)),
        )
}

fn description(text: &str, colors: Theme) -> impl IntoElement {
    let parts = description_parts(text);
    let mut runs = Vec::new();
    let mut links = Vec::new();
    let mut flat = String::new();
    for (part, link) in parts {
        let start = flat.len();
        flat.push_str(&part);
        if link {
            links.push((start..flat.len(), part.clone()));
        }
        runs.push((start..flat.len(), link));
    }
    let highlights = links
        .iter()
        .map(|(range, _)| {
            (
                range.clone(),
                gpui::HighlightStyle {
                    underline: Some(gpui::UnderlineStyle {
                        thickness: px(1.),
                        color: Some(colors.muted_foreground),
                        wavy: false,
                    }),
                    ..Default::default()
                },
            )
        })
        .collect::<Vec<_>>();
    let ranges: Vec<_> = links.iter().map(|(range, _)| range.clone()).collect();
    let urls: Vec<String> = links.into_iter().map(|(_, url)| url).collect();
    div()
        .text_sm()
        .line_height(px(20.))
        .text_color(colors.muted_foreground)
        .child(
            gpui::InteractiveText::new(
                "meeting-description",
                gpui::StyledText::new(flat).with_highlights(highlights),
            )
            .on_click(ranges, move |index, _, cx| cx.open_url(&urls[index])),
        )
}

fn icon(path: &'static str, size: f32, color: Hsla) -> gpui::Svg {
    gpui::svg()
        .path(path)
        .size(px(size))
        .flex_shrink_0()
        .text_color(color)
}

/// Shipping's `addSessionParticipant`, creating the contact first for an `Add "…"` row.
fn add_participant(
    runtime: &RuntimeHandle,
    session: SessionId,
    human: Option<Arc<str>>,
    name: Arc<str>,
) -> Result<desktop_runtime::Reply<Arc<str>>> {
    runtime.submit(move |services| async move {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let mut transaction = services
            .db
            .pool()
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(failure)?;
        let result = async {
            let conn: &mut SqliteConnection = &mut transaction;
            let human = match human {
                Some(human) => human,
                None => {
                    let owner: Option<String> = sqlx::query_scalar(
                        "SELECT owner_user_id FROM sessions WHERE id = ? AND deleted_at IS NULL",
                    )
                    .bind(&session.0)
                    .fetch_optional(&mut *conn)
                    .await
                    .map_err(failure)?;
                    let owner = owner.ok_or_else(|| failure("This note is unavailable"))?;
                    let id = uuid::Uuid::new_v4().to_string();
                    anlg_db_app::create_human(conn, &id, &owner, &name, "", &now)
                        .await
                        .map_err(failure)?;
                    id.into()
                }
            };
            anlg_db_app::revive_excluded_session_participant(
                conn, &session.0, &human, "manual", &now,
            )
            .await
            .map_err(failure)?;
            let id = uuid::Uuid::new_v4().to_string();
            anlg_db_app::insert_manual_session_participant(
                conn, &id, &session.0, &human, "manual", &now,
            )
            .await
            .map_err(failure)?;
            Ok::<_, desktop_runtime::ServiceError>(human)
        }
        .await;
        match result {
            Ok(human) => {
                transaction.commit().await.map_err(failure)?;
                Ok(human)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    })
}

/// Shipping's chip removal: drops the person's speaker assignments in this note's
/// transcripts, then removes (or, for calendar attendees, excludes) the participant.
fn remove_participant(
    runtime: &RuntimeHandle,
    session: SessionId,
    mapping: Arc<str>,
    human: Arc<str>,
) -> Result<desktop_runtime::Reply<()>> {
    runtime.submit(move |services| async move {
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let mut transaction = services
            .db
            .pool()
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(failure)?;
        let conn: &mut SqliteConnection = &mut transaction;
        if !human.is_empty() {
            let transcripts: Vec<(String, String)> = sqlx::query_as(
                "SELECT id, speaker_hints_json FROM transcripts
                 WHERE session_id = ? AND deleted_at IS NULL AND instr(speaker_hints_json, ?) > 0",
            )
            .bind(&session.0)
            .bind(human.as_ref())
            .fetch_all(&mut *conn)
            .await
            .map_err(failure)?;
            for (id, hints) in transcripts {
                let Some(kept) = without_assignments(&hints, &human) else {
                    continue;
                };
                sqlx::query(
                    "UPDATE transcripts SET speaker_hints_json = ?, updated_at = ? WHERE id = ?",
                )
                .bind(kept)
                .bind(&now)
                .bind(id)
                .execute(&mut *conn)
                .await
                .map_err(failure)?;
            }
        }
        anlg_db_app::remove_session_participant_mapping(conn, &mapping, &now)
            .await
            .map_err(failure)?;
        transaction.commit().await.map_err(failure)
    })
}

/// The speaker hints without `human`'s assignments, or `None` when nothing changes.
fn without_assignments(hints: &str, human: &str) -> Option<String> {
    let hints: Vec<Value> = serde_json::from_str(hints).ok()?;
    let assigned = |hint: &Value| {
        if !matches!(
            hint["type"].as_str(),
            Some("automatic_speaker_assignment" | "user_speaker_assignment")
        ) {
            return false;
        }
        let value = match &hint["value"] {
            Value::String(encoded) => serde_json::from_str(encoded).unwrap_or(Value::Null),
            value => value.clone(),
        };
        value["human_id"] == human
    };
    let kept: Vec<&Value> = hints.iter().filter(|hint| !assigned(hint)).collect();
    (kept.len() != hints.len()).then(|| json!(kept).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn human(id: &str, name: &str, email: &str) -> Candidate {
        Candidate {
            id: id.into(),
            name: name.into(),
            email: email.into(),
            phone: "".into(),
            job_title: "".into(),
        }
    }

    #[test]
    fn participant_search_offers_new_contacts_first_and_skips_added_people() {
        let humans = [
            human("maya", "Maya Chen", "maya@northwind.example"),
            human("priya", "Priya Patel", "priya@northwind.example"),
            human("mark", "Mark", ""),
        ];
        let names = |input: &str, added: &[Arc<str>]| {
            options(input, &humans, added)
                .into_iter()
                .map(|option| (option.id.to_string(), option.name.to_string()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names("northwind", &["priya".into()]),
            [
                ("".into(), "northwind".into()),
                ("maya".into(), "Maya Chen".into())
            ]
        );
        // An exact name match needs no `Add "…"` row.
        assert_eq!(names("mark", &[]), [("mark".into(), "Mark".into())]);
    }

    #[test]
    fn event_details_follow_shipping_formats() {
        let clock = Clock {
            use_24_hour_time: false,
            timezone: Some(chrono_tz::UTC),
        };
        assert_eq!(
            event_time("2026-11-16T15:00:00Z", "2026-11-16T16:30:00Z", clock),
            "Nov 16, 2026 3:00 PM to 4:30 PM"
        );
        assert_eq!(
            event_time("2026-11-16T15:00:00Z", "2026-11-17T09:00:00Z", clock),
            "Nov 16, 2026 3:00 PM to Nov 17, 2026 9:00 AM"
        );
        assert_eq!(
            link_domain("https://www.zoom.us/j/1").as_deref(),
            Some("zoom.us")
        );
        assert_eq!(
            description_parts("Agenda: https://example.com/doc. Thanks"),
            [
                ("Agenda: ".to_owned(), false),
                ("https://example.com/doc.".to_owned(), true),
                (" Thanks".to_owned(), false)
            ]
        );
    }

    #[test]
    fn typed_participants_become_contacts_and_removed_attendees_stay_excluded() {
        futures::executor::block_on(async {
            let directory = tempfile::tempdir().unwrap();
            let (runtime, ready) = RuntimeHandle::start(desktop_runtime::Profile {
                database: directory.path().join("library.sqlite"),
            })
            .unwrap();
            ready.receive().await.unwrap();
            let sql = |sql: &'static str| {
                let runtime = runtime.clone();
                async move {
                    runtime
                        .submit(move |services| async move {
                            services
                                .executor
                                .execute(sql.into(), vec![])
                                .await
                                .map_err(failure)
                        })
                        .unwrap()
                        .receive()
                        .await
                        .unwrap()
                }
            };
            sql("INSERT INTO sessions(id,title,owner_user_id) VALUES ('s','Sync','me')").await;
            sql("INSERT INTO humans(id,name) VALUES ('maya','Maya Chen')").await;
            sql("INSERT INTO session_participants(id,session_id,human_id,display_name,source) VALUES ('p','s','maya','Maya Chen','auto')").await;
            sql(r#"INSERT INTO transcripts(id,session_id,speaker_hints_json) VALUES ('t','s','[{"type":"user_speaker_assignment","value":"{\"human_id\":\"maya\"}"}]')"#).await;
            let session = SessionId("s".into());
            let human = add_participant(&runtime, session.clone(), None, "Lee".into())
                .unwrap()
                .receive()
                .await
                .unwrap();
            let added = sql("SELECT h.name, p.source FROM session_participants p JOIN humans h ON h.id = p.human_id WHERE p.session_id = 's' AND h.name = 'Lee' AND p.deleted_at IS NULL").await;
            assert_eq!(added.len(), 1);
            assert_eq!(added[0]["source"], "manual");
            assert!(!human.is_empty());
            remove_participant(&runtime, session, "p".into(), "maya".into())
                .unwrap()
                .receive()
                .await
                .unwrap();
            // Calendar attendees are excluded rather than deleted, so a sync cannot re-add them.
            let removed =
                sql("SELECT source, deleted_at FROM session_participants WHERE id = 'p'").await;
            assert_eq!(removed[0]["source"], "excluded");
            assert_eq!(removed[0]["deleted_at"], Value::Null);
            assert_eq!(
                sql("SELECT speaker_hints_json FROM transcripts WHERE id = 't'").await[0]["speaker_hints_json"],
                "[]"
            );
            runtime.shutdown().await.unwrap();
        });
    }

    /// Changing the note's date once left the header holding the session's old revision, so
    /// the next rename's conditional update reported a conflict.
    #[gpui::test]
    fn a_rename_after_changing_the_date_is_saved(cx: &mut gpui::TestAppContext) {
        cx.executor().allow_parking();
        cx.update(crate::ui::theme::UiFonts::init);
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(desktop_runtime::Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        futures::executor::block_on(ready.receive()).unwrap();
        let open =
            futures::executor::block_on(runtime.create_note("Sync".into()).unwrap().receive())
                .unwrap();
        let created_at = open.summary.created_at.clone();
        let (note, cx) = cx
            .add_window_view(|window, cx| super::super::NoteView::new(runtime.clone(), window, cx));
        let settle = |cx: &mut gpui::VisualTestContext,
                      done: &dyn Fn(&mut gpui::VisualTestContext) -> bool| {
            for _ in 0..400 {
                cx.run_until_parked();
                if done(cx) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            panic!("the note did not settle");
        };
        note.update(cx, |note, cx| {
            note.show(open, cx);
            note.open_meeting_info(cx);
        });
        let info = note.read_with(cx, |note, _| note.meeting_info.as_ref().unwrap().0.clone());
        cx.update(|window, cx| info.update(cx, |info, cx| info.edit_date(window, cx)));
        info.update(cx, |info, cx| {
            let input = info.date.as_ref().unwrap().0.clone();
            input.update(cx, |input, cx| {
                input.set_text("2026-01-02 10:00".into(), cx)
            });
            info.save_date(cx);
        });
        settle(cx, &|cx| {
            note.read_with(cx, |note, _| {
                note.current.as_ref().unwrap().summary.created_at != created_at
            })
        });
        note.update(cx, |note, cx| {
            note.title
                .update(cx, |title, cx| title.set_text("Renamed".into(), cx));
            note.rename(cx);
        });
        settle(cx, &|cx| note.read_with(cx, |note, _| !note.busy));
        note.read_with(cx, |note, _| {
            assert!(!note.rename_error, "{}", note.message);
            assert_eq!(
                note.current.as_ref().unwrap().summary.title.as_ref(),
                "Renamed"
            );
        });
        futures::executor::block_on(runtime.shutdown()).unwrap();
    }
}
