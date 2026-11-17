//! Roles, names and states for screen readers: the native counterpart of shipping's DOM
//! semantics (`<button>`, `aria-label`, Radix menus, dialogs, tabs and switches).
//!
//! GPUI hands AccessKit one node per element that has both an `.id(...)` and a role; plain
//! text children are not nodes, so every control names itself here, from shipping's
//! `aria-label` or its visible text. Nodes without a role are skipped and their children
//! join the nearest ancestor that has one, so tree order is visual (DOM) order.

use gpui::{Role, SharedString, StatefulInteractiveElement, Toggled, accesskit};

pub trait Accessible: StatefulInteractiveElement {
    /// A `<button>`, named by its visible text or shipping's `aria-label`.
    fn a11y_button(self, label: impl Into<SharedString>) -> Self {
        self.role(Role::Button).aria_label(label)
    }

    /// A link that leaves the app or opens a page.
    fn a11y_link(self, label: impl Into<SharedString>) -> Self {
        self.role(Role::Link).aria_label(label)
    }

    /// Radix `Switch` (`role="switch"`).
    fn a11y_switch(self, label: impl Into<SharedString>, on: bool) -> Self {
        self.role(Role::Switch)
            .aria_label(label)
            .aria_toggled(toggled(on))
    }

    fn a11y_checkbox(self, label: impl Into<SharedString>, checked: bool) -> Self {
        self.role(Role::CheckBox)
            .aria_label(label)
            .aria_toggled(toggled(checked))
    }

    /// One choice of a segmented control or radio group.
    fn a11y_radio(self, label: impl Into<SharedString>, checked: bool) -> Self {
        self.role(Role::RadioButton)
            .aria_label(label)
            .aria_toggled(toggled(checked))
    }

    fn a11y_tab(self, label: impl Into<SharedString>, selected: bool) -> Self {
        self.role(Role::Tab)
            .aria_label(label)
            .aria_selected(selected)
    }

    fn a11y_menu_item(self, label: impl Into<SharedString>) -> Self {
        self.role(Role::MenuItem).aria_label(label)
    }

    /// A menu item that shows a check (sort order, grouping, filters).
    fn a11y_menu_check(self, label: impl Into<SharedString>, checked: bool) -> Self {
        self.role(Role::MenuItemCheckBox)
            .aria_label(label)
            .aria_toggled(toggled(checked))
    }

    /// One choice of a menu's radio group (sort order, grouping).
    fn a11y_menu_radio(self, label: impl Into<SharedString>, checked: bool) -> Self {
        self.role(Role::MenuItemRadio)
            .aria_label(label)
            .aria_toggled(toggled(checked))
    }

    /// A row of a navigable list (timeline, contacts, templates, settings): a button
    /// that reports whether it is the open one.
    fn a11y_row(self, label: impl Into<SharedString>, selected: bool) -> Self {
        self.role(Role::Button)
            .aria_label(label)
            .aria_selected(selected)
    }

    /// An option of a listbox or picker; selection follows the keyboard highlight.
    fn a11y_option(self, label: impl Into<SharedString>, selected: bool) -> Self {
        self.role(Role::ListBoxOption)
            .aria_label(label)
            .aria_selected(selected)
    }

    /// A named container: list, menu, tab list, toolbar, navigation or region.
    fn a11y_group(self, role: Role, label: impl Into<SharedString>) -> Self {
        self.role(role).aria_label(label)
    }

    /// A modal dialog named by its title (Radix `Dialog`, `aria-modal`).
    fn a11y_dialog(self, label: impl Into<SharedString>) -> Self {
        self.role(Role::Dialog)
            .aria_label(label)
            .a11y_synthetic_children(|builder| builder.parent_node().set_modal())
    }

    fn a11y_heading(self, label: impl Into<SharedString>, level: usize) -> Self {
        self.role(Role::Heading).aria_label(label).aria_level(level)
    }

    /// Static text a screen reader should reach (labels, descriptions, empty states).
    fn a11y_text(self, text: impl Into<SharedString>) -> Self {
        self.role(Role::Label).aria_value(text)
    }

    /// A live region: toasts and inline errors are read without moving focus.
    fn a11y_status(self, text: impl Into<SharedString>, assertive: bool) -> Self {
        let live = if assertive {
            accesskit::Live::Assertive
        } else {
            accesskit::Live::Polite
        };
        self.role(if assertive { Role::Alert } else { Role::Status })
            .aria_label(text)
            .a11y_synthetic_children(move |builder| builder.parent_node().set_live(live))
    }

