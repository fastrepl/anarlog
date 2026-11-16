use std::{collections::BTreeMap, sync::Arc};

use desktop_runtime::{
    CancellationToken, Generation, Reply, Result, RuntimeHandle, ServiceError, SessionId,
};
use futures::{
    FutureExt,
    future::{Either, select},
};
use gpui::{
    Context, EventEmitter, Render, SharedString, UniformListScrollHandle, Window, canvas, div,
    prelude::*, px, uniform_list,
};
use serde_json::{Value, json};

use super::mutations::{failure, statement, transaction};
use crate::ui::theme::theme;

pub fn visible_columns(width: f32) -> usize {
    if width >= 700. {
        7
    } else if width >= 400. {
        4
    } else if width >= 200. {
        2
    } else {
        1
    }
}

#[derive(Clone, Debug)]
pub struct CalendarRequest {
    pub anchor: Arc<str>,
    pub columns: usize,
    pub week_start: u8,
    pub step: i32,
}

impl Default for CalendarRequest {
    fn default() -> Self {
        Self {
            anchor: "".into(),
            columns: 7,
            week_start: 0,
            step: 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CalendarItem {
    pub id: Arc<str>,
    pub title: Arc<str>,
    pub time: Arc<str>,
    pub location: Arc<str>,
    pub session: Option<SessionId>,
    pub all_day: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CalendarDay {
    pub date: Arc<str>,
    pub today: bool,
    pub items: Arc<[CalendarItem]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CalendarPage {
    pub anchor: Arc<str>,
    pub days: Arc<[CalendarDay]>,
    pub overflow: bool,
}

fn field(row: &Value, key: &str) -> Arc<str> {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .into()
}

const ITEM_QUERY: &str = "WITH candidates AS (
    SELECT e.id, substr(e.title,1,4096) AS title, e.started_at AS time,
        substr(e.location,1,4096) AS location, e.is_all_day AS all_day,
        CASE WHEN e.is_all_day THEN substr(e.started_at,1,10) ELSE date(e.started_at,'localtime') END AS start_day,
        CASE WHEN e.is_all_day THEN substr(e.ended_at,1,10) ELSE date(e.ended_at,'localtime') END AS end_day,
        (SELECT s.id FROM sessions s WHERE s.event_id = e.id AND s.deleted_at IS NULL ORDER BY s.created_at,s.id LIMIT 1) AS session_id
    FROM events e JOIN calendars c ON c.id = e.calendar_id
    WHERE e.deleted_at IS NULL AND c.deleted_at IS NULL AND c.enabled = 1
        AND NOT EXISTS(
            SELECT 1 FROM app_settings a,
                json_each(CASE WHEN json_valid(a.value_json) AND json_type(a.value_json) = 'array' THEN a.value_json ELSE '[]' END) ignored
            WHERE (a.id = 'ignored_events' AND json_extract(ignored.value,'$.tracking_id') = e.tracking_id_event)
                OR (a.id = 'ignored_recurring_series' AND json_extract(ignored.value,'$.id') = e.recurrence_series_id)
        )
    UNION ALL
    SELECT s.id, substr(s.title,1,4096), s.created_at, '', 0,
        date(s.created_at,'localtime'), date(s.created_at,'localtime'), s.id
    FROM sessions s WHERE s.deleted_at IS NULL AND (s.event_id IS NULL OR s.event_id = '')
    ) SELECT * FROM candidates WHERE start_day <= ?2 AND end_day >= ?1
    ORDER BY time,id LIMIT 1001";

pub fn load_calendar(
    runtime: &RuntimeHandle,
    request: CalendarRequest,
    cancel: CancellationToken,
) -> Result<Reply<CalendarPage>> {
    runtime.read(cancel, move |services| async move {
        let grid = services.executor.execute(
            "WITH RECURSIVE base AS (
                SELECT CASE WHEN ?2 = 7 AND ?4 != 0
                    THEN date(COALESCE(NULLIF(?1,''),date('now','localtime')), 'start of month', printf('%+d months',?4))
                    ELSE date(COALESCE(NULLIF(?1,''),date('now','localtime')), printf('%+d days',?4)) END AS anchor
                ), boundaries AS (
                    SELECT anchor,
                        CASE WHEN ?2 = 7 THEN date(anchor,'start of month',
                            printf('-%d days',(CAST(strftime('%w',date(anchor,'start of month')) AS INT) - ?3 + 7) % 7))
                            ELSE date(anchor,'-42 days') END AS first,
                        CASE WHEN ?2 = 7 THEN date(anchor,'start of month','+1 month','-1 day',
                            printf('+%d days',(6 - CAST(strftime('%w',date(anchor,'start of month','+1 month','-1 day')) AS INT) + ?3 + 7) % 7))
                            ELSE date(anchor,'+42 days') END AS last FROM base
                ), days(day, last, anchor) AS (
                    SELECT first,last,anchor FROM boundaries UNION ALL
                    SELECT date(day,'+1 day'),last,anchor FROM days WHERE day < last
                ) SELECT day,anchor, day = date('now','localtime') AS today FROM days LIMIT 85".into(),
            vec![json!(request.anchor),json!(request.columns),json!(request.week_start.min(6)),json!(request.step)],
        ).await.map_err(|error| ServiceError::Failed(error.to_string().into()))?;
        let first = grid.first().ok_or_else(|| ServiceError::Failed("Calendar range is empty.".into()))?;
        let from = field(first,"day");
        let to = field(grid.last().unwrap(),"day");
        let anchor = field(first,"anchor");
        let events = services.executor.execute(
            ITEM_QUERY.into(),
            vec![json!(from),json!(to)],
        ).await.map_err(|error| ServiceError::Failed(error.to_string().into()))?;
        let mut by_day = BTreeMap::<Arc<str>, Vec<CalendarItem>>::new();
        for day in &grid {
            let date = field(day,"day");
            let items = by_day.entry(date.clone()).or_default();
            for event in events.iter().take(1000) {
                let start = field(event,"start_day");
                let end = field(event,"end_day");
                let all_day = event["all_day"] == 1;
                if date < start || date > end || (all_day && end != start && date == end) {
                    continue;
                }
                items.push(CalendarItem {
                    id: field(event,"id"), title: field(event,"title"), time: field(event,"time"),
                    location: field(event,"location"), all_day,
                    session: event["session_id"].as_str().map(|id| SessionId(id.into())),
                });
            }
        }
        let days = grid.iter().map(|day| {
            let date = field(day,"day");
            CalendarDay { items: by_day.remove(&date).unwrap_or_default().into(), date, today: day["today"] == 1 }
        }).collect::<Vec<_>>();
        Ok(CalendarPage { anchor, days: days.into(), overflow: events.len() > 1000 })
    })
}

pub struct CalendarOpen(pub SessionId);

pub struct CalendarConnections;

pub fn open_event(
    runtime: &RuntimeHandle,
    event_id: Arc<str>,
    viewer: Option<Arc<str>>,
) -> Result<Reply<SessionId>> {
    runtime.submit(move |services| async move {
        let executor = &services.executor;
        let rows = executor.execute("SELECT * FROM events WHERE id=? AND deleted_at IS NULL".into(),vec![json!(event_id)]).await.map_err(failure)?;
        let event = rows.first().ok_or(ServiceError::Conflict)?;
        let participants: Value = serde_json::from_str(event["participants_json"].as_str().unwrap_or("[]")).map_err(failure)?;
        let participants = participants.as_array().ok_or_else(|| failure("Invalid event participants"))?;
        let id = uuid::Uuid::new_v4().to_string();
        let tracking = event["tracking_id_event"].as_str().unwrap_or("");
        let provider=event["provider"].as_str().unwrap_or("");
        let event_json = json!({
            "tracking_id":tracking,"calendar_id":event["calendar_id"],"title":event["title"],
            "started_at":event["started_at"],"ended_at":event["ended_at"],"is_all_day":event["is_all_day"]==1,
            "has_recurrence_rules":event["has_recurrence_rules"]==1,"location":event["location"],
            "meeting_link":event["meeting_link"],"description":event["description"],"recurrence_series_id":event["recurrence_series_id"]
        });
        transaction(executor,vec![
            statement("INSERT INTO sessions(id,workspace_id,owner_user_id,title,started_at,ended_at,event_id,external_event_id,external_provider,series_id,event_json)
                SELECT ?1,NULLIF((SELECT json_extract(value_json,'$.workspace_id') FROM app_settings WHERE id='cloudsync_workspace_binding'),''),
                    COALESCE((SELECT library_workspace_id FROM local_library_connections WHERE active=1),NULLIF(NULLIF(?6,''),'00000000-0000-0000-0000-000000000000'),NULLIF((SELECT json_extract(value_json,'$.workspace_id') FROM app_settings WHERE id='cloudsync_workspace_binding'),'')),
                    title,started_at,ended_at,id,tracking_id_event,provider,recurrence_series_id,?2
                FROM events WHERE id=?3 AND deleted_at IS NULL AND NOT EXISTS(SELECT 1 FROM sessions WHERE deleted_at IS NULL AND (event_id=?3 OR (?4<>'' AND external_event_id=?4 AND external_provider=?5)))".into(),
                vec![json!(id),json!(event_json.to_string()),json!(event_id),json!(tracking),json!(provider),json!(viewer)],None),
            statement("INSERT INTO session_documents(id,workspace_id,session_id,kind,body_format,body,created_by,updated_by) SELECT id,workspace_id,id,'note','prosemirror_json','{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\"}]}',owner_user_id,owner_user_id FROM sessions WHERE id=?".into(),vec![json!(id)],None),
        ]).await?;
        let rows = executor.execute("SELECT id FROM sessions WHERE deleted_at IS NULL AND (event_id=?1 OR (?2<>'' AND external_event_id=?2 AND external_provider=?3)) ORDER BY created_at,id LIMIT 1".into(),vec![json!(event_id),json!(tracking),json!(provider)]).await.map_err(failure)?;
        let id = rows.first().and_then(|row| row["id"].as_str()).ok_or(ServiceError::Conflict)?.to_owned();
        let mut statements = Vec::new();
        let mut emails = std::collections::HashSet::new();
        for person in participants {
            if person["is_current_user"]==true { continue; }
            let email = person["email"].as_str().unwrap_or("").trim().to_lowercase();
            if email.is_empty() || !emails.insert(email.clone()) { continue; }
            let human = uuid::Uuid::new_v4().to_string();
            let name = person["name"].as_str().filter(|name| !name.is_empty()).unwrap_or(&email);
            statements.push(statement("INSERT INTO humans(id,workspace_id,owner_user_id,name,email) SELECT ?1,workspace_id,owner_user_id,?2,?3 FROM sessions WHERE id=?4 AND NOT EXISTS(SELECT 1 FROM humans WHERE lower(email)=?3 AND deleted_at IS NULL)".into(),vec![json!(human),json!(name),json!(email),json!(id)],None));
            statements.push(statement("INSERT INTO session_participants(id,workspace_id,owner_user_id,session_id,human_id,display_name,email,source)
                SELECT ?1,s.workspace_id,s.owner_user_id,s.id,h.id,?2,?3,'auto' FROM sessions s JOIN humans h ON lower(h.email)=?3 AND h.deleted_at IS NULL
                WHERE s.id=?4 AND h.id IS NOT s.owner_user_id
                AND NOT EXISTS(SELECT 1 FROM humans owner WHERE owner.id=s.owner_user_id AND lower(owner.email)=?3 AND owner.deleted_at IS NULL)
                AND NOT EXISTS(SELECT 1 FROM session_participants p WHERE p.session_id=s.id AND p.deleted_at IS NULL AND (p.human_id=h.id OR lower(p.email)=?3))
                ORDER BY h.id LIMIT 1".into(),vec![json!(uuid::Uuid::new_v4().to_string()),json!(name),json!(email),json!(id)],None));
        }
        transaction(executor,statements).await?;
        Ok(id.into())
    })
}

pub struct CalendarView {
    runtime: RuntimeHandle,
    request: CalendarRequest,
    page: Option<Arc<CalendarPage>>,
    cancel: CancellationToken,
    generation: Generation,
    scroll: UniformListScrollHandle,
    message: String,
    selected: Option<CalendarItem>,
    watch_cancel: CancellationToken,
    watch_range: Option<(Arc<str>, Arc<str>)>,
    opening: bool,
    pub(super) viewer: Option<Arc<str>>,
}

impl EventEmitter<CalendarOpen> for CalendarView {}
impl EventEmitter<CalendarConnections> for CalendarView {}

impl CalendarView {
    pub fn new(runtime: RuntimeHandle) -> Self {
        Self {
            opening: false,
            viewer: None,
            runtime,
            request: CalendarRequest::default(),
            page: None,
            cancel: CancellationToken::new(),
            generation: Generation::default(),
            scroll: UniformListScrollHandle::new(),
            message: String::new(),
            selected: None,
            watch_cancel: CancellationToken::new(),
            watch_range: None,
        }
    }

    pub fn set_week_start(&mut self, week_start: u8, cx: &mut Context<Self>) {
        if self.request.week_start != week_start.min(6) {
            self.request.week_start = week_start.min(6);
            if self.watch_range.is_some() {
                self.load(cx);
            }
        }
    }

    pub fn suspend(&mut self) {
        self.cancel.cancel();
        self.generation.advance();
        self.watch_cancel.cancel();
        self.watch_range = None;
    }

    pub fn activate(&mut self, cx: &mut Context<Self>) {
        self.load(cx);
    }

    fn watch_range(&mut self, from: Arc<str>, to: Arc<str>, cx: &mut Context<Self>) {
        if self.watch_range.as_ref() == Some(&(from.clone(), to.clone())) {
            return;
        }
        self.watch_range = Some((from.clone(), to.clone()));
        self.watch_cancel.cancel();
        self.watch_cancel = CancellationToken::new();
        let cancel = self.watch_cancel.clone();
        let reply = self
            .runtime
            .watch_query(ITEM_QUERY.into(), vec![json!(from), json!(to)]);
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let mut watch = match result {
                Ok(watch) => watch,
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        if !cancel.is_cancelled() {
                            this.message = format!(
                                "Calendar watch failed: {error}. Use Reload for stored changes."
                            );
                            this.watch_range = None;
                            cx.notify();
                        }
                    });
                    return;
                }
            };
            loop {
                if cancel.is_cancelled() {
                    break;
                }
                watch.snapshots.borrow_and_update();
                if cancel.is_cancelled() {
                    break;
                }
                let error = watch
                    .terminal_error()
                    .or_else(|| watch.errors.try_recv().ok());
                if this
                    .update(cx, |this, cx| {
                        if let Some(error) = error {
                            this.message =
                                format!("Calendar watch failed: {error}. Previous dates retained.");
                            cx.notify();
                        } else {
                            this.refresh(false, cx);
                        }
                    })
                    .is_err()
                    || watch.terminal_error().is_some()
                {
                    break;
                }
                match select(
                    watch.snapshots.changed().boxed(),
                    cancel.cancelled().boxed(),
                )
                .await
                {
                    Either::Left((Ok(()), _)) => {}
                    _ => break,
                }
            }
            let _ = watch.unsubscribe().await;
        })
        .detach();
    }

