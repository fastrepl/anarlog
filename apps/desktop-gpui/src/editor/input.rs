use std::ops::Range;

use gpui::{
    Bounds, Context, EntityInputHandler, KeyDownEvent, Pixels, Point, UTF16Selection, Window,
};

use super::{
    EditorPane, clipboard,
    model::{EditorModel, Selection},
};
use crate::ui::caret::Caret;

impl EditorPane {
    pub(super) fn key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        Caret::reset_entity(&self.caret, cx);
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        // A clicked image is node-selected: Backspace or Delete removes it, as in shipping.
        if let Some((start, id)) = self.selected_image.take()
            && matches!(key, "backspace" | "delete")
            && let Some(model) = &mut self.model
        {
            let before = model.revision;
            let result = model.remove_block_atom(start, id);
            self.edited(before, result, cx);
            cx.stop_propagation();
            return;
        }
        let document_boundary = (matches!(key, "home" | "end")
            && (modifiers.control || modifiers.platform))
            || (cfg!(target_os = "macos") && modifiers.platform && matches!(key, "up" | "down"));
        let key = if cfg!(target_os = "macos") && modifiers.platform {
            match key {
                "left" | "up" => "home",
                "right" | "down" => "end",
                key => key,
            }
        } else {
            key
        };
        if self.link_input.is_some() {
            match key {
                "escape" => self.link_input = None,
                "enter" => self.commit_link(cx),
                "backspace" => {
                    self.link_input.as_mut().expect("link").pop();
                }
                "v" if modifiers.secondary(cx) => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                        self.link_input = Some(text);
                    }
                }
                _ => return,
            }
            cx.notify();
            cx.stop_propagation();
            return;
        }
        if key == "k" && modifiers.secondary(cx) {
            self.link_input = Some(String::new());
            cx.notify();
            cx.stop_propagation();
            return;
        }
        if let Some(menu) = &mut self.mention {
            match key {
                "up" | "down" => {
                    let len = menu.results.candidates.len();
                    if len > 0 {
                        menu.selected =
                            (menu.selected + if key == "down" { 1 } else { len - 1 }) % len;
                    }
                    cx.notify();
                    cx.stop_propagation();
                    return;
                }
                "escape" => {
                    self.dismiss_mention();
                    cx.notify();
                    cx.stop_propagation();
                    return;
                }
                "enter" => {
                    self.commit_mention(cx);
                    cx.stop_propagation();
                    return;
                }
                _ => {}
            }
        }
        if let Some(menu) = &mut self.slash
            && !menu.items().is_empty()
        {
            match key {
                "up" | "down" => {
                    menu.step(key == "down");
                    cx.notify();
                    cx.stop_propagation();
                    return;
                }
                "escape" => {
                    self.slash_dismissed = Some(menu.start);
                    self.slash = None;
                    cx.notify();
                    cx.stop_propagation();
                    return;
                }
                "enter" => {
                    self.slash_commit(cx);
                    cx.stop_propagation();
                    return;
                }
                _ => {}
            }
        }
        if self.model.as_ref().is_some_and(EditorModel::composing) {
            if key == "escape" {
                let model = self.model.as_mut().expect("model");
                let before = model.revision;
                model.cancel_composition();
                self.edited(before, Ok(()), cx);
                cx.stop_propagation();
            }
            return;
        }
        if modifiers.secondary(cx) {
            match key {
                "c" | "x" => {
                    self.copy(key == "x", cx);
                    cx.stop_propagation();
                    return;
                }
                "v" => {
                    if let Some(item) = cx.read_from_clipboard() {
                        if let Some(text) = item.text() {
                            let metadata = if modifiers.shift {
                                None
                            } else {
                                item.metadata().cloned()
                            };
                            self.paste(text, metadata, !modifiers.shift, cx);
                        } else if let Some(gpui::ClipboardEntry::Image(image)) = item
                            .entries()
                            .iter()
                            .find(|entry| matches!(entry, gpui::ClipboardEntry::Image(_)))
                        {
                            self.paste_image(image.clone(), cx);
                            cx.notify();
                        }
                    }
                    cx.stop_propagation();
                    return;
                }
                _ => {}
            }
        }
        let Some(model) = &mut self.model else {
            return;
        };
        let before = model.revision;
        let result = match key {
            "b" if modifiers.secondary(cx) => model.toggle_mark("bold"),
            "i" if modifiers.secondary(cx) => model.toggle_mark("italic"),
            "u" if modifiers.secondary(cx) => model.toggle_mark("underline"),
            "`" if modifiers.secondary(cx) => model.toggle_mark("code"),
            "z" if modifiers.secondary(cx) && modifiers.shift => model.redo(),
            "z" if modifiers.secondary(cx) => model.undo(),
            "y" if modifiers.secondary(cx) => model.redo(),
            "a" if modifiers.secondary(cx) => {
                model.select(Selection {
                    anchor: 0,
                    head: model.document.units(),
                });
                Ok(())
            }
            "left" | "right" if modifiers.alt || modifiers.control => {
                model.move_word(key == "right", modifiers.shift)
            }
            "left" | "right" => model.move_grapheme(key == "right", modifiers.shift),
            "up" | "down" | "pageup" | "pagedown" => {
                let forward = matches!(key, "down" | "pagedown");
                let page = matches!(key, "pageup" | "pagedown");
                let page_height = self
                    .viewport
                    .map(|viewport| viewport.size.height)
                    .unwrap_or(gpui::px(480.));
                let rows = if page {
                    (f32::from(page_height) / 24.).round() as usize
                } else {
                    1
                };
                let caret = self.layouts.values().find_map(|layout| {
                    layout.bounds_for(model.selection.head..model.selection.head)
                });
                if let Some(caret) = caret {
                    let target = gpui::point(
                        caret.left(),
                        caret.top()
                            + if page { page_height } else { caret.size.height }
                                * if forward { 1. } else { -1. },
                    );
                    if let Some(layout) = self
                        .layouts
                        .values()
                        .find(|layout| layout.layout.bounds().contains(&target))
                    {
                        let position = layout.position(target);
                        model.select(Selection {
                            anchor: if modifiers.shift {
                                model.selection.anchor
                            } else {
                                position
                            },
                            head: position,
                        });
                    } else {
                        if let Err(error) =
                            model.move_vertical_blocks(forward, rows, modifiers.shift)
                        {
                            self.message = error;
                        }
                    }
                } else if let Err(error) =
                    model.move_vertical_blocks(forward, rows, modifiers.shift)
                {
                    self.message = error;
                }
                Ok(())
            }
            "home" | "end" if document_boundary => {
                model.move_document_boundary(key == "end", modifiers.shift);
                Ok(())
            }
            "home" | "end" => model
                .document
                .resolve(model.selection.head)
                .map(|resolved| {
                    let end = key == "end";
                    let position = self
                        .layouts
                        .values()
                        .find_map(|layout| {
                            let caret =
                                layout.bounds_for(model.selection.head..model.selection.head)?;
                            Some(layout.position(gpui::point(
                                if end {
                                    layout.layout.bounds().right() + gpui::px(10.)
                                } else {
                                    layout.layout.bounds().left() - gpui::px(10.)
                                },
                                caret.origin.y + caret.size.height / 2.,
                            )))
                        })
                        .unwrap_or(
                            resolved.start
                                + if end {
                                    resolved.node.children.units()
                                } else {
                                    0
                                },
                        );
                    model.select(Selection {
                        anchor: if modifiers.shift {
                            model.selection.anchor
                        } else {
                            position
                        },
                        head: position,
                    });
                }),
            "backspace" | "delete" if modifiers.alt || modifiers.control => {
                if model.selection.is_empty() {
                    model.move_word(key == "delete", true)
                } else {
                    Ok(())
                }
                .and_then(|()| model.replace(model.selection.range(), ""))
            }
            "backspace" | "delete" => model.delete(key == "delete"),
            "enter" if !modifiers.shift => model.split_block(),
            "enter" => model.insert_inline_atom("hardBreak", Default::default()),
            "tab" => {
                let in_table = model
                    .document
                    .resolve(model.selection.head)
                    .is_ok_and(|resolved| {
                        (1..resolved.path.len()).any(|depth| {
                            model
                                .document
                                .node(&resolved.path[..depth])
                                .is_some_and(|node| node.kind() == "table")
                        })
                    });
                if in_table {
                    model.table_move(!modifiers.shift)
                } else {
                    model.indent_list(modifiers.shift)
                }
            }
            "escape" => {
                self.init.return_focus.focus(window, cx);
                return;
            }
            _ => return,
        };
        self.edited(before, result, cx);
        self.reveal_caret();
        self.update_slash();
        self.update_mention(cx);
        cx.stop_propagation();
    }
}

