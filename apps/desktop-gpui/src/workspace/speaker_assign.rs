//! `SpeakerAssignPopover` / `SpeakerParticipantPicker`
//! (`transcript/renderer/speaker-assign.tsx`): the popover a segment's
//! speaker label opens to pick who spoke, with the participants-first
//! option groups, the create option, and the apply-to-all scope.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anlg_transcript::SegmentKey;
use gpui::{
    AnyElement, ClickEvent, Context, Entity, Focusable as _, MouseButton, MouseDownEvent,
    RenderImage, SharedString, Window, div, prelude::*, px, relative,
};

use super::Workspace;
use crate::db::{SessionParticipant, SpeakerTarget};
use crate::speaker_assignment::Mode;
use crate::text_input::{TextInput, TextInputEvent, TextInputStyle};
use crate::theme::alpha;
use crate::transcript::Segment;
use crate::ui::{TailwindText as _, icon};

/// `SpeakerParticipantOption`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeakerOption {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
    pub avatar_data_url: Option<String>,
    pub is_session_participant: bool,
    pub is_new: bool,
    pub is_create_option: bool,
}

impl SpeakerOption {
    fn existing(id: &str, name: &str, email: Option<&str>, avatar: Option<&str>) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            email: email.map(str::to_string),
            avatar_data_url: avatar.map(str::to_string),
            is_session_participant: false,
            is_new: false,
            is_create_option: false,
        }
    }

    /// `ContactFacehash name={option.name || option.email || option.id}`
    fn facehash_seed(&self) -> &str {
        if !self.name.is_empty() {
            &self.name
        } else if let Some(email) = self.email.as_deref().filter(|e| !e.is_empty()) {
            email
        } else {
            &self.id
        }
    }
}

/// One titled option group of the picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub title: &'static str,
    pub options: Vec<SpeakerOption>,
}

/// `getSpeakerParticipantDedupeKeys`
fn dedupe_keys(option: &SpeakerOption) -> Vec<String> {
    let mut keys = vec![format!("id:{}", option.id)];
    if let Some(email) = &option.email {
        keys.push(format!("email:{}", email.to_lowercase()));
    }
    keys
}

fn matches_query(option: &SpeakerOption, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    option.name.to_lowercase().contains(query)
        || option
            .email
            .as_deref()
            .unwrap_or("")
            .to_lowercase()
            .contains(query)
}

/// `buildSpeakerParticipantGroups`: session and event participants first
/// (deduplicated by id / email), then the contacts not among them.
pub fn build_groups(
    session_participants: &[SpeakerOption],
    event_participants: &[SpeakerOption],
    contacts: &[SpeakerOption],
    query: &str,
) -> Vec<Group> {
    let query = query.trim().to_lowercase();
    let mut participant_keys: HashSet<String> = HashSet::new();
    let participants: Vec<SpeakerOption> = session_participants
        .iter()
        .chain(event_participants)
        .filter(|option| {
            let keys = dedupe_keys(option);
            if keys.iter().any(|key| participant_keys.contains(key)) {
                return false;
            }
            participant_keys.extend(keys);
            true
        })
        .filter(|option| matches_query(option, &query))
        .cloned()
        .collect();
    let people: Vec<SpeakerOption> = contacts
        .iter()
        .filter(|option| {
            dedupe_keys(option)
                .iter()
                .all(|key| !participant_keys.contains(key))
        })
        .filter(|option| matches_query(option, &query))
        .cloned()
        .collect();
    let mut groups = Vec::new();
    if !participants.is_empty() {
        groups.push(Group {
            title: "Participants",
            options: participants,
        });
    }
    if !people.is_empty() {
        groups.push(Group {
            title: "People",
            options: people,
        });
    }
    groups
}

