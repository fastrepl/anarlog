//! `TemplateIconPicker` (`templates/template-icon-picker.tsx`): the Icons /
//! Emojis popover behind the template and folder icon buttons, with the
//! colour swatches, the react-colorful custom colour picker, the searchable
//! 12-column icon grid, and the emoji-mart categories.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    AnyElement, Bounds, ClickEvent, Context, Div, Entity, Focusable as _, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PathBuilder, Pixels, Point, ScrollHandle,
    SharedString, Window, canvas, div, linear_color_stop, linear_gradient, point, prelude::*, px,
};

use super::Workspace;
use super::toast::FlashVariant;
use crate::db::TemplateIcon;
use crate::text_input::{TextInput, TextInputEvent, TextInputStyle};
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon};

/// `ICON_COLORS`
const ICON_COLORS: [&str; 9] = [
    "#9ca3af", "#94a3b8", "#5b67d8", "#25b5c9", "#4ab883", "#f2bd00", "#ef923d", "#c99b92",
    "#f05257",
];

const POPOVER_WIDTH: f32 = 420.0;
/// The panel's content width: the chrome's border and `p-0.5` on each side.
const PANEL_WIDTH: f32 = POPOVER_WIDTH - 6.0;
const CELL: f32 = 28.0;
const GAP: f32 = 4.0;
const COLUMNS: usize = 12;

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum IconTarget {
    Template(String),
    Folder(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Icons,
    Emojis,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Drag {
    Saturation,
    Hue,
}

pub(crate) struct IconPicker {
    pub(crate) target: IconTarget,
    tab: Tab,
    icon_search: Entity<TextInput>,
    emoji_search: Entity<TextInput>,
    hex_input: Entity<TextInput>,
    /// `iconColor`
    color: String,
    /// `lastIconValue`
    last_icon_value: String,
    custom_color_open: bool,
    /// The react-colorful HSV state behind the custom picker.
    hsv: (f32, f32, f32),
    drag: Option<Drag>,
    saturation_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    hue_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    icons_scroll: ScrollHandle,
    emojis_scroll: ScrollHandle,
}

impl Workspace {
    pub(crate) fn icon_picker_open(&self) -> bool {
        self.icon_picker.is_some()
    }

    pub(crate) fn close_icon_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.icon_picker.take().is_some() {
            self.focus_handle.focus(window);
            cx.notify();
        }
    }

    /// Opens the picker for `target` showing `current`, or closes it when it
    /// is already open there. The tab follows the current icon's kind.
    pub(crate) fn toggle_icon_picker(
        &mut self,
        target: IconTarget,
        current: &TemplateIcon,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .icon_picker
            .as_ref()
            .is_some_and(|picker| picker.target == target)
        {
            self.close_icon_picker(window, cx);
            return;
        }
        let theme = self.theme;
        let style = TextInputStyle {
            text: theme.foreground,
            placeholder: theme.muted_foreground,
            selection: theme.selection,
            underline_when_focused: false,
            masked: false,
        };
        let mut input = |placeholder: &'static str, cx: &mut Context<Self>| {
            cx.new(|cx| TextInput::new(placeholder, style, window, cx))
        };
        let icon_search = input("Search icons...", cx);
        let emoji_search = input("Search emoji...", cx);
        let hex_input = input("", cx);
        for search in [&icon_search, &emoji_search] {
            cx.subscribe_in(
                search,
                window,
                |this, _, event: &TextInputEvent, window, cx| match event {
                    TextInputEvent::Changed => cx.notify(),
                    TextInputEvent::Escape => this.close_icon_picker(window, cx),
                    _ => {}
                },
            )
            .detach();
        }
        cx.subscribe_in(
            &hex_input,
            window,
            |this, input, event: &TextInputEvent, window, cx| {
                match event {
                    // `HexColorInput`: every valid 6-digit value applies as typed.
                    TextInputEvent::Changed => {
                        let typed = input
                            .read(cx)
                            .text()
                            .trim()
                            .trim_start_matches('#')
                            .to_string();
                        if typed.len() == 6 && typed.chars().all(|c| c.is_ascii_hexdigit()) {
                            this.select_icon_color(format!("#{}", typed.to_lowercase()), false, cx);
                        }
                    }
                    TextInputEvent::Escape => this.close_icon_picker(window, cx),
                    _ => {}
                }
            },
        )
        .detach();
        let (tab, color, last_icon_value) = match current {
            TemplateIcon::Emoji(_) => (
                Tab::Emojis,
                "#9ca3af".to_string(),
                "notebook-tabs".to_string(),
            ),
            TemplateIcon::Icon { name, color } => (Tab::Icons, color.to_lowercase(), name.clone()),
        };
        hex_input.update(cx, |input, cx| input.set_text(color.to_uppercase(), cx));
        let hsv = hex_to_hsv(&color).unwrap_or((0.0, 0.0, 0.6));
        self.icon_picker = Some(IconPicker {
            target,
            tab,
            icon_search,
            emoji_search,
            hex_input,
            color,
            last_icon_value,
            custom_color_open: false,
            hsv,
            drag: None,
            saturation_bounds: Rc::default(),
            hue_bounds: Rc::default(),
            icons_scroll: ScrollHandle::new(),
            emojis_scroll: ScrollHandle::new(),
        });
        cx.notify();
    }

    /// `onChange`: the template form's `saveTemplate`, or `updateFolderIcon`
    /// with the sidebar's optimistic override.
    fn apply_icon(&mut self, icon: TemplateIcon, cx: &mut Context<Self>) {
        let Some(target) = self
            .icon_picker
            .as_ref()
            .map(|picker| picker.target.clone())
        else {
            return;
        };
        match target {
            IconTarget::Template(id) => {
                let next = icon.clone();
                self.update_template(&id, move |template| template.icon = next, cx);
            }
            IconTarget::Folder(path) => {
                if let Some(state) = self.folders.as_mut() {
                    match state
                        .catalog
                        .icons
                        .iter_mut()
                        .find(|(icon_path, _)| *icon_path == path)
                    {
                        Some(entry) => entry.1 = icon.clone(),
                        None => state.catalog.icons.push((path.clone(), icon.clone())),
                    }
                }
                cx.notify();
                let task = self.store.update_folder_icon(path, icon);
                cx.spawn(async move |this, cx| {
                    if let Ok(Err(error)) = task.await {
                        this.update(cx, |this, cx| {
                            this.flash(FlashVariant::Error, error.to_string(), cx)
                        })
                        .ok();
                    }
                })
                .detach();
            }
        }
    }

    /// `selectIcon`
    fn select_icon_value(&mut self, value: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(picker) = self.icon_picker.as_mut() else {
            return;
        };
        picker.last_icon_value = value.clone();
        let color = picker.color.clone();
        self.apply_icon(TemplateIcon::Icon { name: value, color }, cx);
        self.close_icon_picker(window, cx);
    }

    /// `selectColor`: the colour applies immediately to the last icon and the
    /// popover stays open. `sync_hex` refreshes the HEX field unless the
    /// change came from typing in it.
    fn select_icon_color(&mut self, color: String, sync_hex: bool, cx: &mut Context<Self>) {
        let Some(picker) = self.icon_picker.as_mut() else {
            return;
        };
        picker.color = color.clone();
        if let Some(hsv) = hex_to_hsv(&color) {
            // Keep the hue while the colour is achromatic so the pointer
            // does not jump to red (react-colorful does the same).
            if hsv.1 > 0.0 && hsv.2 > 0.0 {
                picker.hsv = hsv;
            } else {
                picker.hsv.1 = hsv.1;
                picker.hsv.2 = hsv.2;
            }
        }
        if sync_hex {
            let hex_input = picker.hex_input.clone();
            hex_input.update(cx, |input, cx| input.set_text(color.to_uppercase(), cx));
        }
        let value = picker.last_icon_value.clone();
        self.apply_icon(TemplateIcon::Icon { name: value, color }, cx);
        cx.notify();
    }

    /// `selectEmoji`: front of the recent list (24 kept), then close.
    fn select_emoji(
        &mut self,
        id: &str,
        native: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.recent_emoji_ids.retain(|recent| recent != id);
        self.recent_emoji_ids.insert(0, id.to_string());
        self.recent_emoji_ids.truncate(24);
        self.apply_icon(TemplateIcon::Emoji(native.to_string()), cx);
        self.close_icon_picker(window, cx);
    }

    fn set_hsv_from_pointer(
        &mut self,
        drag: Drag,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(picker) = self.icon_picker.as_ref() else {
            return;
        };
        let bounds = match drag {
            Drag::Saturation => picker.saturation_bounds.get(),
            Drag::Hue => picker.hue_bounds.get(),
        };
        let Some(bounds) = bounds else {
            return;
        };
        let fx =
            (f32::from(position.x - bounds.left()) / f32::from(bounds.size.width)).clamp(0.0, 1.0);
        let fy =
            (f32::from(position.y - bounds.top()) / f32::from(bounds.size.height)).clamp(0.0, 1.0);
        let (h, s, v) = picker.hsv;
        let next = match drag {
            Drag::Saturation => (h, fx, 1.0 - fy),
            Drag::Hue => (fx * 360.0, s, v),
        };
        let color = hsv_to_hex(next);
        if let Some(picker) = self.icon_picker.as_mut() {
            picker.hsv = next;
        }
        self.select_icon_color(color, true, cx);
    }

    /// The popover under a `size-7` trigger (`align="start" sideOffset={6}`),
    /// rendered only when the picker is open for `target`.
    pub(super) fn render_icon_picker(
        &self,
        target: &IconTarget,
        selected: &TemplateIcon,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let picker = self
            .icon_picker
            .as_ref()
            .filter(|picker| picker.target == *target)?;
        let theme = self.theme;

        let tabs = div()
            .flex()
            .h(px(48.0))
            .items_end()
            .gap(px(24.0))
            .border_b_1()
            .border_color(theme.border)
            .px_4()
            .children(
                [(Tab::Icons, "Icons"), (Tab::Emojis, "Emojis")]
                    .into_iter()
                    .map(|(tab, label)| {
                        let active = picker.tab == tab;
                        div()
                            .id(SharedString::from(format!("icon-picker-tab-{label}")))
                            .relative()
                            .h_full()
                            .pt_1()
                            .flex()
                            .items_center()
                            .tw_text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .cursor_pointer()
                            .when(active, |t| t.text_color(theme.foreground))
                            .when(!active, |t| {
                                t.text_color(theme.muted_foreground)
                                    .hover(move |style| style.text_color(theme.foreground))
                            })
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if let Some(picker) = this.icon_picker.as_mut() {
                                    picker.tab = tab;
                                    cx.notify();
                                }
                            }))
                            .child(label)
                            .when(active, |t| {
                                t.child(
                                    div()
                                        .absolute()
                                        .left_0()
                                        .right_0()
                                        .bottom_0()
                                        .h(px(2.0))
                                        .bg(theme.primary),
                                )
                            })
                    }),
            );

        let body = match picker.tab {
            Tab::Icons => self.render_icon_tab(picker, selected, cx),
            Tab::Emojis => self.render_emoji_tab(picker, cx),
        };

        let panel = super::menu::menu_chrome(theme, "icon-picker", POPOVER_WIDTH)
            .flex()
            .flex_col()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, window, cx| {
                this.close_icon_picker(window, cx);
            }))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                let drag = this.icon_picker.as_ref().and_then(|picker| picker.drag);
                if let Some(drag) = drag {
                    if event.pressed_button == Some(MouseButton::Left) {
                        this.set_hsv_from_pointer(drag, event.position, cx);
                    } else if let Some(picker) = this.icon_picker.as_mut() {
                        picker.drag = None;
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, _| {
                    if let Some(picker) = this.icon_picker.as_mut() {
                        picker.drag = None;
                    }
                }),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .rounded(px(20.0))
                    .child(crate::squircle::squircle(
                        crate::squircle::PANEL_RADIUS,
                        Some(theme.floating_panel),
                        Some((1.0, theme.floating_border)),
                    ))
                    .child(tabs)
                    .child(body),
            );

        Some(
            div()
                .absolute()
                .top(px(CELL + 6.0))
                .left_0()
                .child(
                    gpui::deferred(
                        gpui::anchored()
                            .anchor(gpui::Corner::TopLeft)
                            .snap_to_window_with_margin(px(8.0))
                            .child(panel),
                    )
                    .with_priority(2),
                )
                .into_any_element(),
        )
    }

    fn render_icon_tab(
        &self,
        picker: &IconPicker,
        selected: &TemplateIcon,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let current = picker.color.to_lowercase();
        let swatches = div()
            .flex()
            .items_center()
            .justify_between()
            .children(ICON_COLORS.iter().map(|color| {
                let chosen = current == *color;
                let value = color.to_string();
                div()
                    .id(SharedString::from(format!("swatch-{color}")))
                    .flex()
                    .size(px(CELL))
                    .items_center()
                    .justify_center()
                    .rounded(px(8.0))
                    .bg(super::note::parse_hex_color(color).unwrap_or(theme.muted_foreground))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(picker) = this.icon_picker.as_mut() {
                            picker.custom_color_open = false;
                        }
                        this.select_icon_color(value.clone(), true, cx);
                    }))
                    .when(chosen, |swatch| {
                        swatch.child(icon("check", px(16.0), theme.white))
                    })
            }))
            .child(div().h(px(CELL)).w(px(1.0)).bg(theme.border))
            .child(
                // The conic-gradient custom colour button, `ring-2 ring-primary
                // ring-offset-2` while its picker is open.
                div()
                    .id("swatch-custom")
                    .relative()
                    .size(px(CELL))
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if let Some(picker) = this.icon_picker.as_mut() {
                            picker.custom_color_open = !picker.custom_color_open;
                            cx.notify();
                        }
                    }))
                    .child(conic_swatch(theme.floating_panel))
                    .when(picker.custom_color_open, |button| {
                        button.child(
                            div()
                                .absolute()
                                .top(px(-4.0))
                                .left(px(-4.0))
                                .size(px(CELL + 8.0))
                                .rounded(px(12.0))
                                .border_2()
                                .border_color(theme.primary),
                        )
                    }),
            );

        let custom = picker
            .custom_color_open
            .then(|| self.render_custom_color(picker, cx));

        let query = picker.icon_search.read(cx).text().trim().to_lowercase();
        let icons: Vec<&(&str, &str)> = crate::assets::TEMPLATE_ICON_ASSETS
            .iter()
            .filter(|(value, _)| query.is_empty() || value.replace('-', " ").contains(&query))
            .collect();
        let icon_color =
            super::note::parse_hex_color(&picker.color).unwrap_or(theme.muted_foreground);
        let grid_width = PANEL_WIDTH - 24.0;
        let pitch = (grid_width - GAP * (COLUMNS as f32 - 1.0)) / COLUMNS as f32 + GAP;
        let selected_value = match selected {
            TemplateIcon::Icon { name, .. } => Some(name.as_str()),
            TemplateIcon::Emoji(_) => None,
        };
        let rows = icons.len().div_ceil(COLUMNS);
        let grid = div()
            .relative()
            .w(px(grid_width))
            .h(px(rows as f32 * (CELL + GAP) - GAP).max(px(0.0)))
            .children(icons.iter().enumerate().map(|(index, (value, asset))| {
                let column = index % COLUMNS;
                let row = index / COLUMNS;
                let value = value.to_string();
                let is_selected = selected_value == Some(value.as_str());
                div()
                    .id(SharedString::from(format!("icon-cell-{value}")))
                    .absolute()
                    .left(px(column as f32 * pitch))
                    .top(px(row as f32 * (CELL + GAP)))
                    .flex()
                    .size(px(CELL))
                    .items_center()
                    .justify_center()
                    .rounded_md()
                    .cursor_pointer()
                    .when(is_selected, |cell| cell.bg(theme.accent))
                    .hover(move |style| style.bg(theme.accent))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.select_icon_value(value.clone(), window, cx);
                    }))
                    .child(icon(asset, px(18.0), icon_color))
            }));

        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .border_b_1()
                    .border_color(theme.border)
                    .px_4()
                    .py_3()
                    .child(swatches)
                    .children(custom),
            )
            .child(self.render_picker_search(&picker.icon_search, cx))
            .child(
                div()
                    .relative()
                    .child(
                        div()
                            .id("icon-picker-icons")
                            .max_h(px(360.0))
                            .overflow_y_scroll()
                            .track_scroll(&picker.icons_scroll)
                            .pl_3()
                            .pr(px(12.0) + crate::ui::scrollbar_gutter(&picker.icons_scroll))
                            .py_3()
                            .child(grid)
                            .when(icons.is_empty(), |body| {
                                body.child(
                                    div()
                                        .py_8()
                                        .w_full()
                                        .text_center()
                                        .tw_text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child("No icons found"),
                                )
                            }),
                    )
                    .child(crate::ui::webkit_scrollbar(
                        picker.icons_scroll.clone(),
                        theme.scrollbar_thumb,
                    )),
            )
    }

    /// `HexColorInput` row and the `HexColorPicker` (`h-36 w-full`).
    fn render_custom_color(&self, picker: &IconPicker, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let (h, s, v) = picker.hsv;
        let color = super::note::parse_hex_color(&picker.color).unwrap_or(theme.muted_foreground);
        let hue_color = hsv_to_rgba((h, 1.0, 1.0));
        let width = PANEL_WIDTH - 32.0;
        let saturation_height = 144.0 - 24.0;
        let interactive_height = saturation_height - 12.0;
        let saturation_bounds = picker.saturation_bounds.clone();
        let hue_bounds = picker.hue_bounds.clone();
        let focus_hex = picker.hex_input.clone();

        let pointer = |x: f32, y: f32, fill: gpui::Rgba| {
            div()
                .absolute()
                .left(px(x - 14.0))
                .top(px(y - 14.0))
                .size(px(28.0))
                .rounded_full()
                .border_2()
                .border_color(theme.white)
                .bg(fill)
                .shadow(vec![gpui::BoxShadow {
                    color: gpui::hsla(0.0, 0.0, 0.0, 0.2),
                    offset: point(px(0.0), px(2.0)),
                    blur_radius: px(4.0),
                    spread_radius: px(0.0),
                }])
        };

        let saturation = div()
            .id("color-saturation")
            .relative()
            .w_full()
            .h(px(saturation_height))
            .rounded_tl(px(8.0))
            .rounded_tr(px(8.0))
            .border_b(px(12.0))
            .border_color(gpui::black())
            .bg(hue_color)
            .cursor_default()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    if let Some(picker) = this.icon_picker.as_mut() {
                        picker.drag = Some(Drag::Saturation);
                    }
                    this.set_hsv_from_pointer(Drag::Saturation, event.position, cx);
                }),
            )
            .child(div().absolute().inset_0().bg(linear_gradient(
                90.0,
                linear_color_stop(gpui::white(), 0.0),
                linear_color_stop(alpha(gpui::rgb(0xffffff), 0.0), 1.0),
            )))
            .child(div().absolute().inset_0().bg(linear_gradient(
                0.0,
                linear_color_stop(gpui::black(), 0.0),
                linear_color_stop(alpha(gpui::rgb(0x000000), 0.0), 1.0),
            )))
            .child(
                canvas(
                    move |bounds, _, _| saturation_bounds.set(Some(bounds)),
                    |_, _, _, _| (),
                )
                .absolute()
                .inset_0(),
            )
            .child(pointer(s * width, (1.0 - v) * interactive_height, color));

        let hue_stops = [
            (0xff0000, 0xffff00),
            (0xffff00, 0x00ff00),
            (0x00ff00, 0x00ffff),
            (0x00ffff, 0x0000ff),
            (0x0000ff, 0xff00ff),
            (0xff00ff, 0xff0000),
        ];
        let hue = div()
            .id("color-hue")
            .relative()
            .w_full()
            .h(px(24.0))
            .rounded_bl(px(8.0))
            .rounded_br(px(8.0))
            .overflow_hidden()
            .flex()
            .cursor_default()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    if let Some(picker) = this.icon_picker.as_mut() {
                        picker.drag = Some(Drag::Hue);
                    }
                    this.set_hsv_from_pointer(Drag::Hue, event.position, cx);
                }),
            )
            .children(hue_stops.into_iter().map(|(from, to)| {
                div().flex_1().h_full().bg(linear_gradient(
                    90.0,
                    linear_color_stop(gpui::rgb(from), 0.0),
                    linear_color_stop(gpui::rgb(to), 1.0),
                ))
            }))
            .child(
                canvas(
                    move |bounds, _, _| hue_bounds.set(Some(bounds)),
                    |_, _, _, _| (),
                )
                .absolute()
                .inset_0(),
            )
            .child(pointer(h / 360.0 * width, 12.0, hue_color));

        div()
            .mt_3()
            .flex()
            .flex_col()
            .child(
                div()
                    .mb_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().size(px(24.0)).rounded(px(8.0)).bg(color))
                    .child(
                        div()
                            .tw_text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.muted_foreground)
                            .child("HEX"),
                    )
                    .child(
                        div()
                            .id("color-hex")
                            .min_w_0()
                            .flex_1()
                            .tw_text_sm()
                            .cursor_text()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| {
                                focus_hex.read(cx).focus_handle(cx).focus(window);
                            }))
                            .child(picker.hex_input.clone()),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .w_full()
                    .h(px(144.0))
                    .child(saturation)
                    .child(hue),
            )
    }

    /// `SearchField`: `h-12 border-b px-4 gap-2`.
    fn render_picker_search(
        &self,
        input: &Entity<TextInput>,
        cx: &Context<Self>,
    ) -> gpui::Stateful<Div> {
        let theme = self.theme;
        let has_text = !input.read(cx).text().is_empty();
        let focus = input.clone();
        let clear = input.clone();
        div()
            .id(SharedString::from(format!(
                "picker-search-{}",
                input.entity_id()
            )))
            .flex()
            .h(px(48.0))
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(theme.border)
            .px_4()
            .cursor_text()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| {
                focus.read(cx).focus_handle(cx).focus(window);
            }))
            .child(icon("search", px(16.0), theme.muted_foreground))
            .child(div().min_w_0().flex_1().tw_text_sm().child(input.clone()))
            .when(has_text, |row| {
                row.child(
                    div()
                        .id("picker-search-clear")
                        .rounded(px(4.0))
                        .p_1()
                        .cursor_pointer()
                        .hover(move |style| style.bg(theme.accent))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                            clear.update(cx, |input, cx| input.set_text("", cx));
                        }))
                        .child(icon("x", px(14.0), theme.muted_foreground)),
                )
            })
    }

    /// The emoji tab: the search row, then `Frequently used` and the
    /// categories in a virtualised `px-4 py-3` scroller (`max-h-[480px]`).
    fn render_emoji_tab(&self, picker: &IconPicker, cx: &Context<Self>) -> Div {
        let theme = self.theme;
        let query = picker.emoji_search.read(cx).text().trim().to_lowercase();
        let mut sections: Vec<(&'static str, Vec<&'static crate::emoji::Emoji>)> = Vec::new();
        if query.is_empty() {
            let mut ids: Vec<&str> = self.recent_emoji_ids.iter().map(String::as_str).collect();
            for id in crate::emoji::FREQUENT_IDS {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            sections.push((
                "Frequently used",
                ids.into_iter().filter_map(crate::emoji::by_id).collect(),
            ));
        }
        for category in crate::emoji::CATEGORIES.iter() {
            let emojis: Vec<&'static crate::emoji::Emoji> = category
                .emojis
                .iter()
                .copied()
                .filter(|emoji| query.is_empty() || emoji.search.contains(&query))
                .collect();
            if !emojis.is_empty() {
                sections.push((crate::emoji::category_label(category.id), emojis));
            }
        }

        let grid_width = PANEL_WIDTH - 32.0;
        let pitch = (grid_width - GAP * (COLUMNS as f32 - 1.0)) / COLUMNS as f32 + GAP;
        // Row model with cumulative offsets so only the visible rows render.
        enum Row {
            Header(&'static str),
            Emojis(Vec<&'static crate::emoji::Emoji>),
            Gap(f32),
        }
        let mut rows: Vec<(f32, f32, Row)> = Vec::new();
        let mut y = 0.0;
        for (index, (title, emojis)) in sections.iter().enumerate() {
            rows.push((y, 26.0, Row::Header(title)));
            y += 26.0;
            for (row_index, chunk) in emojis.chunks(COLUMNS).enumerate() {
                if row_index > 0 {
                    rows.push((y, GAP, Row::Gap(GAP)));
                    y += GAP;
                }
                rows.push((y, CELL, Row::Emojis(chunk.to_vec())));
                y += CELL;
            }
            if index + 1 < sections.len() {
                rows.push((y, 16.0, Row::Gap(16.0)));
                y += 16.0;
            }
        }
        let total = y;
        let scroll_top = f32::from(-picker.emojis_scroll.offset().y).max(0.0);
        let viewport = 480.0;
        let visible_top = scroll_top - 64.0;
        let visible_bottom = scroll_top + viewport + 64.0;
        let mut children: Vec<AnyElement> = Vec::new();
        let mut spacer = 0.0;
        let flush_spacer = |children: &mut Vec<AnyElement>, spacer: &mut f32| {
            if *spacer > 0.0 {
                children.push(div().h(px(*spacer)).flex_shrink_0().into_any_element());
                *spacer = 0.0;
            }
        };
        for (top, height, row) in rows {
            if top + height < visible_top || top > visible_bottom {
                spacer += height;
                continue;
            }
            flush_spacer(&mut children, &mut spacer);
            match row {
                Row::Header(title) => children.push(
                    div()
                        .h(px(26.0))
                        .flex_shrink_0()
                        .tw_text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme.muted_foreground)
                        .child(title)
                        .into_any_element(),
                ),
                Row::Gap(gap) => children.push(div().h(px(gap)).flex_shrink_0().into_any_element()),
                Row::Emojis(emojis) => children.push(
                    div()
                        .relative()
                        .h(px(CELL))
                        .w(px(grid_width))
                        .flex_shrink_0()
                        .children(emojis.into_iter().enumerate().map(|(column, emoji)| {
                            let id = emoji.id;
                            let native = emoji.native;
                            div()
                                .id(SharedString::from(format!("emoji-{id}")))
                                .absolute()
                                .left(px(column as f32 * pitch))
                                .top_0()
                                .flex()
                                .size(px(CELL))
                                .items_center()
                                .justify_center()
                                .rounded_md()
                                .tw_text_lg()
                                .cursor_pointer()
                                .hover(move |style| style.bg(theme.accent))
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                    this.select_emoji(id, native, window, cx);
                                }))
                                .child(native)
                        }))
                        .into_any_element(),
                ),
            }
        }
        flush_spacer(&mut children, &mut spacer);
        let _ = total;

        div()
            .flex()
            .flex_col()
            .child(self.render_picker_search(&picker.emoji_search, cx))
            .child(
                div()
                    .relative()
                    .child(
                        div()
                            .id("icon-picker-emojis")
                            .max_h(px(viewport))
                            .overflow_y_scroll()
                            .track_scroll(&picker.emojis_scroll)
                            .pl_4()
                            .pr(px(16.0) + crate::ui::scrollbar_gutter(&picker.emojis_scroll))
                            .py_3()
                            .flex()
                            .flex_col()
                            .children(children)
                            .when(sections.is_empty(), |body| {
                                body.child(
                                    div()
                                        .py_8()
                                        .w_full()
                                        .text_center()
                                        .tw_text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child("No emoji found"),
                                )
                            }),
                    )
                    .child(crate::ui::webkit_scrollbar(
                        picker.emojis_scroll.clone(),
                        theme.scrollbar_thumb,
                    )),
            )
    }
}

