//! `TemplateIconPicker`'s popover: colored icons or emoji for templates and folders.

use std::sync::{Arc, OnceLock};

use gpui::{
    AnyElement, Context, Entity, EventEmitter, Focusable, Render, SharedString, Subscription,
    Window, div, img, prelude::*, px, rgb,
};

use super::{
    catalog_icons::{EMOJI, TEMPLATE_ICONS},
    resource_list::{Glyph, icon},
};
use crate::ui::{
    input::{InputEvent, TextInput},
    theme::{Theme, theme},
};

const COLORS: [u32; 9] = [
    0x9ca3af, 0x94a3b8, 0x5b67d8, 0x25b5c9, 0x4ab883, 0xf2bd00, 0xef923d, 0xc99b92, 0xf05257,
];

const FREQUENT: [&str; 22] = [
    "ok_hand",
    "heart",
    "white_check_mark",
    "+1",
    "pray",
    "joy",
    "eyes",
    "slightly_smiling_face",
    "grinning",
    "smile",
    "thinking_face",
    "sweat_smile",
    "warning",
    "confused",
    "x",
    "raised_hands",
    "tada",
    "wink",
    "blush",
    "shrug",
    "wave",
    "question",
];

const CATEGORIES: [(&str, &str); 8] = [
    ("people", "Smileys & People"),
    ("nature", "Animals & Nature"),
    ("foods", "Food & Drink"),
    ("activity", "Activity"),
    ("places", "Travel & Places"),
    ("objects", "Objects"),
    ("symbols", "Symbols"),
    ("flags", "Flags"),
];

struct Emoji {
    category: &'static str,
    id: &'static str,
    native: &'static str,
    search: &'static str,
}

fn emoji() -> &'static [Emoji] {
    static TABLE: OnceLock<Vec<Emoji>> = OnceLock::new();
    TABLE.get_or_init(|| {
        EMOJI
            .lines()
            .filter_map(|line| {
                let mut fields = line.split('\t');
                Some(Emoji {
                    category: fields.next()?,
                    id: fields.next()?,
                    native: fields.next()?,
                    search: fields.nth(1)?,
                })
            })
            .collect()
    })
}

pub enum IconPickerEvent {
    /// A new icon; `close` when the choice also dismisses the popover.
    Changed { icon: Glyph, close: bool },
}

impl EventEmitter<IconPickerEvent> for IconPicker {}

pub struct IconPicker {
    emojis: bool,
    color: u32,
    last_icon: Arc<str>,
    selected: Glyph,
    icon_search: Entity<TextInput>,
    emoji_search: Entity<TextInput>,
    hex: Option<Entity<TextInput>>,
    recent: Vec<&'static str>,
    _subscriptions: Vec<Subscription>,
}

impl IconPicker {
    pub fn new(selected: Glyph, cx: &mut Context<Self>) -> Self {
        let icon_search = cx.new(|cx| TextInput::new("Search icons...", cx).inline());
        let emoji_search = cx.new(|cx| TextInput::new("Search emoji...", cx).inline());
        let subscriptions = [&icon_search, &emoji_search]
            .map(|input| cx.subscribe(input, |_, _, _: &InputEvent, cx| cx.notify()))
            .into();
        let (color, last_icon) = match &selected {
            Glyph::Icon { key, color } => (*color, key.clone()),
            Glyph::Emoji(_) => (0x9ca3af, "notebook-tabs".into()),
        };
        Self {
            emojis: matches!(selected, Glyph::Emoji(_)),
            color,
            last_icon,
            selected,
            icon_search,
            emoji_search,
            hex: None,
            recent: Vec::new(),
            _subscriptions: subscriptions,
        }
    }

    fn change(&mut self, icon: Glyph, close: bool, cx: &mut Context<Self>) {
        self.selected = icon.clone();
        cx.emit(IconPickerEvent::Changed { icon, close });
        cx.notify();
    }

    fn select_color(&mut self, color: u32, cx: &mut Context<Self>) {
        self.color = color;
        let icon = Glyph::Icon {
            key: self.last_icon.clone(),
            color,
        };
        self.change(icon, false, cx);
    }