/// `buildCreateSpeakerParticipantOption`: an `Add "<query>"` option unless
/// the typed name or email already names an option.
pub fn build_create_option(query: &str, existing: &[SpeakerOption]) -> Option<SpeakerOption> {
    let name = query.trim();
    if name.is_empty() {
        return None;
    }
    let normalized = name.to_lowercase();
    let exists = existing.iter().any(|option| {
        option.name.to_lowercase() == normalized
            || option
                .email
                .as_deref()
                .is_some_and(|email| email.to_lowercase() == normalized)
    });
    if exists {
        return None;
    }
    Some(SpeakerOption {
        id: "new".into(),
        name: name.to_string(),
        email: None,
        avatar_data_url: None,
        is_session_participant: false,
        is_new: true,
        is_create_option: true,
    })
}

/// `buildEventSpeakerParticipantOptions`: attendees matched to contacts by
/// email (or by name when they have no email), the rest pending.
pub fn build_event_options(
    event_participants: &[(String, String)],
    contacts: &[SpeakerOption],
) -> Vec<SpeakerOption> {
    let by_email: HashMap<String, &SpeakerOption> = contacts
        .iter()
        .filter_map(|contact| {
            contact
                .email
                .as_deref()
                .filter(|email| !email.is_empty())
                .map(|email| (email.to_lowercase(), contact))
        })
        .collect();
    let by_name: HashMap<String, &SpeakerOption> = contacts
        .iter()
        .map(|contact| (contact.name.to_lowercase(), contact))
        .collect();
    event_participants
        .iter()
        .enumerate()
        .filter_map(|(index, (name, email))| {
            let name = name.trim();
            let email = email.trim();
            if name.is_empty() && email.is_empty() {
                return None;
            }
            let contact = if !email.is_empty() {
                by_email.get(&email.to_lowercase())
            } else {
                by_name.get(&name.to_lowercase())
            };
            if let Some(contact) = contact {
                let mut option = (*contact).clone();
                if !name.is_empty() {
                    option.name = name.to_string();
                }
                if !email.is_empty() {
                    option.email = Some(email.to_string());
                }
                option.is_session_participant = true;
                return Some(option);
            }
            Some(SpeakerOption {
                id: if email.is_empty() {
                    format!("event:{name}:{index}")
                } else {
                    format!("event:{email}")
                },
                name: if name.is_empty() {
                    email.to_string()
                } else {
                    name.to_string()
                },
                email: (!email.is_empty()).then(|| email.to_string()),
                avatar_data_url: None,
                is_session_participant: true,
                is_new: true,
                is_create_option: false,
            })
        })
        .collect()
}

/// `getAssignmentAnchorWordId`
pub fn anchor_word_id(segment: &Segment) -> Option<String> {
    segment
        .words
        .iter()
        .find_map(|word| word.id.clone().filter(|id| !id.is_empty()))
}

/// `getAssignmentWordIds`
pub fn assignment_word_ids(segment: &Segment) -> Vec<String> {
    segment
        .words
        .iter()
        .filter_map(|word| word.id.clone().filter(|id| !id.is_empty()))
        .collect()
}

/// One `TranscriptWordSelectionGroup`: the words of a selected section.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SelectionGroup {
    pub transcript_id: String,
    pub segment_key: SegmentKey,
    pub word_ids: Vec<String>,
}

/// What the picked human is assigned to.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum AssignTarget {
    /// The label popover: one segment, `all` or `segment` scope.
    Segment {
        transcript_id: String,
        segment_id: String,
        segment_key: SegmentKey,
        word_ids: Vec<String>,
        anchor_word_id: String,
    },
    /// The selection bar's `Change speaker`: every selected section's words.
    Selection { groups: Vec<SelectionGroup> },
}

/// The open speaker picker.
pub(super) struct SpeakerAssign {
    pub session_id: String,
    pub target: AssignTarget,
    /// `showAssignmentScope`: the label popover offers `Apply to all`.
    show_scope: bool,
    query: Entity<TextInput>,
    selected: Option<String>,
    /// `applyToAllMatching`
    apply_to_all: bool,
    assigning: bool,
    session_participants: Vec<SessionParticipant>,
    contacts: Vec<crate::contacts::Human>,
    event_participants: Vec<(String, String)>,
    avatars: HashMap<String, Arc<RenderImage>>,
    photos: HashMap<String, Option<Arc<RenderImage>>>,
}

