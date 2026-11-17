//! Shipping's `SpeakerAssignPopover` and `SpeakerParticipantPicker`: choose or add the person
//! who spoke a segment, for that segment or (Apply to all) every segment of that speaker.

use std::collections::HashMap;
use std::sync::Arc;

use anlg_transcript::{ChannelProfile, SegmentKey};
use desktop_runtime::CancellationToken;
use gpui::{
    AnyElement, Context, Entity, Focusable, FontWeight, MouseButton, SharedString, Subscription,
    Window, div, prelude::*, px, svg,
};
use serde_json::{Value, json};

use super::{Row, TranscriptView};
use crate::{
    meeting::{
        model::failure,
        store::{SpeakerAssignment, SpeakerScope, TranscriptStore},
    },
    ui::{
        input::TextInput,
        theme::{RADIUS, Theme},
    },
    workspace::facehash,
};

/// The segment a speaker is chosen for.
#[derive(Clone)]
pub(super) struct Target {
    segment: String,
    transcript: Arc<str>,
    key: SegmentKey,
    word_ids: Vec<String>,
}

impl Target {
    pub(super) fn new(row: &Row, words: &HashMap<String, Arc<str>>) -> Option<Self> {
        let word_ids: Vec<String> = row
            .words
            .iter()
            .filter_map(|word| word.id.clone())
            .collect();
        Some(Self {
            segment: row.id.clone(),
            transcript: words.get(word_ids.first()?)?.clone(),
            key: row.key.clone(),
            word_ids,
        })
    }

    /// Shipping's `mergeTranscriptSelections`: the selected segments' stored words, one target
    /// per transcript, keyed by the first segment in it.
    pub(super) fn selection(rows: &[&Row], words: &HashMap<String, Arc<str>>) -> Vec<Self> {
        let mut targets: Vec<Self> = Vec::new();
        for row in rows {
            for id in row.words.iter().filter_map(|word| word.id.as_ref()) {
                let Some(transcript) = words.get(id) else {
                    continue;
                };
                match targets
                    .iter_mut()
                    .find(|target| &target.transcript == transcript)
                {
                    Some(target) => target.word_ids.push(id.clone()),
                    None => targets.push(Self {
                        segment: row.id.clone(),
                        transcript: transcript.clone(),
                        key: row.key.clone(),
                        word_ids: vec![id.clone()],
                    }),
                }
            }
        }
        targets
    }

    pub(super) fn transcript(&self) -> &Arc<str> {
        &self.transcript
    }