    fn toggle_custom(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.hex.take().is_some() {
            cx.notify();
            return;
        }
        let color = self.color;
        let hex = cx.new(|cx| {
            let mut input = TextInput::new("", cx).inline();
            input.set_text(format!("#{color:06X}"), cx);
            input
        });
        self._subscriptions
            .push(cx.subscribe(&hex, |this, input, event, cx| {
                if matches!(event, InputEvent::Changed) {
                    let text = input.read(cx).buffer.text.clone();
                    let digits = text.trim().trim_start_matches('#');
                    if digits.len() == 6
                        && let Ok(color) = u32::from_str_radix(digits, 16)
                    {
                        this.select_color(color, cx);
                    }
                }
            }));
        hex.focus_handle(cx).focus(window, cx);
        self.hex = Some(hex);
        cx.notify();
    }

    fn search_field(input: &Entity<TextInput>, colors: Theme, cx: &mut Context<Self>) -> gpui::Div {
        let has_text = !input.read(cx).buffer.text.is_empty();
        let clear = input.clone();
        div()
            .h(px(48.))
            .px_4()
            .flex()
            .items_center()
            .gap_2()
            .border_b_1()
            .border_color(colors.border)
            .child(icon("Search01Icon", 16., colors.muted_foreground))
            .child(div().flex_1().min_w_0().text_sm().child(input.clone()))
            .when(has_text, |row| {
                row.child(
                    div()
                        .id("picker-clear")
                        .p_1()
                        .rounded(px(4.))
                        .cursor_pointer()
                        .hover(|style| style.bg(colors.accent))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            clear.update(cx, |input, cx| input.set_text(String::new(), cx));
                        }))
                        .child(icon("XIcon", 14., colors.muted_foreground)),
                )
            })
    }

    fn render_icons(&self, colors: Theme, cx: &mut Context<Self>) -> AnyElement {
        let query = self.icon_search.read(cx).buffer.text.trim().to_lowercase();
        let icons = TEMPLATE_ICONS
            .iter()
            .filter(|(key, _)| query.is_empty() || key.replace('-', " ").contains(&query))
            .collect::<Vec<_>>();
        let color = self.color;
        div()
            .child(
                div()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .children(COLORS.iter().map(|swatch| {
                                let swatch = *swatch;
                                div()
                                    .id(SharedString::from(format!("color-{swatch:06x}")))
                                    .size(px(28.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(8.))
                                    .bg(rgb(swatch))
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.hex = None;
                                        this.select_color(swatch, cx);
                                    }))
                                    .when(color == swatch, |view| {
                                        view.child(icon("CheckIcon", 16., rgb(0xffffff).into()))
                                    })
                            }))
                            .child(div().w(px(1.)).h(px(28.)).bg(colors.border))
                            .child(
                                div()
                                    .id("custom-color")
                                    .size(px(28.))
                                    .rounded(px(8.))
                                    .overflow_hidden()
                                    .cursor_pointer()
                                    .when(self.hex.is_some(), |view| {
                                        view.border_2().border_color(colors.primary)
                                    })
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.toggle_custom(window, cx)
                                    }))
                                    .child(img("catalog/ColorWheel.svg").size_full()),
                            ),
                    )
                    .when_some(self.hex.clone(), |view, hex| {
                        view.child(
                            div()
                                .mt_3()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(div().size(px(24.)).rounded(px(8.)).bg(rgb(color)))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(colors.muted_foreground)
                                        .child("HEX"),
                                )
                                .child(div().flex_1().min_w_0().text_sm().child(hex)),
                        )
                    }),
            )
            .child(Self::search_field(&self.icon_search, colors, cx))
            .child(
                div()
                    .id("picker-icons")
                    .max_h(px(360.))
                    .overflow_y_scroll()
                    .p_3()
                    .child(
                        div()
                            .grid()
                            .grid_cols(12)
                            .gap_1()
                            .children(icons.into_iter().map(|(key, name)| {
                                let selected = matches!(
                                    &self.selected,
                                    Glyph::Icon { key: current, .. } if current.as_ref() == *key
                                );
                                let key: Arc<str> = (*key).into();
                                div()
                                    .id(SharedString::from(format!("icon-{key}")))
                                    .size(px(28.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(6.))
                                    .cursor_pointer()
                                    .when(selected, |view| view.bg(colors.accent))
                                    .hover(|style| style.bg(colors.accent))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.last_icon = key.clone();
                                        let icon = Glyph::Icon {
                                            key: key.clone(),
                                            color: this.color,
                                        };
                                        this.change(icon, true, cx);
                                    }))
                                    .child(icon(name, 18., rgb(color).into()))
                            })),
                    )
                    .when(
                        !query.is_empty()
                            && TEMPLATE_ICONS
                                .iter()
                                .all(|(key, _)| !key.replace('-', " ").contains(&query)),
                        |view| view.child(empty("No icons found", colors)),
                    ),
            )
            .into_any_element()
    }

    fn render_emojis(&self, colors: Theme, cx: &mut Context<Self>) -> AnyElement {
        let query = self.emoji_search.read(cx).buffer.text.trim().to_lowercase();
        let table = emoji();
        let mut sections = Vec::new();
        if query.is_empty() {
            let mut ids = self.recent.clone();
            for id in FREQUENT {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            let frequent = ids
                .into_iter()
                .filter_map(|id| table.iter().find(|emoji| emoji.id == id))
                .collect::<Vec<_>>();
            sections.push(("Frequently used", frequent));
        }
        for (category, label) in CATEGORIES {
            let items = table
                .iter()
                .filter(|emoji| {
                    emoji.category == category
                        && (query.is_empty() || emoji.search.contains(&query))
                })
                .collect::<Vec<_>>();
            if !items.is_empty() {
                sections.push((label, items));
            }
        }
        let found = sections
            .iter()
            .any(|(label, _)| *label != "Frequently used");
        div()
            .child(Self::search_field(&self.emoji_search, colors, cx))
            .child(
                div()
                    .id("picker-emoji")
                    .max_h(px(480.))
                    .overflow_y_scroll()
                    .px_4()
                    .py_3()
                    .children(
                        sections
                            .into_iter()
                            .enumerate()
                            .map(|(index, (label, items))| {
                                div()
                                    .when(index > 0, |view| view.mt_4())
                                    .child(
                                        div()
                                            .mb(px(6.))
                                            .text_sm()
                                            .line_height(px(20.))
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .text_color(colors.muted_foreground)
                                            .child(label),
                                    )
                                    .child(div().grid().grid_cols(12).gap_1().children(
                                        items.into_iter().map(|emoji| {
                                            let (id, native) = (emoji.id, emoji.native);
                                            div()
                                                .id(SharedString::from(format!("{label}-{id}")))
                                                .size(px(28.))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .rounded(px(6.))
                                                .text_lg()
                                                .cursor_pointer()
                                                .hover(|style| style.bg(colors.accent))
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.recent.retain(|recent| *recent != id);
                                                    this.recent.insert(0, id);
                                                    this.recent.truncate(24);
                                                    this.change(
                                                        Glyph::Emoji(native.into()),
                                                        true,
                                                        cx,
                                                    );
                                                }))
                                                .child(native)
                                        }),
                                    ))
                            }),
                    )
                    .when(!found, |view| view.child(empty("No emoji found", colors))),
            )
            .into_any_element()
    }
}

