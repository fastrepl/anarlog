//! The overflow menu's "Meeting info" submenu:
//! `session/components/outer-header/metadata` (`MetadataPanelContent`,
//! `DateEditor`, `EventDisplay`, `ParticipantsDisplay`).

use chrono::{Datelike, TimeZone};
use gpui::{
    AnyElement, ClickEvent, Context, Div, Entity, Focusable as _, MouseButton, SharedString,
    Window, div, prelude::*, px,
};

use super::Workspace;
use crate::db::{Human, ParticipantTarget, SessionParticipant};
use crate::text_input::{TextInput, TextInputEvent, TextInputStyle};
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon};

/// `useParticipantMappings` + the date editor + the picker input state for the
/// session whose overflow menu is open.
pub(crate) struct MeetingInfo {
    session_id: String,
    participants: Vec<SessionParticipant>,
    humans: Vec<Human>,
    input: Entity<TextInput>,
    /// `showDropdown && inputValue.trim()`
    dropdown_open: bool,
    selected_index: usize,
    /// Names added but not yet listed by the query (`pendingAdds`).
    pending_adds: Vec<String>,
    /// `EditableDateForm` while the pencil is active.
    date_editor: Option<Entity<TextInput>>,
    date_error: Option<&'static str>,
    /// Masks the live `created_at` until it catches up with the save.
    pending_created_at: Option<String>,
}

/// A row of the picker dropdown (`useDropdownOptions`).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate {
    id: String,
    name: String,
    job_title: String,
    is_new: bool,
}

impl Workspace {
    fn panel_input_style(&self) -> TextInputStyle {
        TextInputStyle {
            text: self.theme.foreground,
            placeholder: self.theme.muted_foreground,
            selection: self.theme.selection,
            underline_when_focused: false,
            masked: false,
        }
    }

    /// Prepares the panel state for `session_id` (called when the overflow
    /// menu opens) and loads participants and contacts.
    pub(crate) fn prepare_meeting_info(
        &mut self,
        session_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .meeting_info
            .as_ref()
            .is_some_and(|info| info.session_id == session_id)
        {
            self.reload_meeting_info(cx);
            return;
        }
        let style = self.panel_input_style();
        let input = cx.new(|cx| TextInput::new("Add participants", style, window, cx));
        cx.subscribe_in(
            &input,
            window,
            |this, _, event: &TextInputEvent, window, cx| {
                match event {
                    TextInputEvent::Changed => {
                        if let Some(info) = this.meeting_info.as_mut() {
                            info.dropdown_open = true;
                            info.selected_index = 0;
                            cx.notify();
                        }
                    }
                    // Enter (or Tab) adds the highlighted option.
                    TextInputEvent::Enter => this.pick_meeting_participant(window, cx),
                    TextInputEvent::Navigate(delta) => {
                        let count = this.meeting_candidates(cx).len();
                        let Some(info) = this.meeting_info.as_mut() else {
                            return;
                        };
                        if count > 0 {
                            let index = info.selected_index as i32 + delta;
                            info.selected_index = index.clamp(0, count as i32 - 1) as usize;
                            cx.notify();
                        }
                    }
                    TextInputEvent::Escape => {
                        if let Some(info) = this.meeting_info.as_mut() {
                            info.dropdown_open = false;
                            cx.notify();
                        }
                    }
                    // Backspace on an empty field removes the last participant.
                    TextInputEvent::BackspaceEmpty => {
                        let last = this.meeting_info.as_ref().and_then(|info| {
                            info.participants
                                .iter()
                                .rfind(|participant| participant.source != "excluded")
                                .cloned()
                        });
                        if let Some(last) = last {
                            this.remove_meeting_participant(last, cx);
                        }
                    }
                    TextInputEvent::Committed => {}
                }
            },
        )
        .detach();
        self.meeting_info = Some(MeetingInfo {
            session_id,
            participants: Vec::new(),
            humans: Vec::new(),
            input,
            dropdown_open: false,
            selected_index: 0,
            pending_adds: Vec::new(),
            date_editor: None,
            date_error: None,
            pending_created_at: None,
        });
        self.reload_meeting_info(cx);
    }