/// `bg-[conic-gradient(from_180deg,red,#ff0,#0f0,#0ff,#00f,#f0f,red)]` on a
/// `size-7 rounded-full` (8px) button: hue wedges around the centre with the
/// corners masked back to the panel colour.
fn conic_swatch(mask: gpui::Rgba) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let center = bounds.center();
            let radius = f32::from(bounds.size.width);
            let steps = 72;
            window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
                for step in 0..steps {
                    // CSS conic gradients start at the top and run clockwise;
                    // `from 180deg` starts at the bottom.
                    let start =
                        std::f32::consts::PI + step as f32 / steps as f32 * std::f32::consts::TAU;
                    let end = start + std::f32::consts::TAU / steps as f32 + 0.01;
                    let hue = step as f32 / steps as f32 * 360.0;
                    let mut path = PathBuilder::fill();
                    path.move_to(center);
                    path.line_to(point(
                        center.x + px(radius * start.sin()),
                        center.y - px(radius * start.cos()),
                    ));
                    path.line_to(point(
                        center.x + px(radius * end.sin()),
                        center.y - px(radius * end.cos()),
                    ));
                    path.close();
                    if let Ok(path) = path.build() {
                        window.paint_path(path, hsv_to_rgba((hue, 1.0, 1.0)));
                    }
                }
            });
            // Mask outside the rounded square: four corner pieces.
            let r = 8.0f32;
            let corners = [
                (bounds.left(), bounds.top(), 1.0, 1.0),
                (bounds.right(), bounds.top(), -1.0, 1.0),
                (bounds.right(), bounds.bottom(), -1.0, -1.0),
                (bounds.left(), bounds.bottom(), 1.0, -1.0),
            ];
            let k = 0.5523f32;
            for (x, y, sx, sy) in corners {
                let mut path = PathBuilder::fill();
                path.move_to(point(x, y));
                path.line_to(point(x + px(r * sx), y));
                path.cubic_bezier_to(
                    point(x, y + px(r * sy)),
                    point(x + px(r * sx * (1.0 - k)), y),
                    point(x, y + px(r * sy * (1.0 - k))),
                );
                path.close();
                if let Ok(path) = path.build() {
                    window.paint_path(path, mask);
                }
            }
        },
    )
    .size_full()
}