impl SpeakerAssign {
    /// The segment whose label opened this picker, if any.
    pub fn segment_id(&self) -> Option<&str> {
        match &self.target {
            AssignTarget::Segment { segment_id, .. } => Some(segment_id),
            AssignTarget::Selection { .. } => None,
        }
    }

    pub fn for_selection(&self) -> bool {
        matches!(self.target, AssignTarget::Selection { .. })
    }

    fn contact_options(&self) -> Vec<SpeakerOption> {
        self.contacts
            .iter()
            .filter_map(|human| {
                let name = human.name.trim();
                let email = human.email.trim();
                if name.is_empty() && email.is_empty() {
                    return None;
                }
                Some(SpeakerOption::existing(
                    &human.id,
                    if name.is_empty() { email } else { name },
                    (!email.is_empty()).then_some(email),
                    human.avatar_data_url.as_deref(),
                ))
            })
            .collect()
    }

    fn participant_options(&self) -> Vec<SpeakerOption> {
        let avatar_by_id: HashMap<&str, Option<&str>> = self
            .contacts
            .iter()
            .map(|human| (human.id.as_str(), human.avatar_data_url.as_deref()))
            .collect();
        self.session_participants
            .iter()
            .filter(|participant| !participant.human_id.is_empty())
            .map(|participant| {
                let name = participant.name.trim();
                let email = participant.email.trim();
                let mut option = SpeakerOption::existing(
                    &participant.human_id,
                    if !name.is_empty() {
                        name
                    } else if !email.is_empty() {
                        email
                    } else {
                        "Unknown"
                    },
                    (!email.is_empty()).then_some(email),
                    avatar_by_id
                        .get(participant.human_id.as_str())
                        .copied()
                        .flatten(),
                );
                option.is_session_participant = true;
                option
            })
            .collect()
    }

    /// The groups, the create option, and whether a "People" group exists.
    fn options(&self, query: &str) -> (Vec<Group>, Option<SpeakerOption>) {
        let participants = self.participant_options();
        let contacts = self.contact_options();
        let events = build_event_options(&self.event_participants, &contacts);
        let groups = build_groups(&participants, &events, &contacts, query);
        let existing: Vec<SpeakerOption> = participants
            .iter()
            .chain(&events)
            .chain(&contacts)
            .cloned()
            .collect();
        (groups, build_create_option(query, &existing))
    }

    fn option_by_id(&self, query: &str, id: &str) -> Option<SpeakerOption> {
        let (groups, create) = self.options(query);
        groups
            .into_iter()
            .flat_map(|group| group.options)
            .chain(create)
            .find(|option| option.id == id)
    }
}

const POPOVER_WIDTH: f32 = 320.0;

impl Workspace {
    pub(crate) fn close_speaker_assign(&mut self, cx: &mut Context<Self>) -> bool {
        if self.transcript_view.speaker_assign.take().is_some() {
            cx.notify();
            return true;
        }
        false
    }

    /// The label click: open the popover for `segment`, or close it when it
    /// is the open one.
    pub(super) fn toggle_speaker_assign(
        &mut self,
        session_id: String,
        transcript_id: String,
        segment: &Segment,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .transcript_view
            .speaker_assign
            .as_ref()
            .is_some_and(|open| open.segment_id() == Some(segment.id.as_str()))
        {
            self.close_speaker_assign(cx);
            return;
        }
        let Some(anchor) = anchor_word_id(segment) else {
            return;
        };
        self.open_speaker_picker(
            session_id,
            AssignTarget::Segment {
                transcript_id,
                segment_id: segment.id.clone(),
                segment_key: segment.key.clone(),
                word_ids: assignment_word_ids(segment),
                anchor_word_id: anchor,
            },
            true,
            window,
            cx,
        );
    }