    pub fn load(&mut self, cx: &mut Context<Self>) {
        self.refresh(true, cx);
    }

    fn refresh(&mut self, feedback: bool, cx: &mut Context<Self>) {
        self.cancel.cancel();
        self.cancel = CancellationToken::new();
        let generation = self.generation.advance();
        let reply = load_calendar(&self.runtime, self.request.clone(), self.cancel.clone());
        if feedback {
            self.message = "Loading stored calendar…".into();
            cx.notify();
        }
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                if generation != this.generation {
                    return;
                }
                match result {
                    Ok(page) => {
                        if this.page.as_deref() == Some(&page) && this.message.is_empty() {
                            return;
                        }
                        if let (Some(first), Some(last)) = (page.days.first(), page.days.last()) {
                            this.watch_range(first.date.clone(), last.date.clone(), cx);
                        }
                        this.request.anchor = page.anchor.clone();
                        this.request.step = 0;
                        this.message = if page.overflow {
                            "Only the first 1,000 items in this range are shown."
                        } else {
                            ""
                        }
                        .into();
                        this.page = Some(Arc::new(page));
                        if feedback && this.request.columns != 7 {
                            this.scroll.scroll_to_item(
                                42 / this.request.columns,
                                gpui::ScrollStrategy::Top,
                            );
                        }
                    }
                    Err(error) => {
                        this.message =
                            format!("Calendar load failed: {error}. Previous dates retained.")
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn step(&mut self, direction: i32, cx: &mut Context<Self>) {
        self.request.step += if self.request.columns == 7 {
            direction
        } else {
            direction * self.request.columns as i32
        };
        self.load(cx);
    }
}

impl Drop for CalendarView {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.watch_cancel.cancel();
    }
}

impl Render for CalendarView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let cols = self.request.columns;
        let days = self
            .page
            .as_ref()
            .map(|page| page.days.clone())
            .unwrap_or_else(|| Arc::from([]));
        let entity = cx.entity().downgrade();
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(48.))
                    .flex_shrink_0()
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_lg()
                            .flex_1()
                            .child(SharedString::from(self.request.anchor.clone())),
                    )
                    .child(
                        div()
                            .id("calendar-connections")
                            .cursor_pointer()
                            .child("Calendar connections")
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(CalendarConnections))),
                    )
                    .child(
                        div()
                            .id("calendar-previous")
                            .cursor_pointer()
                            .child("Previous")
                            .on_click(cx.listener(|this, _, _, cx| this.step(-1, cx))),
                    )
                    .child(
                        div()
                            .id("calendar-today")
                            .cursor_pointer()
                            .child("Today")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.request.anchor = "".into();
                                this.request.step = 0;
                                this.load(cx);
                            })),
                    )
                    .child(
                        div()
                            .id("calendar-next")
                            .cursor_pointer()
                            .child("Next")
                            .on_click(cx.listener(|this, _, _, cx| this.step(1, cx))),
                    )
                    .child(
                        div()
                            .id("calendar-refresh")
                            .cursor_pointer()
                            .child("Reload")
                            .on_click(cx.listener(|this, _, _, cx| this.load(cx))),
                    ),
            )
            .child(
                div()
                    .px_4()
                    .pb_2()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child("Events from enabled calendars stored on this device."),
            )
            .child(
                div().flex_1().min_h_0().child(
                    uniform_list(
                        "calendar-weeks",
                        days.len().div_ceil(cols),
                        cx.processor(move |_, range: std::ops::Range<usize>, _, cx| {
                            range
                                .map(|row| {
                                    div().h(px(190.)).flex().children(
                                        days.iter().skip(row * cols).take(cols).map(|day| {
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .h_full()
                                                .border_1()
                                                .border_color(colors.border)
                                                .px_2()
                                                .py_1()
                                                .flex()
                                                .flex_col()
                                                .gap_1()
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .text_color(if day.today {
                                                            colors.ring
                                                        } else {
                                                            colors.foreground
                                                        })
                                                        .child(SharedString::from(
                                                            day.date.clone(),
                                                        )),
                                                )
                                                .children(day.items.iter().take(5).map(|item| {
                                                    let item = item.clone();
                                                    let title = item.title.clone();
                                                    div()
                                                        .id(SharedString::from(format!(
                                                            "{}:{}",
                                                            day.date, item.id
                                                        )))
                                                        .rounded(px(8.))
                                                        .px_1()
                                                        .bg(colors.accent)
                                                        .text_xs()
                                                        .truncate()
                                                        .cursor_pointer()
                                                        .child(SharedString::from(title))
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.selected = Some(item.clone());
                                                                cx.notify();
                                                            },
                                                        ))
                                                }))
                                                .when(day.items.len() > 5, |view| {
                                                    view.child(div().text_xs().child(format!(
                                                        "{} more stored items",
                                                        day.items.len() - 5
                                                    )))
                                                })
                                        }),
                                    )
                                })
                                .collect()
                        }),
                    )
                    .track_scroll(&self.scroll)
                    .size_full(),
                ),
            )
            .when(!self.message.is_empty(), |view| {
                view.child(div().px_4().text_xs().child(self.message.clone()))
            })
            .when_some(self.selected.clone(), |view, item| {
                view.child(
                    div()
                        .p_4()
                        .border_t_1()
                        .border_color(colors.border)
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .child(SharedString::from(item.title.clone())),
                        )
                        .child(div().text_xs().child(if item.all_day {
                            format!("All day · {}", item.time)
                        } else {
                            item.time.to_string()
                        }))
                        .child(
                            div()
                                .text_xs()
                                .child(SharedString::from(item.location.clone())),
                        )
                        .when_some(item.session.clone(), |view, id| {
                            view.child(
                                div()
                                    .id("calendar-open-note")
                                    .cursor_pointer()
                                    .child("Open note")
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        cx.emit(CalendarOpen(id.clone()))
                                    })),
                            )
                        })
                        .when(item.session.is_none(), |view| {
                            let id = item.id.clone();
                            view.child(
                                div()
                                    .id("calendar-create-note")
                                    .cursor_pointer()
                                    .child(if self.opening {
                                        "Opening…"
                                    } else {
                                        "Create note for this event"
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if this.opening {
                                            return;
                                        }
                                        let reply = open_event(
                                            &this.runtime,
                                            id.clone(),
                                            this.viewer.clone(),
                                        );
                                        this.opening = true;
                                        cx.notify();
                                        cx.spawn(async move |this, cx| {
                                            let result = match reply {
                                                Ok(reply) => reply.receive().await,
                                                Err(error) => Err(error),
                                            };
                                            let _ = this.update(cx, |this, cx| {
                                                this.opening = false;
                                                match result {
                                                    Ok(id) => {
                                                        cx.emit(CalendarOpen(id));
                                                        this.load(cx);
                                                    }
                                                    Err(error) => {
                                                        this.message = format!(
                                                            "Could not open event: {error}"
                                                        );
                                                        cx.notify();
                                                    }
                                                }
                                            });
                                        })
                                        .detach();
                                    })),
                            )
                        })
                        .child(
                            div()
                                .id("calendar-dismiss")
                                .text_xs()
                                .cursor_pointer()
                                .child("Close details")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.selected = None;
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let columns = visible_columns(f32::from(bounds.size.width));
                        if columns != cols {
                            window.defer(cx, move |_, cx| {
                                let _ = entity.update(cx, |this, cx| {
                                    if this.request.columns != columns {
                                        this.request.columns = columns;
                                        this.request.step = 0;
                                        this.load(cx);
                                    }
                                });
                            });
                        }
                    },
                    |_, (), _, _| {},
                )
                .absolute()
                .size_full(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipping_calendar_breakpoints_are_exact() {
        for (width, cols) in [
            (0., 1),
            (199., 1),
            (200., 2),
            (399., 2),
            (400., 4),
            (699., 4),
            (700., 7),
        ] {
            assert_eq!(visible_columns(width), cols);
        }
    }
}
