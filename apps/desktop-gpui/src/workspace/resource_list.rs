//! Shared chrome of the templates and folders routes: shipping's `CustomSidebarHeader`, the
//! sidebar search field, list rows, `TemplateIconGlyph` and the app floating menus.

use std::sync::Arc;

use desktop_runtime::{CancellationToken, RuntimeHandle};
use futures::{
    FutureExt,
    future::{Either, select},
};
use gpui::{
    AnyElement, Context, Hsla, IntoElement, SharedString, Stateful, Window, div, prelude::*, px,
    rgb, svg,
};
use serde_json::Value;

use super::catalog_icons::{self, TEMPLATE_ICONS};
use crate::ui::{
    input::TextInput,
    theme::{Theme, is_dark},
};

/// The macOS traffic lights overlay the sidebar; keep the shipping app's gutter.
const CHROME_GUTTER: f32 = if cfg!(target_os = "macos") { 76. } else { 8. };

/// Follows a live query until `cancel`, handing each snapshot (or error) to `apply`.
pub fn watch<T: 'static>(
    runtime: &RuntimeHandle,
    sql: &'static str,
    cancel: CancellationToken,
    cx: &mut Context<T>,
    apply: impl Fn(&mut T, Result<Vec<Value>, String>, &mut Context<T>) + 'static,
) {
    let reply = runtime.watch_query(sql.into(), vec![]);
    cx.spawn(async move |this, cx| {
        let mut watch = match async { reply?.receive().await }.await {
            Ok(watch) => watch,
            Err(error) => {
                let _ = this.update(cx, |this, cx| apply(this, Err(error.to_string()), cx));
                return;
            }
        };
        loop {
            if cancel.is_cancelled() {
                break;
            }
            let rows = watch.snapshots.borrow_and_update().rows.clone();
            let result = match watch
                .terminal_error()
                .or_else(|| watch.errors.try_recv().ok())
            {
                Some(error) => Err(error.to_string()),
                None => Ok(rows.to_vec()),
            };
            if this.update(cx, |this, cx| apply(this, result, cx)).is_err()
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

/// A stored template or folder icon (`TemplateIcon` in shipping).
#[derive(Clone, Debug, PartialEq)]
pub enum Glyph {
    Icon { key: Arc<str>, color: u32 },
    Emoji(Arc<str>),
}

impl Glyph {
    pub fn template_default() -> Self {
        Self::Icon {
            key: "notebook-tabs".into(),
            color: 0x9ca3af,
        }
    }

    pub fn folder_default() -> Self {
        Self::Icon {
            key: "folder".into(),
            color: 0x9ca3af,
        }
    }

    /// `normalizeTemplateIcon`: unknown keys, bad colors and malformed JSON fall back.
    pub fn template(raw: &str) -> Self {
        serde_json::from_str::<Value>(raw)
            .ok()
            .and_then(|value| Self::from_value(&value))
            .unwrap_or_else(Self::template_default)
    }

    /// `normalizeFolderIcon`: unwraps up to three JSON string layers; anything that is not an
    /// explicit icon or emoji is the folder icon.
    pub fn folder(raw: &str) -> Self {
        let mut value = Value::String(raw.into());
        for _ in 0..3 {
            let Value::String(text) = &value else { break };
            let text = text.trim();
            if text.is_empty() {
                return Self::folder_default();
            }
            match serde_json::from_str(text) {
                Ok(next) => value = next,
                Err(_) => return Self::folder_default(),
            }
        }
        let explicit = matches!(value["type"].as_str(), Some("icon" | "emoji"))
            && value["value"]
                .as_str()
                .is_some_and(|text| !text.trim().is_empty());
        if explicit {
            Self::from_value(&value).unwrap_or_else(Self::template_default)
        } else {
            Self::folder_default()
        }
    }

    fn from_value(value: &Value) -> Option<Self> {
        let text = value["value"].as_str()?;
        match value["type"].as_str()? {
            "emoji" if !text.trim().is_empty() => Some(Self::Emoji(text.into())),
            "icon" => {
                TEMPLATE_ICONS.iter().find(|(key, _)| *key == text)?;
                let color = value["color"].as_str()?.strip_prefix('#')?;
                (color.len() == 6)
                    .then(|| u32::from_str_radix(color, 16).ok())
                    .flatten()
                    .map(|color| Self::Icon {
                        key: text.into(),
                        color,
                    })
            }
            _ => None,
        }
    }

    pub fn to_json(&self) -> String {
        match self {
            Self::Icon { key, color } => {
                serde_json::json!({"type": "icon", "value": key, "color": format!("#{color:06x}")})
            }
            Self::Emoji(value) => serde_json::json!({"type": "emoji", "value": value}),
        }
        .to_string()
    }

    /// `TemplateIconGlyph` at `size` (an icon's box, or an emoji's font size).
    pub fn render(&self, size: f32) -> AnyElement {
        match self {
            Self::Icon { key, color } => {
                let name = TEMPLATE_ICONS
                    .iter()
                    .find(|(candidate, _)| *candidate == key.as_ref())
                    .map_or("NotebookIcon", |(_, name)| name);
                svg()
                    .path(catalog_icons::path(name))
                    .size(px(size))
                    .flex_shrink_0()
                    .text_color(rgb(*color))
                    .into_any_element()
            }
            // Shipping sets emoji in `text-sm` inside the icon's `size-4` box.
            Self::Emoji(value) => div()
                .size(px(size))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(size - 2.))
                .line_height(px(size))
                .child(SharedString::from(value.clone()))
                .into_any_element(),
        }
    }
}

pub fn icon(name: &str, size: f32, color: Hsla) -> gpui::Svg {
    svg()
        .path(catalog_icons::path(name))
        .size(px(size))
        .flex_shrink_0()
        .text_color(color)
}

/// Shipping's `CustomSidebarHeader`: the back arrow in the chrome row, actions on the right.
pub fn sidebar_header(
    id: &'static str,
    colors: Theme,
    on_back: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
    actions: impl IntoIterator<Item = AnyElement>,
) -> gpui::Div {
    div()
        .h(px(48.))
        .flex_shrink_0()
        .flex()
        .items_start()
        .pr_1()
        .pl(px(CHROME_GUTTER))
        .pt(px(9.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .child(ghost_icon_button(id, "ArrowLeft02Icon", colors).on_click(on_back)),
        )
        .child(
            div()
                .ml_1()
                .flex_shrink_0()
                .flex()
                .items_center()
                .children(actions),
        )
}

/// `Button size="icon" variant="ghost"` with muted text that brightens on hover.
pub fn ghost_icon_button(
    id: impl Into<SharedString>,
    icon_name: &'static str,
    colors: Theme,
) -> Stateful<gpui::Div> {
    let id = id.into();
    div()
        .id(id.clone())
        .group(id.clone())
        .size(px(28.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(8.))
        .cursor_pointer()
        .hover(|style| style.bg(colors.accent))
        .child(
            icon(icon_name, 16., colors.muted_foreground)
                .group_hover(id, |style| style.text_color(colors.foreground)),
        )
}

/// The sidebar search field: a 32px pill with a magnifier, the input and a clear button.
pub fn search_field(
    input: &gpui::Entity<TextInput>,
    focused: bool,
    has_text: bool,
    colors: Theme,
    on_clear: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> gpui::Div {
    div().pb_2().child(
        div()
            .h(px(32.))
            .w_full()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .rounded(px(8.))
            .border_1()
            .border_color(colors.border)
            .bg(if focused {
                colors.accent
            } else {
                colors.accent.opacity(0.5)
            })
            .child(svg_search(colors))
            .child(div().flex_1().min_w_0().text_sm().child(input.clone()))
            .when(has_text, |view| {
                view.child(
                    div()
                        .id("clear-search")
                        .size(px(16.))
                        .flex_shrink_0()
                        .cursor_pointer()
                        .on_click(on_clear)
                        .child(icon("XIcon", 16., colors.muted_foreground)),
                )
            }),
    )
}

fn svg_search(colors: Theme) -> gpui::Svg {
    icon("Search01Icon", 16., colors.muted_foreground)
}

/// A sidebar list row (`rounded-lg px-3 py-2 text-sm`), selected rows on the accent. Folder
/// rows mute their text until selected or hovered.
pub fn list_row(
    id: impl Into<gpui::ElementId>,
    selected: bool,
    muted: bool,
    colors: Theme,
) -> Stateful<gpui::Div> {
    div()
        .id(id)
        .w_full()
        .px_3()
        .py_2()
        .rounded(px(8.))
        .text_sm()
        .line_height(px(20.))
        .cursor_pointer()
        .map(|row| {
            if selected {
                row.bg(colors.accent)
            } else if muted {
                row.text_color(colors.muted_foreground).hover(|style| {
                    style
                        .bg(colors.accent.opacity(0.5))
                        .text_color(colors.foreground)
                })
            } else {
                row.hover(|style| style.bg(colors.accent.opacity(0.5)))
            }
        })
}

/// The empty state of a sidebar list: a 32px muted icon over a short message.
pub fn empty_list(icon_name: &'static str, message: &'static str, colors: Theme) -> gpui::Div {
    div()
        .px_3()
        .py_8()
        .flex()
        .flex_col()
        .items_center()
        .text_color(colors.muted_foreground)
        .child(icon(icon_name, 32., colors.muted_foreground.opacity(0.7)).mb_2())
        .child(div().text_sm().line_height(px(20.)).child(message))
}

/// Shipping's `AppFloatingPanel` inside its chrome ring.
pub fn floating_menu(width: f32, colors: Theme) -> gpui::Div {
    div()
        .w(px(width))
        .p(px(2.))
        .rounded(px(22.))
        .border_1()
        .border_color(colors.app_floating_border)
        .bg(colors.app_floating_chrome)
        .shadow_lg()
}

pub fn floating_menu_body(colors: Theme) -> gpui::Div {
    div()
        .p(px(6.))
        .rounded(px(20.))
        .border_1()
        .border_color(colors.app_floating_border)
        .bg(colors.app_floating_panel)
        .text_color(colors.popover_foreground)
        .flex()
        .flex_col()
}

/// A `DropdownMenuItem`; destructive items are red, disabled ones dimmed and inert.
pub fn menu_item(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
    destructive: bool,
    disabled: bool,
    window: &Window,
    colors: Theme,
) -> Stateful<gpui::Div> {
    let red: Hsla = if is_dark(window) {
        rgb(0xf87171).into()
    } else {
        rgb(0xdc2626).into()
    };
    div()
        .id(id)
        .h(px(32.))
        .px_2()
        .flex()
        .items_center()
        .rounded(px(14.))
        .text_sm()
        .line_height(px(20.))
        .when(destructive, |item| item.text_color(red))
        .when_else(
            disabled,
            |item| item.opacity(0.5),
            |item| item.cursor_pointer().hover(|style| style.bg(colors.accent)),
        )
        .child(label)
}

/// The right-click menu, in the style of the timeline's note menu.
pub fn context_menu(colors: Theme) -> gpui::Div {
    div()
        .w(px(224.))
        .p_1()
        .rounded(px(8.))
        .bg(colors.card)
        .border_1()
        .border_color(colors.border)
        .shadow_lg()
}

pub fn context_item(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
    colors: Theme,
) -> Stateful<gpui::Div> {
    div()
        .id(id)
        .h(px(30.))
        .px_3()
        .flex()
        .items_center()
        .text_sm()
        .rounded(px(4.))
        .cursor_pointer()
        .hover(|style| style.bg(colors.accent))
        .child(label)
}

/// Closes whichever dropdown the view has open.
pub trait MenuOwner {
    fn close_menu(&mut self);
}

/// A dropdown aligned under its trigger's right edge (`align="end"`).
pub fn dropdown<T: MenuOwner + 'static>(
    corner: gpui::Anchor,
    menu: gpui::Div,
    cx: &mut Context<T>,
) -> AnyElement {
    div()
        .absolute()
        .top(px(32.))
        .right_0()
        .child(gpui::deferred(
            gpui::anchored().anchor(corner).snap_to_window().child(
                menu.id("dropdown")
                    .occlude()
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_down_out(cx.listener(|this: &mut T, _, _, cx| {
                        this.close_menu();
                        cx.notify();
                    })),
            ),
        ))
        .into_any_element()
}

/// A popover opening under its 28px trigger's left edge (`align="start" sideOffset={6}`).
pub fn popover<T: MenuOwner + 'static>(content: AnyElement, cx: &mut Context<T>) -> AnyElement {
    div()
        .absolute()
        .top(px(34.))
        .left_0()
        .child(gpui::deferred(
            gpui::anchored().snap_to_window().child(
                div()
                    .id("popover")
                    .occlude()
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_down_out(cx.listener(|this: &mut T, _, _, cx| {
                        this.close_menu();
                        cx.notify();
                    }))
                    .child(content),
            ),
        ))
        .into_any_element()
}

/// A window-wide dimmed backdrop centering `content`.
pub fn modal(window: &Window, content: AnyElement) -> AnyElement {
    gpui::deferred(
        gpui::anchored()
            .position(gpui::point(px(0.), px(0.)))
            .child(
                div()
                    .id("modal")
                    .w(window.viewport_size().width)
                    .h(window.viewport_size().height)
                    .occlude()
                    .bg(gpui::rgba(0x00000066))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(content),
            ),
    )
    .with_priority(2)
    .into_any_element()
}

/// Shipping's `GlassDialogContent`; native views cannot blur what is behind them.
pub fn glass_dialog(colors: Theme) -> Stateful<gpui::Div> {
    div()
        .id("glass-dialog")
        .w(px(320.))
        .p_5()
        .flex()
        .flex_col()
        .gap_4()
        .rounded(px(26.))
        .border_1()
        .border_color(colors.border.opacity(0.45))
        .bg(colors.card.opacity(0.6))
        .shadow(vec![gpui::BoxShadow {
            color: gpui::hsla(0., 0., 0., 0.32),
            offset: gpui::point(px(0.), px(24.)),
            blur_radius: px(70.),
            spread_radius: px(0.),
            inset: false,
        }])
        .occlude()
}

/// The glass dialog's footer buttons: cancel, or a filled (destructive) confirm.
pub fn dialog_button(
    id: &'static str,
    label: &'static str,
    destructive: bool,
    colors: Theme,
) -> Stateful<gpui::Div> {
    let button = div()
        .id(id)
        .h(px(32.))
        .px_4()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(8.))
        .text_xs()
        .line_height(px(16.))
        .font_weight(crate::ui::theme::MEDIUM)
        .cursor_pointer()
        .shadow_sm()
        .child(label);
    match (id, destructive) {
        ("dialog-cancel", _) => button
            .border_1()
            .border_color(colors.border.opacity(0.7))
            .bg(colors.background.opacity(0.5))
            .text_color(colors.foreground)
            .hover(|style| style.bg(colors.background.opacity(0.8))),
        (_, true) => button
            .bg(colors.destructive)
            .text_color(colors.destructive_foreground)
            .hover(|style| style.bg(colors.destructive.opacity(0.9))),
        _ => button
            .bg(colors.primary)
            .text_color(colors.primary_foreground)
            .hover(|style| style.bg(colors.primary.opacity(0.9))),
    }
}

/// `ResourceShareButton`'s trigger.
pub fn share_button(id: &'static str, colors: Theme) -> Stateful<gpui::Div> {
    div()
        .id(id)
        .group(id)
        .h(px(28.))
        .px_2()
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded(px(8.))
        .text_xs()
        .line_height(px(16.))
        .font_weight(crate::ui::theme::MEDIUM)
        .text_color(colors.muted_foreground)
        .cursor_pointer()
        .hover(|style| style.bg(colors.accent).text_color(colors.foreground))
        .child(
            icon("Share08Icon", 16., colors.muted_foreground)
                .group_hover(id, |style| style.text_color(colors.foreground)),
        )
        .child("Share")
}

/// A `toast.warning` with an action, as the app toaster shows it: the action, Dismiss, and the
/// auto-dismiss countdown that Pause holds.
pub struct WarningToast {
    pub message: &'static str,
    pub action: &'static str,
    pub remaining_ms: u64,
    pub total_ms: u64,
    pub paused: bool,
}

type Click = Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App)>;

impl WarningToast {
    pub fn render(
        &self,
        [on_action, on_dismiss, on_pause]: [Click; 3],
        window: &Window,
        colors: Theme,
    ) -> AnyElement {
        let amber: Hsla = if is_dark(window) {
            rgb(0xfbbf24).into()
        } else {
            rgb(0xd97706).into()
        };
        let seconds = self.remaining_ms.div_ceil(1000);
        let progress = self.remaining_ms as f32 / self.total_ms.max(1) as f32;
        let button = |id: &'static str, label: &'static str, primary: bool| {
            div()
                .id(id)
                .min_h(px(28.))
                .px(px(10.))
                .py_1()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(6.))
                .text_xs()
                .line_height(px(16.))
                .font_weight(crate::ui::theme::MEDIUM)
                .cursor_pointer()
                .map(|button| {
                    if primary {
                        button
                            .bg(colors.primary)
                            .text_color(colors.primary_foreground)
                            .hover(|style| style.bg(colors.primary.opacity(0.9)))
                    } else {
                        button
                            .border_1()
                            .border_color(colors.border)
                            .bg(colors.background)
                            .text_color(colors.foreground)
                            .hover(|style| style.bg(colors.accent))
                    }
                })
                .child(label)
        };
        gpui::deferred(
            div()
                .id("app-toast")
                .absolute()
                .bottom(px(16.))
                .right(px(16.))
                .w(px(320.))
                .overflow_hidden()
                .rounded(px(14.))
                .border_1()
                .border_color(colors.border.opacity(0.7))
                .bg(colors.popover)
                .text_color(colors.popover_foreground)
                .shadow_md()
                .occlude()
                .child(
                    div()
                        .min_h(px(44.))
                        .px(px(10.))
                        .py(px(6.))
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .size(px(32.))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(icon("AlertCircleIcon", 16., amber)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(px(13.))
                                .line_height(px(20.))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(self.message),
                        ),
                )
                .child(
                    div()
                        .px(px(10.))
                        .pb(px(10.))
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(button("toast-action", self.action, true).on_click(on_action))
                        .child(button("toast-dismiss", "Dismiss", false).on_click(on_dismiss)),
                )
                .child(
                    div()
                        .id("toast-timer")
                        .relative()
                        .min_h(px(32.))
                        .px(px(10.))
                        .py(px(6.))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .border_t_1()
                        .border_color(colors.border.opacity(0.6))
                        .bg(colors.muted.opacity(0.5))
                        .text_size(px(11.))
                        .text_color(colors.muted_foreground)
                        .cursor_pointer()
                        .hover(|style| style.bg(colors.muted))
                        .on_click(on_pause)
                        .child(
                            div()
                                .flex()
                                .gap(px(3.))
                                .child(if self.paused {
                                    "Timer paused"
                                } else {
                                    "Closing in"
                                })
                                .child(
                                    div()
                                        .text_color(colors.foreground)
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .child(seconds.to_string()),
                                )
                                .child(if seconds == 1 { "second" } else { "seconds" }),
                        )
                        .child(
                            div()
                                .text_color(colors.foreground)
                                .font_weight(crate::ui::theme::MEDIUM)
                                .child(if self.paused { "Resume" } else { "Pause" }),
                        )
                        .child(
                            div()
                                .absolute()
                                .left_0()
                                .bottom_0()
                                .h(px(2.))
                                .w(gpui::relative(progress))
                                .bg(rgb(0x3b82f6)),
                        ),
                ),
        )
        .with_priority(1)
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_icons_normalize_like_shipping() {
        assert_eq!(
            Glyph::template(r##"{"type":"icon","value":"rocket","color":"#5B67D8"}"##),
            Glyph::Icon {
                key: "rocket".into(),
                color: 0x5b67d8
            }
        );
        assert_eq!(
            Glyph::template(r#"{"type":"emoji","value":"🚀"}"#),
            Glyph::Emoji("🚀".into())
        );
        for raw in [
            r##"{"type":"icon","value":"unknown","color":"#000000"}"##,
            r#"{"type":"icon","value":"rocket","color":"red"}"#,
            r#"{"type":"emoji","value":"  "}"#,
            "{broken",
        ] {
            assert_eq!(Glyph::template(raw), Glyph::template_default());
        }
        let nested = serde_json::to_string(r#"{"type":"emoji","value":"📁"}"#).unwrap();
        assert_eq!(Glyph::folder(&nested), Glyph::Emoji("📁".into()));
        assert_eq!(Glyph::folder(""), Glyph::folder_default());
        assert_eq!(
            Glyph::folder(r#"{"type":"other"}"#),
            Glyph::folder_default()
        );
        // An explicit but invalid icon falls back like `normalizeTemplateIcon`.
        assert_eq!(
            Glyph::folder(r##"{"type":"icon","value":"nope","color":"#000000"}"##),
            Glyph::template_default()
        );
        let icon = Glyph::Icon {
            key: "folder".into(),
            color: 0x4ab883,
        };
        assert_eq!(Glyph::folder(&icon.to_json()), icon);
    }
}
