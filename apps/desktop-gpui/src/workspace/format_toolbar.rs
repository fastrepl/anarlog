//! `packages/editor/src/widgets/format-toolbar.tsx`: the floating mark
//! toolbar over a non-empty selection in the memo or summary editor.

use gpui::{
    AnyElement, Bounds, Context, Entity, Focusable as _, MouseButton, Pixels, SharedString, Window,
    div, prelude::*, px,
};

use super::Workspace;
use crate::editor::BodyEditor;
use crate::ui::icon;

/// `TOOLBAR_BUTTONS`: `(mark, icon)` in order.
const BUTTONS: [(&str, &str); 6] = [
    ("bold", "text-bold"),
    ("italic", "text-italic"),
    ("underline", "text-underline"),
    ("strike", "text-strikethrough"),
    ("code", "code"),
    ("highlight", "highlighter"),
];

const BUTTON: f32 = 28.0;
const GAP: f32 = 2.0;
const PADDING: f32 = 4.0;
/// `offset(8)` and the `flip` / `shift` padding.
const OFFSET: f32 = 8.0;

/// floating-ui's `placement: "top"` with `offset(8)`, `flip` to the bottom
/// and `shift` inside the boundary (both with `padding: 8`).
pub(crate) fn place(
    reference: Bounds<Pixels>,
    size: gpui::Size<Pixels>,
    boundary: Bounds<Pixels>,
) -> gpui::Point<Pixels> {
    let centre = reference.left() + reference.size.width / 2.0;
    let mut x = centre - size.width / 2.0;
    let above = reference.top() - px(OFFSET) - size.height;
    let below = reference.bottom() + px(OFFSET);
    let y = if above < boundary.top() + px(OFFSET) && below + size.height <= boundary.bottom() {
        below
    } else {
        above
    };
    let min_x = boundary.left() + px(OFFSET);
    let max_x = boundary.right() - px(OFFSET) - size.width;
    if max_x >= min_x {
        x = x.max(min_x).min(max_x);
    }
    gpui::Point::new(x, y)
}

impl Workspace {
    /// The editor whose selection the toolbar formats: the memo editor on the
    /// memo tab, the summary editor on its tab.
    fn format_toolbar_editor(&self, cx: &Context<Self>) -> Option<Entity<BodyEditor>> {
        let editor = match &self.note {
            super::Note::Ready { tab, .. } => match tab {
                super::NoteTab::Enhanced(note_id) => self
                    .enhanced_editor
                    .as_ref()
                    .filter(|(id, _)| id == note_id)
                    .map(|(_, editor)| editor.clone()),
                _ => self.editor.clone(),
            },
            _ => None,
        }?;
        // `shouldShowToolbar`: a non-empty selection outside the title.
        let state = editor.read(cx);
        (state.selection().is_some() && !state.selection_touches_title()).then_some(editor)
    }

    pub(crate) fn render_format_toolbar(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let editor = self.format_toolbar_editor(cx)?;
        let mut reference = editor.read(cx).selection_rect()?;
        // `coordsAtPos` measures the text's inline boxes, not the line boxes.
        let inset = self.body_half_leading(window);
        reference.origin.y += inset;
        reference.size.height -= inset * 2.0;
        let theme = self.theme;
        let size = gpui::size(
            px(BUTTONS.len() as f32 * BUTTON + (BUTTONS.len() - 1) as f32 * GAP + 2.0 * PADDING),
            px(BUTTON + 2.0 * PADDING),
        );
        // `getClipBoundary(view.dom)`: the note's scroll container.
        let boundary = self.note_scroll.bounds();
        // `ring-1` draws outside the box, so the border sits a pixel out.
        let position = place(reference, size, boundary) - gpui::point(px(1.0), px(1.0));
        let active: Vec<bool> = {
            let state = editor.read(cx);
            BUTTONS
                .iter()
                .map(|(mark, _)| state.selection_has_mark(mark))
                .collect()
        };
        Some(
            gpui::deferred(
                div()
                    .id("format-toolbar")
                    .absolute()
                    .left(position.x)
                    .top(position.y)
                    .flex()
                    .items_center()
                    .gap(px(GAP))
                    .p(px(PADDING))
                    .rounded(px(12.0))
                    .bg(theme.popover)
                    .border_1()
                    .border_color(theme.border)
                    // `shadow-lg`
                    .shadow(vec![
                        gpui::BoxShadow {
                            color: gpui::hsla(0.0, 0.0, 0.0, 0.1),
                            offset: gpui::point(px(0.0), px(10.0)),
                            blur_radius: px(15.0),
                            spread_radius: px(-3.0),
                        },
                        gpui::BoxShadow {
                            color: gpui::hsla(0.0, 0.0, 0.0, 0.1),
                            offset: gpui::point(px(0.0), px(4.0)),
                            blur_radius: px(6.0),
                            spread_radius: px(-4.0),
                        },
                    ])
                    // `onMouseDown={(e) => e.preventDefault()}`: the selection stays.
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .children(BUTTONS.iter().zip(active).map(|((mark, glyph), active)| {
                        let editor = editor.clone();
                        div()
                            .id(SharedString::from(format!("format-toolbar-{glyph}")))
                            .flex()
                            .size(px(BUTTON))
                            .items_center()
                            .justify_center()
                            .rounded(px(6.0))
                            .cursor_pointer()
                            .map(|button| {
                                if active {
                                    button.bg(theme.primary)
                                } else {
                                    button.hover(move |style| style.bg(theme.accent))
                                }
                            })
                            .child(icon(
                                glyph,
                                px(16.0),
                                if active {
                                    theme.primary_foreground
                                } else {
                                    theme.muted_foreground
                                },
                            ))
                            .on_click(move |_, window, cx| {
                                editor.update(cx, |editor, cx| {
                                    editor.toggle_mark(mark, cx);
                                    editor.focus_handle(cx).focus(window);
                                });
                            })
                    })),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds::new(gpui::point(px(x), px(y)), gpui::size(px(w), px(h)))
    }

    #[test]
    fn toolbar_sits_centred_above_and_flips_or_shifts_at_the_edges() {
        let size = gpui::size(px(186.0), px(36.0));
        let boundary = bounds(200.0, 100.0, 800.0, 600.0);
        // Centred over a selection with room above.
        let at = place(bounds(400.0, 300.0, 100.0, 24.0), size, boundary);
        assert_eq!(at, gpui::point(px(357.0), px(256.0)));
        // Too close to the top: below the selection instead.
        let at = place(bounds(400.0, 110.0, 100.0, 24.0), size, boundary);
        assert_eq!(at, gpui::point(px(357.0), px(142.0)));
        // Shifted inside the boundary's left padding.
        let at = place(bounds(205.0, 300.0, 10.0, 24.0), size, boundary);
        assert_eq!(at.x, px(208.0));
        // And the right.
        let at = place(bounds(990.0, 300.0, 5.0, 24.0), size, boundary);
        assert_eq!(at.x, px(1000.0 - 8.0 - 186.0));
    }
}