    /// Open the picker for `target` and load the people it lists.
    pub(super) fn open_speaker_picker(
        &mut self,
        session_id: String,
        target: AssignTarget,
        show_scope: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let style = TextInputStyle {
            text: self.theme.foreground,
            placeholder: self.theme.muted_foreground,
            selection: self.theme.selection,
            underline_when_focused: false,
            masked: false,
        };
        let query = cx.new(|cx| TextInput::new("Select or type to add speaker", style, window, cx));
        cx.subscribe_in(&query, window, |this, _, event: &TextInputEvent, _, cx| {
            match event {
                // Typing clears the highlighted option.
                TextInputEvent::Changed => {
                    if let Some(open) = this.transcript_view.speaker_assign.as_mut() {
                        open.selected = None;
                        cx.notify();
                    }
                }
                TextInputEvent::Escape => {
                    this.close_speaker_assign(cx);
                }
                _ => {}
            }
        })
        .detach();
        query.read(cx).focus_handle(cx).focus(window);
        let generation = self.transcript_view.picker_generation.wrapping_add(1);
        self.transcript_view.picker_generation = generation;
        self.transcript_view.speaker_assign = Some(SpeakerAssign {
            session_id: session_id.clone(),
            target,
            show_scope,
            query,
            selected: None,
            apply_to_all: true,
            assigning: false,
            session_participants: Vec::new(),
            contacts: Vec::new(),
            event_participants: Vec::new(),
            avatars: HashMap::new(),
            photos: HashMap::new(),
        });
        cx.notify();

        let participants = self.store.list_session_participants(session_id.clone());
        let contacts = self.store.list_contacts();
        let events = self.store.session_event_participants(session_id);
        cx.spawn(async move |this, cx| {
            let participants = participants.await.ok().and_then(Result::ok);
            let contacts = contacts.await.ok().and_then(Result::ok);
            let events = events.await.ok().and_then(Result::ok);
            this.update(cx, |this, cx| {
                if this.transcript_view.picker_generation != generation {
                    return;
                }
                let Some(open) = this.transcript_view.speaker_assign.as_mut() else {
                    return;
                };
                if let Some(participants) = participants {
                    open.session_participants = participants;
                }
                if let Some((humans, _)) = contacts {
                    open.contacts = humans;
                }
                if let Some(events) = events {
                    open.event_participants = events;
                }
                // Rasterise every avatar the picker can show, once.
                let (groups, _) = open.options("");
                for option in groups.iter().flat_map(|group| &group.options) {
                    match &option.avatar_data_url {
                        Some(data_url) => {
                            open.photos
                                .entry(data_url.clone())
                                .or_insert_with(|| super::contacts_tab::decode_photo(data_url));
                        }
                        None => {
                            let seed = option.facehash_seed().to_string();
                            open.avatars
                                .entry(seed.clone())
                                .or_insert_with(|| super::contacts_tab::rasterize_avatar(&seed));
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `handleConfirm`: resolve the human, link it to the session, assign
    /// (`all` across the session's transcripts, this segment's words, or each
    /// selected section's words), then close and reload the note.
    fn confirm_speaker_assign(&mut self, cx: &mut Context<Self>) {
        let Some(open) = self.transcript_view.speaker_assign.as_mut() else {
            return;
        };
        if open.assigning {
            return;
        }
        let query = open.query.read(cx).text().to_string();
        let Some(option) = open
            .selected
            .clone()
            .and_then(|id| open.option_by_id(&query, &id))
        else {
            return;
        };
        open.assigning = true;
        cx.notify();
        let target = if option.is_new {
            SpeakerTarget::New {
                name: option.name.clone(),
                email: option.email.clone().unwrap_or_default(),
            }
        } else {
            SpeakerTarget::Existing(option.id.clone())
        };
        let session_id = open.session_id.clone();
        let assign_target = open.target.clone();
        let apply_to_all = open.show_scope && open.apply_to_all;
        let generation = self.transcript_view.picker_generation;
        let store = self.store.clone();
        let prepare = store.prepare_speaker(session_id.clone(), target);
        cx.spawn(async move |this, cx| {
            let result: anyhow::Result<()> = async {
                let human_id = prepare.await??;
                match assign_target {
                    AssignTarget::Segment {
                        transcript_id,
                        segment_key,
                        word_ids,
                        anchor_word_id,
                        ..
                    } => {
                        if apply_to_all {
                            store
                                .assign_session_transcript_speaker(
                                    session_id.clone(),
                                    transcript_id,
                                    segment_key,
                                    human_id,
                                    anchor_word_id,
                                )
                                .await??;
                        } else {
                            store
                                .assign_transcript_speaker(
                                    transcript_id,
                                    segment_key,
                                    human_id,
                                    Some(anchor_word_id),
                                    Mode::Segment { word_ids },
                                )
                                .await??;
                        }
                    }
                    AssignTarget::Selection { groups } => {
                        for group in groups {
                            let Some(anchor) = group.word_ids.first().cloned() else {
                                continue;
                            };
                            store
                                .assign_transcript_speaker(
                                    group.transcript_id,
                                    group.segment_key,
                                    human_id.clone(),
                                    Some(anchor),
                                    Mode::Segment {
                                        word_ids: group.word_ids,
                                    },
                                )
                                .await??;
                        }
                    }
                }
                Ok(())
            }
            .await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        let for_selection = this
                            .transcript_view
                            .speaker_assign
                            .as_ref()
                            .is_some_and(|open| open.for_selection());
                        if this.transcript_view.picker_generation == generation {
                            this.transcript_view.speaker_assign = None;
                        }
                        // The bar's and the menu's `handleAssign` also clear
                        // the selection.
                        if for_selection {
                            this.clear_transcript_selection(cx);
                            this.clear_text_selection(cx);
                        }
                        if this.selected.as_deref() == Some(session_id.as_str()) {
                            this.reload_note(session_id, cx);
                        }
                    }
                    Err(error) => {
                        tracing::error!(%error, "[transcript] failed to assign speaker");
                        if let Some(open) = this.transcript_view.speaker_assign.as_mut() {
                            open.assigning = false;
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `<PopoverContent variant="app" side="right" align="start"
    /// sideOffset={8} collisionPadding={16} className="w-80">`, rendered
    /// inside the segment's label so it anchors to the trigger's top-right.
    pub(super) fn render_speaker_assign_popover(
        &self,
        segment_id: &str,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let open = self.transcript_view.speaker_assign.as_ref()?;
        if open.segment_id() != Some(segment_id) {
            return None;
        }
        Some(
            div()
                .absolute()
                .top_0()
                .left(relative(1.0))
                .ml(px(8.0))
                .child(
                    gpui::deferred(
                        gpui::anchored()
                            .anchor(gpui::Corner::TopLeft)
                            .snap_to_window_with_margin(px(16.0))
                            .child(self.render_speaker_picker(open, cx)),
                    )
                    .with_priority(3),
                )
                .into_any_element(),
        )
    }

    /// `<PopoverContent variant="app" side="top" align="center" sideOffset={8}
    /// className="w-80">` over the selection bar's `Change speaker` button.
    pub(super) fn render_selection_speaker_popover(
        &self,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let open = self.transcript_view.speaker_assign.as_ref()?;
        if !open.for_selection() {
            return None;
        }
        Some(
            div()
                .absolute()
                .bottom(relative(1.0))
                .left(relative(0.5))
                .mb(px(8.0))
                .ml(px(-POPOVER_WIDTH / 2.0))
                .child(
                    gpui::deferred(
                        gpui::anchored()
                            .anchor(gpui::Corner::BottomLeft)
                            .snap_to_window_with_margin(px(16.0))
                            .child(self.render_speaker_picker(open, cx)),
                    )
                    .with_priority(3),
                )
                .into_any_element(),
        )
    }

    /// `SpeakerParticipantPicker` in its `variant="app"` chrome.
    fn render_speaker_picker(
        &self,
        open: &SpeakerAssign,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        super::menu::menu_chrome(self.theme, "speaker-assign", POPOVER_WIDTH)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                this.close_speaker_assign(cx);
            }))
            .child(self.render_speaker_picker_body(open, cx))
    }

    /// `SpeakerParticipantPicker` itself: the `max-h-[min(available, 28rem)]`
    /// column of the search-and-list panel over the footer.
    pub(super) fn render_speaker_picker_body(
        &self,
        open: &SpeakerAssign,
        cx: &Context<Self>,
    ) -> gpui::Div {
        let theme = self.theme;
        let query = open.query.read(cx).text().trim().to_string();
        let (groups, create) = open.options(&query);
        let has_people_group = groups.iter().any(|group| group.title == "People");
        let can_confirm = open.selected.is_some() && !open.assigning;

        let mut list = div().py_1().pb_3();
        for group in &groups {
            let mut section = div().child(self.speaker_group_title(group.title));
            for option in &group.options {
                section = section.child(self.render_speaker_option(open, option, cx));
            }
            list = list.child(section);
        }
        if let Some(create) = &create {
            let mut section = div();
            if !has_people_group {
                section = section.child(self.speaker_group_title("People"));
            }
            list = list.child(section.child(self.render_speaker_option(open, create, cx)));
        }
        if create.is_none() && groups.is_empty() {
            list = list.child(
                div()
                    .px_3()
                    .py_2()
                    .tw_text_xs()
                    .text_color(theme.muted_foreground)
                    .child(if query.is_empty() {
                        "No people"
                    } else {
                        "No matching people"
                    }),
            );
        }
        if query.is_empty() {
            // `Create new speaker` focuses the search field.
            list = list.child(
                div()
                    .id("speaker-create-new")
                    .flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py(px(6.0))
                    .tw_text_sm()
                    .cursor_pointer()
                    .hover(move |style| style.bg(theme.accent))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        if let Some(open) = this.transcript_view.speaker_assign.as_ref() {
                            open.query.read(cx).focus_handle(cx).focus(window);
                        }
                    }))
                    .child(icon("plus", px(16.0), theme.foreground))
                    .child("Create new speaker"),
            );
        }

        // `AppFloatingPanel`: the search row over the scrolling list.
        let panel = div()
            .relative()
            .flex()
            .min_h_0()
            .flex_1()
            .flex_col()
            .overflow_hidden()
            .rounded(px(crate::squircle::PANEL_RADIUS))
            .child(crate::squircle::squircle(
                crate::squircle::PANEL_RADIUS,
                Some(theme.floating_panel),
                Some((1.0, theme.floating_border)),
            ))
            .child(
                div()
                    .relative()
                    .border_b_1()
                    .border_color(theme.border)
                    .py_1()
                    .child(
                        div()
                            .flex()
                            .h(px(32.0))
                            .items_center()
                            .gap_2()
                            .px_3()
                            .child(icon("search", px(16.0), theme.muted_foreground))
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .tw_text_sm()
                                    .child(open.query.clone()),
                            ),
                    ),
            )
            .child(
                div()
                    .id("speaker-options")
                    .relative()
                    .min_h_0()
                    .flex_1()
                    .overflow_y_scroll()
                    .child(list),
            );

        // Footer: the `Apply to all` checkbox and the Confirm button.
        let apply_to_all = open.apply_to_all;
        let footer = div()
            .flex()
            .items_center()
            .justify_end()
            .gap_3()
            .py_1()
            .pl_2()
            .when(open.show_scope, |footer| {
                footer.child(
                    div()
                        .id("speaker-apply-to-all")
                        .flex()
                        .min_w_0()
                        .flex_1()
                        .items_center()
                        .gap_2()
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(open) = this.transcript_view.speaker_assign.as_mut() {
                                open.apply_to_all = !open.apply_to_all;
                                cx.notify();
                            }
                        }))
                        .child(
                            // `Checkbox`: `h-4 w-4 rounded-xs border border-primary
                            // shadow-xs`, filled with the check when checked.
                            div()
                                .flex()
                                .size(px(16.0))
                                .flex_shrink_0()
                                .items_center()
                                .justify_center()
                                .rounded(px(2.0))
                                .border_1()
                                .border_color(theme.primary)
                                .shadow(vec![gpui::BoxShadow {
                                    color: gpui::hsla(0.0, 0.0, 0.0, 0.05),
                                    offset: gpui::point(px(0.0), px(1.0)),
                                    blur_radius: px(2.0),
                                    spread_radius: px(0.0),
                                }])
                                .when(apply_to_all, |box_| {
                                    box_.bg(theme.primary).child(icon(
                                        "check",
                                        px(16.0),
                                        theme.primary_foreground,
                                    ))
                                }),
                        )
                        .child(
                            div()
                                .tw_text_sm()
                                .text_color(theme.muted_foreground)
                                .whitespace_nowrap()
                                .child("Apply to all"),
                        ),
                )
            })
            .child(
                // `bg-primary h-8 rounded-full px-3 text-xs font-medium
                // hover:bg-primary/90 disabled:opacity-50`
                div()
                    .id("speaker-confirm")
                    .relative()
                    .flex()
                    .h(px(32.0))
                    .items_center()
                    .px_3()
                    .text_color(theme.primary_foreground)
                    .tw_text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .when(!can_confirm, |button| button.opacity(0.5))
                    .when(can_confirm, |button| {
                        button.cursor_pointer().on_click(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                this.confirm_speaker_assign(cx);
                            },
                        ))
                    })
                    .child(crate::squircle::squircle(
                        crate::squircle::CONTROL_RADIUS,
                        Some(
                            if can_confirm && self.transcript_view.speaker_confirm_hovered {
                                alpha(theme.primary, 0.9)
                            } else {
                                theme.primary
                            },
                        ),
                        None,
                    ))
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        if this.transcript_view.speaker_confirm_hovered != *hovered {
                            this.transcript_view.speaker_confirm_hovered = *hovered;
                            cx.notify();
                        }
                    }))
                    .child(div().relative().child("Confirm")),
            );

        div()
            .flex()
            .max_h(px(448.0))
            .flex_col()
            .gap_1()
            .overflow_hidden()
            .child(panel)
            .child(footer)
    }

    /// `text-muted-foreground px-3 pt-2 pb-1 text-[11px] font-medium uppercase`
    fn speaker_group_title(&self, title: &'static str) -> AnyElement {
        div()
            .px_3()
            .pt_2()
            .pb_1()
            .text_size(px(11.0))
            .line_height(px(16.0))
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(self.theme.muted_foreground)
            .child(title.to_uppercase())
            .into_any_element()
    }

    /// `ParticipantOptionButton`: `flex w-full items-center gap-2 px-3 py-1.5
    /// text-left text-sm`, `bg-accent` while selected, the 28px avatar (or
    /// the `size-7 rounded-full border bg-muted` plus for the create
    /// option), the name and the muted `text-xs` email.
    fn render_speaker_option(
        &self,
        open: &SpeakerAssign,
        option: &SpeakerOption,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let selected = open.selected.as_deref() == Some(option.id.as_str());
        let avatar = if option.is_create_option {
            div()
                .flex()
                .size(px(28.0))
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .rounded(px(crate::squircle::CONTROL_RADIUS))
                .border_1()
                .border_color(theme.border)
                .bg(theme.muted)
                .child(icon("plus", px(14.0), theme.muted_foreground))
                .into_any_element()
        } else {
            let photo = option
                .avatar_data_url
                .as_ref()
                .and_then(|data_url| open.photos.get(data_url).cloned().flatten());
            match photo {
                Some(image) => Self::render_avatar_image(Some(image), option.facehash_seed(), 28.0),
                None => Self::render_avatar_image(
                    open.avatars.get(option.facehash_seed()).cloned(),
                    option.facehash_seed(),
                    28.0,
                ),
            }
        };
        let label = if option.is_create_option {
            format!("Add \"{}\"", option.name)
        } else {
            option.name.clone()
        };
        let id = option.id.clone();
        div()
            .id(SharedString::from(format!("speaker-option-{}", option.id)))
            .flex()
            .w_full()
            .items_center()
            .gap_2()
            .px_3()
            .py(px(6.0))
            .tw_text_sm()
            .cursor_pointer()
            .when(selected, |row| row.bg(theme.accent))
            .when(!selected, |row| {
                row.hover(move |style| style.bg(theme.accent))
            })
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if let Some(open) = this.transcript_view.speaker_assign.as_mut() {
                    open.selected = Some(id.clone());
                    cx.notify();
                }
            }))
            .child(avatar)
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .truncate()
                            .child(SharedString::from(label)),
                    )
                    .when_some(option.email.clone(), |column, email| {
                        column.child(
                            div()
                                .tw_text_xs()
                                .text_color(theme.muted_foreground)
                                .truncate()
                                .child(SharedString::from(email)),
                        )
                    }),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn option(id: &str, name: &str) -> SpeakerOption {
        SpeakerOption::existing(id, name, None, None)
    }

    fn with_email(mut option: SpeakerOption, email: &str) -> SpeakerOption {
        option.email = Some(email.to_string());
        option
    }

    fn participant(mut option: SpeakerOption) -> SpeakerOption {
        option.is_session_participant = true;
        option
    }

    #[test]
    fn falls_back_to_contacts_when_a_transcript_has_no_session_participants() {
        let groups = build_groups(&[], &[], &[option("human-1", "Alice")], "");
        assert_eq!(
            groups,
            vec![Group {
                title: "People",
                options: vec![option("human-1", "Alice")],
            }]
        );
    }

    #[test]
    fn keeps_participants_first_and_excludes_duplicate_people() {
        let alice = participant(option("human-1", "Alice"));
        let carol = participant(option("human-3", "Carol"));
        let groups = build_groups(
            std::slice::from_ref(&alice),
            std::slice::from_ref(&carol),
            &[option("human-1", "Alice"), option("human-2", "Bob")],
            "",
        );
        assert_eq!(
            groups,
            vec![
                Group {
                    title: "Participants",
                    options: vec![alice, carol],
                },
                Group {
                    title: "People",
                    options: vec![option("human-2", "Bob")],
                },
            ]
        );
    }

    #[test]
    fn matches_event_participants_to_existing_people_by_email() {
        let contact = with_email(option("human-1", "Alice"), "alice@example.com");
        let options = build_event_options(
            &[("Alice A.".into(), "alice@example.com".into())],
            std::slice::from_ref(&contact),
        );
        assert_eq!(
            options,
            vec![participant(with_email(
                option("human-1", "Alice A."),
                "alice@example.com"
            ))]
        );
    }

    #[test]
    fn creates_pending_participant_options_for_event_attendees_without_people() {
        let options = build_event_options(&[("Bob".into(), "bob@example.com".into())], &[]);
        let mut expected = participant(with_email(
            option("event:bob@example.com", "Bob"),
            "bob@example.com",
        ));
        expected.is_new = true;
        assert_eq!(options, vec![expected]);
    }

    #[test]
    fn does_not_match_event_attendees_by_name_when_their_email_differs() {
        let options = build_event_options(
            &[("Bob".into(), "bob@example.com".into())],
            &[with_email(option("human-1", "Bob"), "other@example.com")],
        );
        assert_eq!(options[0].id, "event:bob@example.com");
        assert!(options[0].is_new);
    }

    #[test]
    fn keeps_duplicate_event_attendees_without_emails_selectable() {
        let options = build_event_options(
            &[("Bob".into(), String::new()), ("Bob".into(), String::new())],
            &[],
        );
        let ids: Vec<&str> = options.iter().map(|option| option.id.as_str()).collect();
        assert_eq!(ids, ["event:Bob:0", "event:Bob:1"]);
        assert!(
            options
                .iter()
                .all(|option| option.is_new && option.is_session_participant)
        );
    }

    #[test]
    fn creates_an_add_option_for_a_new_typed_contact_name() {
        let created = build_create_option("  Charlie  ", &[option("human-1", "Alice")]).unwrap();
        assert_eq!(created.id, "new");
        assert_eq!(created.name, "Charlie");
        assert!(created.is_new && created.is_create_option && !created.is_session_participant);
    }

    #[test]
    fn does_not_create_a_duplicate_add_option() {
        assert!(
            build_create_option(
                "alice@example.com",
                &[with_email(option("human-1", "Alice"), "alice@example.com")],
            )
            .is_none()
        );
        assert!(build_create_option("   ", &[]).is_none());
    }
}