    /// Re-runs `useSessionParticipants` and `useHumans`.
    pub(crate) fn reload_meeting_info(&mut self, cx: &mut Context<Self>) {
        let Some(info) = self.meeting_info.as_ref() else {
            return;
        };
        let session_id = info.session_id.clone();
        let participants = self.store.list_session_participants(session_id.clone());
        let humans = self.store.list_humans();
        cx.spawn(async move |this, cx| {
            let participants = participants.await.ok().and_then(Result::ok);
            let humans = humans.await.ok().and_then(Result::ok);
            this.update(cx, |this, cx| {
                if let Some(info) = this
                    .meeting_info
                    .as_mut()
                    .filter(|info| info.session_id == session_id)
                {
                    if let Some(participants) = participants {
                        // A pending chip disappears once its human is listed.
                        info.pending_adds.retain(|name| {
                            !participants
                                .iter()
                                .any(|participant| participant.name == *name)
                        });
                        info.participants = participants;
                    }
                    if let Some(humans) = humans {
                        info.humans = humans;
                    }
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// `useCandidateSearch` + `useDropdownOptions`.
    fn meeting_candidates(&self, cx: &gpui::App) -> Vec<Candidate> {
        let Some(info) = self.meeting_info.as_ref() else {
            return Vec::new();
        };
        let query = info.input.read(cx).text().to_string();
        let query_lower = query.to_lowercase();
        let existing: std::collections::HashSet<&str> = info
            .participants
            .iter()
            .filter(|participant| participant.source != "excluded")
            .map(|participant| participant.human_id.as_str())
            .collect();
        let mut candidates: Vec<Candidate> = info
            .humans
            .iter()
            .filter(|human| !existing.contains(human.id.as_str()))
            .filter(|human| {
                query.is_empty()
                    || human.name.to_lowercase().contains(&query_lower)
                    || human.email.to_lowercase().contains(&query_lower)
                    || human.phone.to_lowercase().contains(&query_lower)
            })
            .map(|human| Candidate {
                id: human.id.clone(),
                name: human.name.clone(),
                job_title: human.job_title.clone(),
                is_new: false,
            })
            .collect();
        let trimmed = query.trim();
        let show_custom = !trimmed.is_empty()
            && !candidates
                .iter()
                .any(|candidate| candidate.name.to_lowercase() == query_lower);
        if show_custom {
            candidates.insert(
                0,
                Candidate {
                    id: "new".to_string(),
                    name: trimmed.to_string(),
                    job_title: String::new(),
                    is_new: true,
                },
            );
        }
        candidates
    }

    /// `handleSelect` / Enter: add the highlighted candidate and reset the field.
    fn pick_meeting_participant(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let candidates = self.meeting_candidates(cx);
        let Some(info) = self.meeting_info.as_mut() else {
            return;
        };
        if info.input.read(cx).text().trim().is_empty() {
            return;
        }
        let Some(candidate) = candidates.get(info.selected_index).cloned() else {
            return;
        };
        let target = if candidate.is_new {
            ParticipantTarget::New(candidate.name.clone())
        } else {
            ParticipantTarget::Existing(candidate.id.clone())
        };
        info.pending_adds.push(candidate.name.clone());
        info.dropdown_open = false;
        info.selected_index = 0;
        let input = info.input.clone();
        let session_id = info.session_id.clone();
        input.update(cx, |input, cx| input.set_text("", cx));
        input.read(cx).focus_handle(cx).focus(window);
        let task = self.store.add_session_participant(session_id, target);
        cx.spawn(async move |this, cx| {
            match task.await.map_err(anyhow::Error::from).and_then(|r| r) {
                Ok(()) => {}
                Err(error) => tracing::error!(%error, "failed to add participant"),
            }
            this.update(cx, |this, cx| {
                if let Some(info) = this.meeting_info.as_mut() {
                    info.pending_adds.retain(|name| *name != candidate.name);
                }
                this.reload_meeting_info(cx);
            })
            .ok();
        })
        .detach();
    }

    /// `useRemoveParticipant`: drop speaker assignments, then the mapping.
    fn remove_meeting_participant(
        &mut self,
        participant: SessionParticipant,
        cx: &mut Context<Self>,
    ) {
        let Some(info) = self.meeting_info.as_mut() else {
            return;
        };
        info.participants
            .retain(|existing| existing.id != participant.id);
        let task = self.store.remove_session_participant(
            info.session_id.clone(),
            participant.id.clone(),
            participant.human_id.clone(),
        );
        cx.spawn(async move |this, cx| {
            if let Err(error) = task.await.map_err(anyhow::Error::from).and_then(|r| r) {
                tracing::error!(%error, "failed to remove participant");
            }
            this.update(cx, |this, cx| this.reload_meeting_info(cx))
                .ok();
        })
        .detach();
        cx.notify();
    }

    /// The pencil: open `EditableDateForm` seeded with `yyyy-MM-dd'T'HH:mm`.
    fn start_date_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let created_at = self.current_created_at();
        let style = self.panel_input_style();
        let seed = created_at
            .as_deref()
            .and_then(|value| crate::timeline::parse_date(value, &chrono::Local))
            .map(|utc| {
                utc.with_timezone(&chrono::Local)
                    .format("%Y-%m-%dT%H:%M")
                    .to_string()
            })
            .unwrap_or_default();
        let editor = cx.new(|cx| {
            let mut input = TextInput::new("", style, window, cx);
            input.set_text(seed, cx);
            input
        });
        cx.subscribe_in(
            &editor,
            window,
            |this, _, event: &TextInputEvent, window, cx| match event {
                TextInputEvent::Enter => this.save_date_edit(window, cx),
                TextInputEvent::Escape => this.cancel_date_edit(cx),
                TextInputEvent::Changed => {
                    if let Some(info) = this.meeting_info.as_mut() {
                        info.date_error = None;
                        cx.notify();
                    }
                }
                _ => {}
            },
        )
        .detach();
        editor.read(cx).focus_handle(cx).focus(window);
        if let Some(info) = self.meeting_info.as_mut() {
            info.date_editor = Some(editor);
            info.date_error = None;
        }
        cx.notify();
    }

    fn cancel_date_edit(&mut self, cx: &mut Context<Self>) {
        if let Some(info) = self.meeting_info.as_mut() {
            info.date_editor = None;
            info.date_error = None;
        }
        cx.notify();
    }

    /// `form.handleSubmit`: validate, write `created_at`, mask until it lands.
    fn save_date_edit(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(info) = self.meeting_info.as_mut() else {
            return;
        };
        let Some(editor) = info.date_editor.clone() else {
            return;
        };
        let value = editor.read(cx).text().trim().to_string();
        if value.is_empty() {
            info.date_error = Some("Date and time are required");
            cx.notify();
            return;
        }
        let Some(iso) = datetime_local_to_iso(&value) else {
            info.date_error = Some("Enter a valid date and time");
            cx.notify();
            return;
        };
        info.date_editor = None;
        info.date_error = None;
        info.pending_created_at = Some(iso.clone());
        let session_id = info.session_id.clone();
        let task = self.store.update_created_at(session_id.clone(), iso);
        cx.spawn(async move |this, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update(cx, |this, cx| {
                match result {
                    Ok(()) => this.reload_note(session_id, cx),
                    Err(error) => {
                        tracing::error!(%error, "failed to update session date");
                        if let Some(info) = this.meeting_info.as_mut() {
                            info.pending_created_at = None;
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn current_created_at(&self) -> Option<String> {
        match &self.note {
            super::Note::Ready { preview, .. } => Some(preview.session.created_at.clone()),
            _ => None,
        }
    }

    /// `MetadataPanelContent`: `flex flex-col gap-4 p-4` inside the panel.
    pub(crate) fn render_meeting_info_panel(&self, cx: &Context<Self>) -> AnyElement {
        let event = match &self.note {
            super::Note::Ready { preview, .. } => preview.session_event(),
            _ => None,
        };
        let mut body = div().flex().flex_col().min_w_0().gap_4().p_4();
        match event {
            Some(event) => {
                body = body.child(self.render_event_display(&event, cx));
            }
            None => {
                body = body
                    .child(self.render_date_editor(cx))
                    .child(self.render_participants(cx));
            }
        }
        body.into_any_element()
    }

    /// `DateEditor`: the `h-7` read-only row with the pencil, or the form.
    fn render_date_editor(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let info = self.meeting_info.as_ref();
        if let Some(editor) = info.and_then(|info| info.date_editor.clone()) {
            let error = info.and_then(|info| info.date_error);
            let hovered_cancel = self.hovered == Some("date-cancel");
            let hovered_save = self.hovered == Some("date-save");
            let (red_bg, red_fg, green_bg, green_fg) = if theme.dark {
                (
                    alpha(gpui::rgb(0x460809), 0.5),
                    gpui::rgb(0xffa2a2),
                    alpha(gpui::rgb(0x032e15), 0.5),
                    gpui::rgb(0x7bf1a8),
                )
            } else {
                (
                    gpui::rgb(0xfef2f2),
                    gpui::rgb(0xe7000b),
                    gpui::rgb(0xf0fdf4),
                    gpui::rgb(0x00a63e),
                )
            };
            let round_button = |id: &'static str,
                                glyph: &'static str,
                                hovered: bool,
                                bg: gpui::Rgba,
                                fg: gpui::Rgba| {
                div()
                    .id(id)
                    .relative()
                    .size(px(28.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .when(hovered, |button| {
                        button.child(crate::squircle::squircle(
                            crate::squircle::CONTROL_RADIUS,
                            Some(bg),
                            None,
                        ))
                    })
                    .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                        this.set_hovered(id, *hovering, cx);
                    }))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(icon(
                        glyph,
                        px(16.0),
                        if hovered { fg } else { theme.muted_foreground },
                    ))
            };
            return div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .min_w_0()
                        .flex_col()
                        .gap_1()
                        .child(
                            // `Input type="datetime-local"` stripped of its chrome.
                            div()
                                .h(px(28.0))
                                .w_full()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .tw_text_sm()
                                .child(editor),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_shrink_0()
                                .items_center()
                                .justify_end()
                                .child(
                                    round_button(
                                        "date-cancel",
                                        "x",
                                        hovered_cancel,
                                        red_bg,
                                        red_fg,
                                    )
                                    .on_click(cx.listener(
                                        |this, _: &ClickEvent, _, cx| this.cancel_date_edit(cx),
                                    )),
                                )
                                .child(
                                    round_button(
                                        "date-save",
                                        "check",
                                        hovered_save,
                                        green_bg,
                                        green_fg,
                                    )
                                    .on_click(cx.listener(
                                        |this, _: &ClickEvent, window, cx| {
                                            this.save_date_edit(window, cx)
                                        },
                                    )),
                                ),
                        ),
                )
                .when_some(error, |column, error| {
                    column.child(
                        div()
                            .tw_text_xs()
                            .text_color(gpui::rgb(0xe7000b))
                            .child(error),
                    )
                });
        }

        let created_at = info
            .and_then(|info| info.pending_created_at.clone())
            .or_else(|| self.current_created_at());
        let label = created_at
            .as_deref()
            .and_then(|value| crate::timeline::parse_date(value, &chrono::Local))
            .map(|utc| format_medium(&utc.with_timezone(&chrono::Local)))
            .unwrap_or_else(|| "Unknown date".to_string());
        let hovered = self.hovered == Some("date-edit");
        div()
            .flex()
            .h(px(28.0))
            .items_center()
            .justify_between()
            .gap_3()
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .tw_text_sm()
                    .text_color(theme.muted_foreground)
                    .child(SharedString::from(label)),
            )
            .child(
                div()
                    .id("date-edit")
                    .relative()
                    .size(px(28.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .when(hovered, |button| {
                        button.child(crate::squircle::squircle(
                            crate::squircle::CONTROL_RADIUS,
                            Some(theme.accent),
                            None,
                        ))
                    })
                    .on_hover(cx.listener(|this, hovering: &bool, _, cx| {
                        this.set_hovered("date-edit", *hovering, cx);
                    }))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.start_date_edit(window, cx)
                    }))
                    .child(icon(
                        "pencil-edit",
                        px(16.0),
                        if hovered {
                            theme.foreground
                        } else {
                            theme.muted_foreground
                        },
                    )),
            )
    }

    /// `EventDisplay`: title, rule, location, meeting link with Join, the
    /// time range, participants, and the description.
    pub(super) fn render_event_display(
        &self,
        event: &crate::timeline::SessionEvent,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let rule = || div().h(px(1.0)).bg(theme.accent);
        let title = event
            .title
            .clone()
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| "Untitled Event".to_string());
        let location = event
            .location
            .clone()
            .filter(|location| !location.is_empty() && url::Url::parse(location).is_err());
        let meeting_link = event.meeting_link.clone().filter(|link| !link.is_empty());
        let meeting_domain = meeting_link.as_deref().and_then(|link| {
            url::Url::parse(link).ok().and_then(|url| {
                url.host_str()
                    .map(|host| host.trim_start_matches("www.").to_string())
            })
        });
        let time_range = event.started_at.as_deref().and_then(|started| {
            let start =
                crate::timeline::parse_date(started, &chrono::Local)?.with_timezone(&chrono::Local);
            let start_text = format_medium(&start);
            let Some(end) = event
                .ended_at
                .as_deref()
                .and_then(|ended| crate::timeline::parse_date(ended, &chrono::Local))
                .map(|utc| utc.with_timezone(&chrono::Local))
            else {
                return Some(start_text);
            };
            let same_day = start.date_naive() == end.date_naive();
            let end_text = if same_day {
                end.format("%-I:%M %p").to_string()
            } else {
                format_medium(&end)
            };
            Some(format!("{start_text} to {end_text}"))
        });
        let description = event
            .description
            .clone()
            .filter(|description| !description.is_empty());

        let mut column = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .min_w_0()
                    .tw_text_base()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.foreground)
                    .child(SharedString::from(title)),
            )
            .child(rule());
        if let Some(location) = location {
            column = column.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .tw_text_sm()
                    .text_color(theme.muted_foreground)
                    .child(icon("map-pin", px(16.0), theme.muted_foreground))
                    .child(div().min_w_0().child(SharedString::from(location))),
            );
        }
        if let Some(link) = meeting_link {
            let hovered = self.hovered == Some("event-join");
            column = column.child(
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
                            .tw_text_sm()
                            .text_color(theme.muted_foreground)
                            .child(icon("video-camera", px(16.0), theme.muted_foreground))
                            .child(div().truncate().child(SharedString::from(
                                meeting_domain.unwrap_or_else(|| "Meeting link".to_string()),
                            ))),
                    )
                    .child(
                        // `Button size="sm" variant="default"`: `h-8 px-3 text-sm`.
                        div()
                            .id("event-join")
                            .relative()
                            .flex_shrink_0()
                            .flex()
                            .h(px(32.0))
                            .items_center()
                            .px_3()
                            .child(crate::squircle::squircle(
                                crate::squircle::CONTROL_RADIUS,
                                Some(if hovered {
                                    alpha(theme.primary, 0.9)
                                } else {
                                    theme.primary
                                }),
                                None,
                            ))
                            .tw_text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.primary_foreground)
                            .cursor_pointer()
                            .on_hover(cx.listener(|this, hovering: &bool, _, cx| {
                                this.set_hovered("event-join", *hovering, cx);
                            }))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(move |_, _, cx| cx.open_url(&link))
                            .child("Join"),
                    ),
            );
        }
        if let Some(range) = time_range {
            column = column.child(
                div()
                    .tw_text_sm()
                    .text_color(theme.muted_foreground)
                    .child(SharedString::from(range)),
            );
        }
        column = column.child(self.render_participants(cx));
        if let Some(description) = description {
            column = column.child(rule()).child(
                div()
                    .tw_text_sm()
                    .text_color(theme.muted_foreground)
                    .child(SharedString::from(description)),
            );
        }
        column
    }

    /// `ParticipantsDisplay`: the `bg-accent` rule, then the chip input.
    fn render_participants(&self, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let Some(info) = self.meeting_info.as_ref() else {
            return div();
        };
        let chips = info
            .participants
            .iter()
            .filter(|participant| participant.source != "excluded")
            .filter_map(|participant| {
                let name = if participant.name.trim().is_empty() {
                    participant.email.trim().to_string()
                } else {
                    participant.name.trim().to_string()
                };
                if name.is_empty() {
                    return None;
                }
                let participant = participant.clone();
                let hover_id: SharedString = format!("participant-{}", participant.id).into();
                Some(
                    // `Badge variant="secondary"`: `bg-foreground/10 px-2 py-0.5
                    // text-xs rounded-full` under the control squircle, with the
                    // 12px remove button.
                    div()
                        .id(hover_id.clone())
                        .relative()
                        .flex()
                        .items_center()
                        .gap_1()
                        .px_2()
                        .py(px(2.0))
                        .child(crate::squircle::squircle(
                            crate::squircle::CONTROL_RADIUS,
                            Some(alpha(theme.foreground, 0.1)),
                            None,
                        ))
                        .tw_text_xs()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.foreground)
                        .cursor_pointer()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(SharedString::from(name))
                        .child(
                            div()
                                .id(SharedString::from(format!("remove-{}", participant.id)))
                                .ml(px(2.0))
                                .size(px(12.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.remove_meeting_participant(participant.clone(), cx);
                                }))
                                .child(icon("x", px(10.0), theme.foreground)),
                        )
                        .into_any_element(),
                )
            })
            .collect::<Vec<_>>();
        let pending = info.pending_adds.iter().map(|name| {
            div()
                .relative()
                .flex()
                .items_center()
                .gap_1()
                .px_2()
                .py(px(2.0))
                .opacity(0.6)
                .child(crate::squircle::squircle(
                    crate::squircle::CONTROL_RADIUS,
                    Some(alpha(theme.foreground, 0.1)),
                    None,
                ))
                .tw_text_xs()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.foreground)
                .child(SharedString::from(name.clone()))
                .child(icon("arrows-clockwise", px(10.0), theme.foreground))
                .into_any_element()
        });
        let input = info.input.clone();
        let show_dropdown = info.dropdown_open && !info.input.read(cx).text().trim().is_empty();
        let candidates = if show_dropdown {
            self.meeting_candidates(cx)
        } else {
            Vec::new()
        };
        let selected = info.selected_index;

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().h(px(1.0)).bg(theme.accent))
            .child(
                div()
                    .relative()
                    .child(
                        div()
                            .id("participant-input")
                            .flex()
                            .min_h(px(38.0))
                            .w_full()
                            .flex_wrap()
                            .items_center()
                            .gap_2()
                            .cursor_text()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                if let Some(info) = this.meeting_info.as_mut() {
                                    info.input.read(cx).focus_handle(cx).focus(window);
                                    info.dropdown_open = true;
                                    cx.notify();
                                }
                            }))
                            .children(chips)
                            .children(pending)
                            .child(div().min_w(px(120.0)).flex_1().tw_text_sm().child(input)),
                    )
                    .when(show_dropdown && !candidates.is_empty(), |anchor| {
                        anchor.child(self.render_participant_dropdown(candidates, selected, cx))
                    }),
            )
    }

    /// `ParticipantDropdown`: `bg-popover rounded-md border shadow-md`, rows
    /// `px-3 py-1.5 text-sm`, the highlighted one `bg-muted` with the ⏎ glyph.
    fn render_participant_dropdown(
        &self,
        candidates: Vec<Candidate>,
        selected: usize,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        // The panel is already a deferred layer, so the dropdown is an
        // absolutely positioned sibling rather than another deferred draw.
        div()
            .absolute()
            .top_full()
            .left_0()
            .child(
                div()
                    .id("participant-dropdown")
                    .occlude()
                    .mt(px(4.0))
                    .w(px(256.0))
                    .max_h(px(200.0))
                    .overflow_hidden()
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.popover)
                    .shadow_md()
                    .py_1()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .children(
                        candidates
                            .into_iter()
                            .enumerate()
                            .map(|(index, candidate)| {
                                let highlighted = index == selected;
                                let label: AnyElement = if candidate.is_new {
                                    div()
                                        .flex()
                                        .child("Add \u{201c}")
                                        .child(
                                            div()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .child(SharedString::from(candidate.name.clone())),
                                        )
                                        .child("\u{201d}")
                                        .into_any_element()
                                } else {
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .child(SharedString::from(candidate.name.clone())),
                                        )
                                        .when(!candidate.job_title.is_empty(), |row| {
                                            row.child(
                                                div()
                                                    .tw_text_xs()
                                                    .text_color(theme.muted_foreground)
                                                    .child(SharedString::from(
                                                        candidate.job_title.clone(),
                                                    )),
                                            )
                                        })
                                        .into_any_element()
                                };
                                div()
                                    .id(SharedString::from(format!("candidate-{index}")))
                                    .flex()
                                    .w_full()
                                    .items_center()
                                    .justify_between()
                                    .px_3()
                                    .py(px(6.0))
                                    .tw_text_sm()
                                    .text_color(theme.foreground)
                                    .cursor_pointer()
                                    .when(highlighted, |row| row.bg(theme.muted))
                                    .when(!highlighted, |row| {
                                        row.hover(move |style| style.bg(theme.accent))
                                    })
                                    .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                                        if *hovering && let Some(info) = this.meeting_info.as_mut()
                                        {
                                            info.selected_index = index;
                                            cx.notify();
                                        }
                                    }))
                                    .on_click(cx.listener(
                                        move |this, _: &ClickEvent, window, cx| {
                                            if let Some(info) = this.meeting_info.as_mut() {
                                                info.selected_index = index;
                                            }
                                            this.pick_meeting_participant(window, cx);
                                        },
                                    ))
                                    .child(label)
                                    .when(highlighted, |row| {
                                        row.child(icon(
                                            "arrow-elbow-down-left",
                                            px(12.0),
                                            theme.muted_foreground,
                                        ))
                                    })
                            }),
                    ),
            )
            .into_any_element()
    }
}