fn empty(message: &'static str, colors: Theme) -> gpui::Div {
    div()
        .py_8()
        .text_center()
        .text_sm()
        .text_color(colors.muted_foreground)
        .child(message)
}

impl Render for IconPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let tab = |label: &'static str, emojis: bool, active: bool, cx: &mut Context<Self>| {
            div()
                .id(label)
                .relative()
                .h_full()
                .pt_1()
                .flex()
                .items_center()
                .text_sm()
                .font_weight(gpui::FontWeight::MEDIUM)
                .cursor_pointer()
                .text_color(if active {
                    colors.foreground
                } else {
                    colors.muted_foreground
                })
                .hover(|style| style.text_color(colors.foreground))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.emojis = emojis;
                    cx.notify();
                }))
                .child(label)
                .when(active, |view| {
                    view.child(
                        div()
                            .absolute()
                            .bottom_0()
                            .left_0()
                            .right_0()
                            .h(px(2.))
                            .bg(colors.primary),
                    )
                })
        };
        let content = if self.emojis {
            self.render_emojis(colors, cx)
        } else {
            self.render_icons(colors, cx)
        };
        div()
            .w(px(420.))
            .p(px(2.))
            .rounded(px(22.))
            .border_1()
            .border_color(colors.app_floating_border)
            .bg(colors.app_floating_chrome)
            .shadow_lg()
            .child(
                div()
                    .overflow_hidden()
                    .rounded(px(20.))
                    .border_1()
                    .border_color(colors.app_floating_border)
                    .bg(colors.app_floating_panel)
                    .text_color(colors.popover_foreground)
                    .child(
                        div()
                            .h(px(48.))
                            .px_4()
                            .flex()
                            .items_end()
                            .gap_6()
                            .border_b_1()
                            .border_color(colors.border)
                            .child(tab("Icons", false, !self.emojis, cx))
                            .child(tab("Emojis", true, self.emojis, cx)),
                    )
                    .child(content),
            )
    }
}