impl EntityInputHandler for EditorPane {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let text = clipboard::text_for_range(&self.model.as_ref()?.document, range.clone()).ok()?;
        *actual = Some(range);
        Some(text)
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let selection = self.model.as_ref()?.selection;
        Some(UTF16Selection {
            range: selection.range(),
            reversed: selection.head < selection.anchor,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.model.as_ref()?.marked_range()
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        Caret::reset_entity(&self.caret, cx);
        if let Some(model) = &mut self.model {
            model.commit_composition();
            if let Some(journal) = &self.journal {
                journal.publish(model.revision, model.document.clone());
            }
            cx.notify();
        }
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(input) = &mut self.link_input {
            if input.len() + text.len() <= 8192 {
                input.push_str(text);
            }
            cx.notify();
            return;
        }
        let Some(model) = &mut self.model else {
            return;
        };
        let before = model.revision;
        let range = range
            .or_else(|| model.marked_range())
            .unwrap_or_else(|| model.selection.range());
        let result = model.type_text(range, text);
        if result.is_ok() {
            model.commit_composition();
        }
        self.edited(before, result, cx);
        self.update_slash();
        self.update_mention(cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(model) = &mut self.model else {
            return;
        };
        let before = model.revision;
        let end = text.encode_utf16().count();
        let result = model.compose(range, text, selected.unwrap_or(end..end));
        self.edited(before, result, cx);
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        self.layouts
            .values()
            .find_map(|layout| layout.bounds_for(range.clone()))
    }
    fn character_index_for_point(
        &mut self,
        position: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        self.hit(position)
    }
}