    /// The screen reader's press for a control that acts on mouse down: GPUI only wires
    /// `on_click` to AccessKit's click action.
    fn a11y_press<T: 'static>(
        self,
        cx: &gpui::Context<T>,
        press: impl Fn(&mut T, &mut gpui::Window, &mut gpui::Context<T>) + 'static,
    ) -> Self {
        let entity = cx.weak_entity();
        self.on_a11y_action(accesskit::Action::Click, move |_, window, cx| {
            let _ = entity.update(cx, |this, cx| press(this, window, cx));
        })
    }

    /// GPUI's element API has no disabled state, so it is set on the element's own node.
    /// This takes the element's synthetic-children slot; don't combine it with another.
    fn a11y_disabled(self, disabled: bool) -> Self {
        if disabled {
            self.a11y_synthetic_children(|builder| builder.parent_node().set_disabled())
        } else {
            self
        }
    }
}

impl<E: StatefulInteractiveElement> Accessible for E {}

pub fn toggled(on: bool) -> Toggled {
    if on { Toggled::True } else { Toggled::False }
}

/// Text runs for a custom-painted field, so a screen reader can read it and move by
/// character, word and line with the caret. One run per source line (AccessKit counts at
/// most 255 characters per run, so longer lines continue in linked runs). `selection` is a
/// byte range into `text`, anchor first.
pub fn text_runs(
    builder: &mut gpui::A11ySubtreeBuilder,
    text: &str,
    selection: Option<(usize, usize)>,
) {
    use unicode_segmentation::UnicodeSegmentation;

    // (id, node, first byte, byte length of each character)
    let mut runs: Vec<(accesskit::NodeId, accesskit::Node, usize, Vec<usize>)> = Vec::new();
    let mut line_start = 0;
    // An empty field, or one ending in a newline, still has a line for the caret.
    let trailing = (text.is_empty() || text.ends_with('\n')).then_some("");
    for line in text.split_inclusive('\n').chain(trailing) {
        let graphemes = line.grapheme_indices(true).collect::<Vec<_>>();
        let chunks = if graphemes.is_empty() {
            vec![&graphemes[..]]
        } else {
            graphemes.chunks(u8::MAX as usize).collect()
        };
        let first = runs.len();
        for chunk in chunks {
            let start = chunk.first().map_or(0, |(offset, _)| *offset);
            let end = chunk.last().map_or(0, |(offset, g)| offset + g.len());
            let mut run = accesskit::Node::new(Role::TextRun);
            run.set_value(&line[start..end]);
            run.set_character_lengths(
                chunk
                    .iter()
                    .map(|(_, g)| g.len().min(u8::MAX as usize) as u8)
                    .collect::<Vec<_>>(),
            );
            let mut previous_space = true;
            let mut word_starts = Vec::new();
            for (index, (_, grapheme)) in chunk.iter().enumerate() {
                let space = grapheme.chars().all(char::is_whitespace);
                if previous_space && !space {
                    word_starts.push(index as u8);
                }
                previous_space = space;
            }
            run.set_word_starts(word_starts);
            let id = builder.synthetic_node_id(("text-run", runs.len()));
            if runs.len() > first {
                let previous = runs.last_mut().expect("run");
                previous.1.set_next_on_line(id);
                run.set_previous_on_line(previous.0);
            }
            let lengths = chunk.iter().map(|(_, g)| g.len()).collect();
            runs.push((id, run, line_start + start, lengths));
        }
        line_start += line.len();
    }
    let position = |offset: usize| {
        let (id, _, start, lengths) = runs
            .iter()
            .rev()
            .find(|(_, _, start, _)| *start <= offset)
            .or(runs.first())?;
        let mut byte = *start;
        let character_index = lengths
            .iter()
            .take_while(|length| {
                let before = byte < offset;
                byte += **length;
                before
            })
            .count();
        Some(accesskit::TextPosition {
            node: *id,
            character_index,
        })
    };
    let selection = selection.and_then(|(anchor, focus)| {
        Some(accesskit::TextSelection {
            anchor: position(anchor)?,
            focus: position(focus)?,
        })
    });
    if let Some(selection) = selection {
        builder.parent_node().set_text_selection(selection);
    }
    for (id, run, _, _) in runs {
        builder.push_child(id, run);
    }
}