    pub(super) fn word_ids(&self) -> &[String] {
        &self.word_ids
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Person {
    id: String,
    name: String,
    email: String,
}

#[derive(Clone, Debug, PartialEq)]
enum Choice {
    Person(String),
    New(String),
}

pub(super) struct SpeakerPicker {
    /// The segment whose speaker label opened the picker; `None` when the selection bar did.
    pub(super) segment: Option<String>,
    /// One per transcript the selection spans.
    targets: Vec<Target>,
    search: Entity<TextInput>,
    people: Option<(Vec<Person>, Vec<Person>)>,
    selected: Option<Choice>,
    apply_all: bool,
    assigning: bool,
    _search: Subscription,
}

/// Shipping's `buildSpeakerParticipantGroups` and `buildCreateSpeakerParticipantOption`:
/// participants first, then other people, both filtered by name or email; a new name can be
/// added when nobody matches it exactly.
fn groups(
    participants: &[Person],
    contacts: &[Person],
    query: &str,
) -> (Vec<Person>, Vec<Person>, Option<String>) {
    let query = query.trim();
    let lower = query.to_lowercase();
    let matches = |person: &Person| {
        lower.is_empty()
            || person.name.to_lowercase().contains(&lower)
            || person.email.to_lowercase().contains(&lower)
    };
    let keys = |person: &Person| {
        let mut keys = vec![format!("id:{}", person.id)];
        if !person.email.is_empty() {
            keys.push(format!("email:{}", person.email.to_lowercase()));
        }
        keys
    };
    let mut seen = Vec::new();
    let mut listed = Vec::new();
    for person in participants {
        let person_keys = keys(person);
        if person_keys.iter().any(|key| seen.contains(key)) {
            continue;
        }
        seen.extend(person_keys);
        listed.push(person.clone());
    }
    let others: Vec<Person> = contacts
        .iter()
        .filter(|person| keys(person).iter().all(|key| !seen.contains(key)))
        .filter(|person| matches(person))
        .cloned()
        .collect();
    let exists = participants
        .iter()
        .chain(contacts)
        .any(|person| person.name.to_lowercase() == lower || person.email.to_lowercase() == lower);
    let create = (!query.is_empty() && !exists).then(|| query.to_owned());
    (
        listed
            .into_iter()
            .filter(|person| matches(person))
            .collect(),
        others,
        create,
    )
}

fn channel(key: &SegmentKey) -> i32 {
    match key.channel {
        ChannelProfile::DirectMic => 0,
        ChannelProfile::RemoteParty => 1,
        _ => 2,
    }
}

impl TranscriptView {
    pub(super) fn open_picker(
        &mut self,
        target: Target,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let segment = Some(target.segment.clone());
        self.open_speaker_picker(segment, vec![target], window, cx);
    }

    /// The selection bar's Change speaker: the chosen person speaks every selected word.
    pub(super) fn open_selection_picker(
        &mut self,
        targets: Vec<Target>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !targets.is_empty() {
            self.open_speaker_picker(None, targets, window, cx);
        }
    }

    fn open_speaker_picker(
        &mut self,
        segment: Option<String>,
        targets: Vec<Target>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let search = cx.new(|cx| TextInput::new("Select or type to add speaker", cx).inline());
        let subscription = cx.subscribe(&search, |this, _, _, cx| {
            if let Some(picker) = &mut this.picker {
                picker.selected = None;
            }
            cx.notify();
        });
        search.focus_handle(cx).focus(window, cx);
        self.picker = Some(SpeakerPicker {
            segment,
            targets,
            search,
            people: None,
            selected: None,
            apply_all: true,
            assigning: false,
            _search: subscription,
        });
        let session = self.session.clone();
        let reply = self.runtime.read(CancellationToken::new(), move |services| async move {
            let people = |rows: Vec<Value>| -> Vec<Person> {
                rows.into_iter()
                    .filter_map(|row| {
                        let name = row["name"].as_str().unwrap_or_default().trim().to_owned();
                        let email = row["email"].as_str().unwrap_or_default().trim().to_owned();
                        (!name.is_empty() || !email.is_empty()).then(|| Person {
                            id: row["id"].as_str().unwrap_or_default().to_owned(),
                            name: if name.is_empty() { email.clone() } else { name },
                            email,
                        })
                    })
                    .collect()
            };
            let participants = services.executor.execute(
                "SELECT p.human_id AS id, COALESCE(NULLIF(h.name, ''), p.display_name) AS name, COALESCE(NULLIF(h.email, ''), p.email) AS email
                 FROM session_participants p LEFT JOIN humans h ON h.id = p.human_id AND h.deleted_at IS NULL
                 WHERE p.session_id = ? AND p.deleted_at IS NULL AND p.human_id <> '' AND p.source <> 'excluded'
                 ORDER BY p.created_at, p.id LIMIT 500".into(),
                vec![json!(session)],
            ).await.map_err(failure)?;
            let contacts = services.executor.execute(
                "SELECT id, name, email FROM humans WHERE deleted_at IS NULL ORDER BY name COLLATE NOCASE, id LIMIT 500".into(),
                vec![],
            ).await.map_err(failure)?;
            Ok((people(participants), people(contacts)))
        });
        cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(people) => {
                        if let Some(picker) = &mut this.picker {
                            picker.people = Some(people);
                        }
                    }
                    Err(error) => this.error = Some(format!("People unavailable: {error}")),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn confirm_speaker(&mut self, cx: &mut Context<Self>) {
        let Some(picker) = &mut self.picker else {
            return;
        };
        let Some(choice) = picker.selected.clone() else {
            return;
        };
        let selection = picker.segment.is_none();
        let Some(first) = picker.targets.first().cloned() else {
            return;
        };
        // Apply to all reaches the speaker's other segments; the selection bar never does.
        let all = !selection && picker.apply_all && first.key.speaker_index.is_some();
        let (human_id, new_name) = match choice {
            Choice::Person(id) => (id, None),
            Choice::New(name) => (uuid::Uuid::new_v4().to_string(), Some(name)),
        };
        let assignments: Vec<SpeakerAssignment> = picker
            .targets
            .iter()
            .filter_map(|target| {
                Some(SpeakerAssignment {
                    transcript_id: target.transcript.clone(),
                    anchor: target.word_ids.first()?.clone(),
                    human_id: human_id.clone(),
                    scope: match target.key.speaker_index {
                        Some(speaker_index) if all => SpeakerScope::All {
                            channel: channel(&target.key),
                            speaker_index,
                        },
                        _ => SpeakerScope::Segment {
                            word_ids: target.word_ids.clone(),
                        },
                    },
                })
            })
            .collect();
        if assignments.is_empty() {
            return;
        }
        picker.assigning = true;
        let reply = TranscriptStore(self.runtime.clone()).assign_participants(
            self.session.clone(),
            assignments,
            all.then(|| first.key.speaker_human_id.clone()).flatten(),
            new_name,
        );
        cx.spawn(async move |this, cx| {
            let result = async { reply?.receive().await }.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.picker = None;
                        if selection {
                            this.clear_selection();
                        }
                    }
                    Err(error) => {
                        this.error = Some(format!("Speaker not assigned: {error}"));
                        if let Some(picker) = &mut this.picker {
                            picker.assigning = false;
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// The picker panel, shown to the right of the segment's speaker label.
    pub(super) fn speaker_picker(
        &self,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let picker = self.picker.as_ref()?;
        let query = picker.search.read(cx).buffer.text.clone();
        let (participants, others, create) = picker
            .people
            .as_ref()
            .map(|(participants, contacts)| groups(participants, contacts, &query))
            .unwrap_or_default();
        let header = |title: &'static str| {
            div()
                .px_3()
                .pt_2()
                .pb_1()
                .text_size(px(11.))
                .line_height(px(16.5))
                .font_weight(crate::ui::theme::MEDIUM)
                .text_color(colors.muted_foreground)
                .child(title)
        };
        let option = |id: SharedString,
                      choice: Choice,
                      avatar: AnyElement,
                      name: String,
                      email: String,
                      cx: &mut Context<Self>| {
            let selected = picker.selected.as_ref() == Some(&choice);
            div()
                .id(id)
                .w_full()
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py(px(6.))
                .text_sm()
                .line_height(px(20.))
                .cursor_pointer()
                .when_else(
                    selected,
                    |row| row.bg(colors.accent).text_color(colors.accent_foreground),
                    move |row| row.hover(move |style| style.bg(colors.accent)),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(picker) = &mut this.picker {
                        picker.selected = Some(choice.clone());
                    }
                    cx.notify();
                }))
                .child(avatar)
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .child(
                            div()
                                .truncate()
                                .font_weight(crate::ui::theme::MEDIUM)
                                .child(name),
                        )
                        .when(!email.is_empty(), |view| {
                            view.child(
                                div()
                                    .truncate()
                                    .text_xs()
                                    .line_height(px(16.))
                                    .text_color(colors.muted_foreground)
                                    .child(email),
                            )
                        }),
                )
                .into_any_element()
        };
        let mut rows: Vec<AnyElement> = Vec::new();
        for (title, people) in [("PARTICIPANTS", &participants), ("PEOPLE", &others)] {
            if people.is_empty() {
                continue;
            }
            rows.push(header(title).into_any_element());
            for person in people.iter() {
                rows.push(option(
                    format!("speaker-{}", person.id).into(),
                    Choice::Person(person.id.clone()),
                    facehash(&person.name, 28.).into_any_element(),
                    person.name.clone(),
                    person.email.clone(),
                    cx,
                ));
            }
        }
        if let Some(name) = &create {
            if others.is_empty() {
                rows.push(header("PEOPLE").into_any_element());
            }
            rows.push(option(
                "speaker-new".into(),
                Choice::New(name.clone()),
                div()
                    .size_7()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(RADIUS))
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.muted)
                    .child(
                        svg()
                            .path("editor/PlusIcon.svg")
                            .size(px(14.))
                            .text_color(colors.muted_foreground),
                    )
                    .into_any_element(),
                format!("Add \"{name}\""),
                String::new(),
                cx,
            ));
        }
        if create.is_none()
            && participants.is_empty()
            && others.is_empty()
            && picker.people.is_some()
        {
            rows.push(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child(if query.trim().is_empty() {
                        "No people"
                    } else {
                        "No matching people"
                    })
                    .into_any_element(),
            );
        }
        if query.trim().is_empty() {
            let search = picker.search.focus_handle(cx);
            rows.push(
                div()
                    .id("speaker-create")
                    .w_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py(px(6.))
                    .text_sm()
                    .line_height(px(20.))
                    .cursor_pointer()
                    .hover(move |style| style.bg(colors.accent))
                    .on_click(move |_, window, cx| search.focus(window, cx))
                    .child(
                        svg()
                            .path("editor/PlusIcon.svg")
                            .size_4()
                            .text_color(colors.foreground),
                    )
                    .child("Create new speaker")
                    .into_any_element(),
            );
        }
        let scope = picker.segment.is_some()
            && picker
                .targets
                .first()
                .is_some_and(|target| target.key.speaker_index.is_some());
        let can_confirm = picker.selected.is_some() && !picker.assigning;
        Some(
            div()
                .id("speaker-picker")
                .when(picker.segment.is_some(), |view| view.ml_2())
                .w(px(320.))
                .occlude()
                .p(px(2.))
                .rounded(px(22.))
                .border_1()
                .border_color(colors.app_floating_border)
                .bg(colors.app_floating_chrome)
                .shadow_lg()
                .text_color(colors.popover_foreground)
                .font_weight(FontWeight::NORMAL)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.picker = None;
                    cx.notify();
                }))
                .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                    if event.keystroke.key == "escape" {
                        this.picker = None;
                        cx.stop_propagation();
                        cx.notify();
                    }
                }))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .overflow_hidden()
                                .rounded(px(20.))
                                .border_1()
                                .border_color(colors.app_floating_border)
                                .bg(colors.app_floating_panel)
                                .child(
                                    div().border_b_1().border_color(colors.border).py_1().child(
                                        div()
                                            .h_8()
                                            .px_3()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                svg()
                                                    .path("editor/Search01Icon.svg")
                                                    .size_4()
                                                    .flex_shrink_0()
                                                    .text_color(colors.muted_foreground),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .text_sm()
                                                    .child(picker.search.clone()),
                                            ),
                                    ),
                                )
                                .child(
                                    div()
                                        .id("speaker-options")
                                        .max_h(px(360.))
                                        .overflow_y_scroll()
                                        .py_1()
                                        .pb_3()
                                        .flex()
                                        .flex_col()
                                        .children(rows),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_end()
                                .gap_3()
                                .py_1()
                                .pl_2()
                                .when(scope, |footer| {
                                    let checked = picker.apply_all;
                                    footer.child(
                                        div()
                                            .id("speaker-apply-all")
                                            .flex_1()
                                            .min_w_0()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if let Some(picker) = &mut this.picker {
                                                    picker.apply_all = !picker.apply_all;
                                                }
                                                cx.notify();
                                            }))
                                            .child(
                                                div()
                                                    .size_4()
                                                    .flex_shrink_0()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .rounded(px(4.))
                                                    .border_1()
                                                    .border_color(if checked {
                                                        colors.primary
                                                    } else {
                                                        colors.input
                                                    })
                                                    .when(checked, |box_| {
                                                        box_.bg(colors.primary).child(
                                                            svg()
                                                                .path("editor/CheckIcon.svg")
                                                                .size(px(12.))
                                                                .text_color(
                                                                    colors.primary_foreground,
                                                                ),
                                                        )
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .line_height(px(20.))
                                                    .whitespace_nowrap()
                                                    .text_color(colors.muted_foreground)
                                                    .child("Apply to all"),
                                            ),
                                    )
                                })
                                .child(
                                    div()
                                        .id("speaker-confirm")
                                        .h_8()
                                        .px_3()
                                        .flex()
                                        .items_center()
                                        .rounded(px(RADIUS))
                                        .bg(colors.primary)
                                        .text_color(colors.primary_foreground)
                                        .text_xs()
                                        .font_weight(crate::ui::theme::MEDIUM)
                                        .when_else(
                                            can_confirm,
                                            |button| {
                                                button
                                                    .cursor_pointer()
                                                    .hover(move |style| {
                                                        style.bg(colors.primary.opacity(0.9))
                                                    })
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.confirm_speaker(cx)
                                                    }))
                                            },
                                            |button| button.opacity(0.5),
                                        )
                                        .child("Confirm"),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person(id: &str, name: &str, email: &str) -> Person {
        Person {
            id: id.into(),
            name: name.into(),
            email: email.into(),
        }
    }

    #[test]
    fn participants_lead_people_and_new_names_can_be_added() {
        let participants = [person("a", "Alex Kim", "alex@x.com")];
        let contacts = [
            person("a", "Alex Kim", "alex@x.com"),
            person("b", "Bea Park", "bea@x.com"),
            person("c", "Other Alex", "ALEX@x.com"),
        ];
        let (listed, others, create) = groups(&participants, &contacts, "");
        assert_eq!(listed, [person("a", "Alex Kim", "alex@x.com")]);
        assert_eq!(
            others,
            [person("b", "Bea Park", "bea@x.com")],
            "same email dedupes"
        );
        assert_eq!(create, None);
        let (_, others, create) = groups(&participants, &contacts, "bea");
        assert_eq!(others.len(), 1);
        assert_eq!(create.as_deref(), Some("bea"));
        let (_, _, create) = groups(&participants, &contacts, "Bea Park");
        assert_eq!(create, None, "an existing name is chosen, not added");
    }
}