/// `safeFormat(date, "MMM d, yyyy h:mm a")`
fn format_medium<Tz: TimeZone>(date: &chrono::DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    format!(
        "{} {}, {} {}",
        date.format("%b"),
        date.day(),
        date.year(),
        date.format("%-I:%M %p")
    )
}

/// `toIsoString` for a `datetime-local` value: local wall time to an ISO
/// instant in UTC (`new Date(value).toISOString()`).
fn datetime_local_to_iso(value: &str) -> Option<String> {
    let naive = chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S"))
        .ok()?;
    let local = chrono::Local.from_local_datetime(&naive).earliest()?;
    Some(
        local
            .with_timezone(&chrono::Utc)
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datetime_local_values_round_trip_to_iso() {
        let iso = datetime_local_to_iso("2026-09-05T14:30").expect("valid");
        assert!(iso.ends_with('Z'));
        assert_eq!(iso.len(), "2026-09-05T14:30:00.000Z".len());
        assert!(datetime_local_to_iso("not a date").is_none());
        assert!(datetime_local_to_iso("").is_none());
    }

    #[test]
    fn medium_format_matches_date_fns() {
        let date = chrono::Utc.with_ymd_and_hms(2026, 3, 7, 9, 5, 0).unwrap();
        assert_eq!(format_medium(&date), "Mar 7, 2026 9:05 AM");
    }
}