fn hex_to_hsv(hex: &str) -> Option<(f32, f32, f32)> {
    let hex = hex.trim().strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let channel = |i: usize| {
        u8::from_str_radix(&hex[i..i + 2], 16)
            .ok()
            .map(|v| v as f32 / 255.0)
    };
    let (r, g, b) = (channel(0)?, channel(2)?, channel(4)?);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let h = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    let h = if h < 0.0 { h + 360.0 } else { h };
    let s = if max == 0.0 { 0.0 } else { delta / max };
    Some((h, s, max))
}

fn hsv_to_rgb((h, s, v): (f32, f32, f32)) -> (f32, f32, f32) {
    let h = (h % 360.0 + 360.0) % 360.0 / 60.0;
    let i = h.floor();
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match i as i32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

fn hsv_to_rgba(hsv: (f32, f32, f32)) -> gpui::Rgba {
    let (r, g, b) = hsv_to_rgb(hsv);
    gpui::Rgba { r, g, b, a: 1.0 }
}

/// react-colorful's `hsvaToHex`: rounded channels, lowercase.
fn hsv_to_hex(hsv: (f32, f32, f32)) -> String {
    let (r, g, b) = hsv_to_rgb(hsv);
    format!(
        "#{:02x}{:02x}{:02x}",
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips_through_hsv() {
        for hex in ICON_COLORS {
            let hsv = hex_to_hsv(hex).unwrap();
            assert_eq!(hsv_to_hex(hsv), hex, "{hex}");
        }
        assert_eq!(hex_to_hsv("#ff0000"), Some((0.0, 1.0, 1.0)));
        assert_eq!(hsv_to_hex((120.0, 1.0, 1.0)), "#00ff00");
        assert_eq!(hex_to_hsv("nope"), None);
    }
}
