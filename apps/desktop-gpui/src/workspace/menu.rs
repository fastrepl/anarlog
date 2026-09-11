//! `DropdownMenuContent variant="app"` menus: the `rounded-[22px] p-0.5`
//! chrome around an `AppFloatingPanel` (`rounded-[20px] p-1.5`), items with
//! `rounded-[14px] px-2 py-1.5 text-sm gap-2`, and submenus that open to the
//! right of their trigger on hover.

use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, BoxShadow, ClickEvent, Context, Corner, KeyBinding, KeyDownEvent, MouseButton,
    MouseDownEvent, Pixels, Point, SharedString, Window, actions, anchored, deferred, div, hsla,
    point, prelude::*, px,
};

use super::Workspace;
use crate::theme::Theme;
use crate::ui::{TailwindText as _, icon};

actions!(
    menu,
    [Up, Down, Home, End, Activate, Right, Left, Escape, Tab]
);

const KEY_CONTEXT: &str = "Menu";

/// Radix `Menu`'s keys: arrows walk the enabled items without looping, Home
/// / End jump, Enter / Space select, Right opens a submenu on its first item,
/// Left closes one, Escape dismisses, and Tab is swallowed while a menu is
/// open.
pub fn bind_keys(cx: &mut App) {
    let ctx = Some(KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("up", Up, ctx),
        KeyBinding::new("down", Down, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("enter", Activate, ctx),
        KeyBinding::new("space", Activate, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("escape", Escape, ctx),
        KeyBinding::new("tab", Tab, ctx),
        KeyBinding::new("shift-tab", Tab, ctx),
    ]);
}

/// Radix's typeahead forgets its search after a second.
const TYPEAHEAD_TIMEOUT: Duration = Duration::from_secs(1);

/// One item of the open menu as the last render put it on screen, so the
/// keys can act on it.
pub(crate) struct RuntimeItem {
    pub label: SharedString,
    pub enabled: bool,
    pub submenu: bool,
    /// Closes the menu and runs the item (a submenu trigger opens it instead).
    pub activate: Run,
}

/// The open menu's items; `None` entries are separators, so indices match
/// the spec's entries (and `open_sub`).
#[derive(Default)]
pub(crate) struct MenuRuntime {
    pub id: &'static str,
    pub items: Vec<Option<RuntimeItem>>,
    /// The open `Submenu::Entries`.
    pub sub_items: Option<Vec<Option<RuntimeItem>>>,
    pub set_sub: Option<SetSub>,
}

/// Radix's roving focus inside the open menu: the highlighted item per
/// level (`data-highlighted`), which the pointer and the keys both move, and
/// the typeahead search.
#[derive(Default)]
pub(crate) struct MenuKeyboard {
    pub menu: Option<&'static str>,
    pub highlighted: Option<usize>,
    /// The submenu has the focus.
    pub in_sub: bool,
    pub sub_highlighted: Option<usize>,
    /// The submenu was opened from the keyboard: its first item is focused
    /// once it renders.
    pub sub_first: bool,
    typeahead: String,
    typeahead_at: Option<Instant>,
}

impl MenuKeyboard {
    pub fn is_highlighted(&self, id: &'static str, index: usize) -> bool {
        self.menu == Some(id) && self.highlighted == Some(index)
    }

    pub fn is_sub_highlighted(&self, id: &'static str, index: usize, first: Option<usize>) -> bool {
        self.menu == Some(id)
            && self.in_sub
            && self
                .sub_highlighted
                .or(if self.sub_first { first } else { None })
                == Some(index)
    }
}

fn enabled_indices(items: &[Option<RuntimeItem>]) -> Vec<usize> {
    items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.as_ref().is_some_and(|item| item.enabled))
        .map(|(index, _)| index)
        .collect()
}

pub(crate) fn first_enabled(items: &[Option<RuntimeItem>]) -> Option<usize> {
    enabled_indices(items).first().copied()
}

/// `getNextMatch`: the items after the current one first (wrapping), a
/// repeated letter cycling through its matches, the current item excluded
/// for a single-letter search.
fn typeahead_match(
    items: &[Option<RuntimeItem>],
    current: Option<usize>,
    search: &str,
) -> Option<usize> {
    let mut chars = search.chars();
    let first = chars.next()?;
    let repeated = search.chars().count() > 1 && chars.all(|c| c == first);
    let normalized: String = if repeated {
        first.to_lowercase().collect()
    } else {
        search.to_lowercase()
    };
    let indices = enabled_indices(items);
    let start = current
        .and_then(|current| indices.iter().position(|&index| index == current))
        .unwrap_or(0);
    let exclude_current = normalized.chars().count() == 1;
    indices
        .iter()
        .cycle()
        .skip(start)
        .take(indices.len())
        .copied()
        .filter(|&index| !(exclude_current && Some(index) == current))
        .find(|&index| {
            items[index]
                .as_ref()
                .is_some_and(|item| item.label.to_lowercase().starts_with(&normalized))
        })
}

impl Workspace {
    /// The runtime recorded by this frame's menu render, when a menu is open.
    fn with_menu_runtime<R>(&self, f: impl FnOnce(&MenuRuntime) -> R) -> Option<R> {
        self.menu_runtime.borrow().as_ref().map(f)
    }

    /// Whether the keys act on the submenu list.
    fn menu_level_is_sub(&self) -> bool {
        self.menu_keyboard.in_sub && self.with_menu_runtime(|r| r.sub_items.is_some()) == Some(true)
    }

    fn menu_step(&mut self, delta: isize, cx: &mut Context<Self>) {
        let sub = self.menu_level_is_sub();
        let (indices, current) = match self.with_menu_runtime(|runtime| {
            if sub {
                let items = runtime.sub_items.as_deref().unwrap_or(&[]);
                let current =
                    self.menu_keyboard
                        .sub_highlighted
                        .or(if self.menu_keyboard.sub_first {
                            first_enabled(items)
                        } else {
                            None
                        });
                (enabled_indices(items), current)
            } else {
                (
                    enabled_indices(&runtime.items),
                    self.menu_keyboard.highlighted,
                )
            }
        }) {
            Some(level) => level,
            None => return,
        };
        if indices.is_empty() {
            return;
        }
        // `loop={false}`: the ends stay put.
        let next = match current.and_then(|current| indices.iter().position(|&i| i == current)) {
            Some(position) => {
                indices[(position as isize + delta).clamp(0, indices.len() as isize - 1) as usize]
            }
            None if delta > 0 => indices[0],
            None => indices[indices.len() - 1],
        };
        self.menu_set_highlight(sub, Some(next), cx);
    }

    fn menu_jump(&mut self, to_end: bool, cx: &mut Context<Self>) {
        let sub = self.menu_level_is_sub();
        let Some(indices) = self.with_menu_runtime(|runtime| {
            if sub {
                enabled_indices(runtime.sub_items.as_deref().unwrap_or(&[]))
            } else {
                enabled_indices(&runtime.items)
            }
        }) else {
            return;
        };
        let target = if to_end {
            indices.last().copied()
        } else {
            indices.first().copied()
        };
        if target.is_some() {
            self.menu_set_highlight(sub, target, cx);
        }
    }

    fn menu_set_highlight(&mut self, sub: bool, index: Option<usize>, cx: &mut Context<Self>) {
        if sub {
            self.menu_keyboard.sub_highlighted = index;
            self.menu_keyboard.sub_first = false;
        } else {
            self.menu_keyboard.highlighted = index;
        }
        cx.notify();
    }

    /// The pointer over an item focuses it, like Radix's `onPointerMove`;
    /// leaving hands the focus back to the content.
    pub(crate) fn menu_hover(
        &mut self,
        id: &'static str,
        sub: bool,
        index: usize,
        hovered: bool,
        cx: &mut Context<Self>,
    ) {
        if self.menu_keyboard.menu != Some(id) {
            self.menu_keyboard = MenuKeyboard {
                menu: Some(id),
                ..Default::default()
            };
        }
        if hovered {
            self.menu_keyboard.in_sub = sub;
            if sub {
                self.menu_keyboard.sub_highlighted = Some(index);
                self.menu_keyboard.sub_first = false;
            } else {
                self.menu_keyboard.highlighted = Some(index);
                self.menu_keyboard.sub_highlighted = None;
                self.menu_keyboard.sub_first = false;
            }
            cx.notify();
        } else if sub {
            if self.menu_keyboard.sub_highlighted == Some(index) {
                self.menu_keyboard.sub_highlighted = None;
                cx.notify();
            }
        } else if self.menu_keyboard.highlighted == Some(index) && !self.menu_keyboard.in_sub {
            self.menu_keyboard.highlighted = None;
            cx.notify();
        }
    }

    /// Enter / Space / Right on a submenu trigger opens it on its first item.
    fn menu_open_sub(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(set_sub) = self.with_menu_runtime(|runtime| runtime.set_sub).flatten() else {
            return;
        };
        set_sub(self, Some(index), cx);
        self.menu_keyboard.highlighted = Some(index);
        self.menu_keyboard.in_sub = true;
        self.menu_keyboard.sub_highlighted = None;
        self.menu_keyboard.sub_first = true;
        cx.notify();
    }

    fn menu_up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.menu_step(-1, cx);
    }

    fn menu_down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.menu_step(1, cx);
    }

    fn menu_home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.menu_jump(false, cx);
    }

    fn menu_end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.menu_jump(true, cx);
    }

    fn menu_activate(&mut self, _: &Activate, window: &mut Window, cx: &mut Context<Self>) {
        let sub = self.menu_level_is_sub();
        let Some((activate, submenu, index)) = self
            .with_menu_runtime(|runtime| {
                let (items, current) = if sub {
                    let items = runtime.sub_items.as_deref().unwrap_or(&[]);
                    let current =
                        self.menu_keyboard
                            .sub_highlighted
                            .or(if self.menu_keyboard.sub_first {
                                first_enabled(items)
                            } else {
                                None
                            });
                    (items, current)
                } else {
                    (&runtime.items[..], self.menu_keyboard.highlighted)
                };
                let index = current?;
                let item = items.get(index)?.as_ref()?;
                item.enabled
                    .then(|| (item.activate.clone(), item.submenu, index))
            })
            .flatten()
        else {
            return;
        };
        if submenu && !sub {
            self.menu_open_sub(index, cx);
            return;
        }
        activate(self, window, cx);
    }

    fn menu_right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.menu_level_is_sub() {
            return;
        }
        let Some(index) = self.menu_keyboard.highlighted else {
            return;
        };
        let is_sub = self
            .with_menu_runtime(|runtime| {
                runtime
                    .items
                    .get(index)
                    .and_then(|item| item.as_ref())
                    .is_some_and(|item| item.submenu && item.enabled)
            })
            .unwrap_or(false);
        if is_sub {
            self.menu_open_sub(index, cx);
        }
    }

    fn menu_left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if !self.menu_keyboard.in_sub {
            return;
        }
        if let Some(set_sub) = self.with_menu_runtime(|runtime| runtime.set_sub).flatten() {
            set_sub(self, None, cx);
        }
        self.menu_keyboard.in_sub = false;
        self.menu_keyboard.sub_highlighted = None;
        self.menu_keyboard.sub_first = false;
        cx.notify();
    }

    fn menu_escape(&mut self, _: &Escape, _: &mut Window, cx: &mut Context<Self>) {
        self.close_open_menus(cx);
    }

    fn menu_tab(&mut self, _: &Tab, _: &mut Window, _: &mut Context<Self>) {}

    /// `handleTypeaheadSearch`: printable keys accumulate for a second and
    /// focus the next item whose label starts with them.
    fn menu_typeahead(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        if keystroke.modifiers.control || keystroke.modifiers.alt || keystroke.modifiers.platform {
            return;
        }
        let Some(key_char) = keystroke.key_char.as_deref() else {
            return;
        };
        if key_char.chars().count() != 1 {
            return;
        }
        let expired = self
            .menu_keyboard
            .typeahead_at
            .is_none_or(|at| at.elapsed() >= TYPEAHEAD_TIMEOUT);
        // Space selects unless a search is under way.
        if key_char == " " && (expired || self.menu_keyboard.typeahead.is_empty()) {
            return;
        }
        if expired {
            self.menu_keyboard.typeahead.clear();
        }
        self.menu_keyboard.typeahead.push_str(key_char);
        self.menu_keyboard.typeahead_at = Some(Instant::now());
        let search = self.menu_keyboard.typeahead.clone();
        let sub = self.menu_level_is_sub();
        let found = self
            .with_menu_runtime(|runtime| {
                if sub {
                    let items = runtime.sub_items.as_deref().unwrap_or(&[]);
                    let current =
                        self.menu_keyboard
                            .sub_highlighted
                            .or(if self.menu_keyboard.sub_first {
                                first_enabled(items)
                            } else {
                                None
                            });
                    typeahead_match(items, current, &search)
                } else {
                    typeahead_match(&runtime.items, self.menu_keyboard.highlighted, &search)
                }
            })
            .flatten();
        if found.is_some() {
            self.menu_set_highlight(sub, found, cx);
        }
        cx.stop_propagation();
    }

    /// The open menu takes the focus like Radix's content (so the keys reach
    /// it and the field behind it blurs), and gives it back when it closes.
    /// Runs once this frame is drawn.
    pub(crate) fn sync_menu_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let open = self.with_menu_runtime(|runtime| runtime.id);
        if self.menu_keyboard.menu != open {
            self.menu_keyboard = MenuKeyboard {
                menu: open,
                ..Default::default()
            };
        }
        let menu_focus = self.menu_focus.clone();
        let entity = cx.entity().downgrade();
        window.on_next_frame(move |window, cx| {
            let menu_focused = menu_focus.is_focused(window);
            match (open, menu_focused) {
                (Some(_), false) => {
                    let previous = window.focused(cx);
                    entity
                        .update(cx, |this, _| {
                            if this.menu_previous_focus.is_none() {
                                this.menu_previous_focus = previous;
                            }
                        })
                        .ok();
                    window.focus(&menu_focus);
                }
                (None, true) => {
                    let previous = entity
                        .update(cx, |this, _| this.menu_previous_focus.take())
                        .ok()
                        .flatten();
                    match previous {
                        Some(previous) => window.focus(&previous),
                        None => window.blur(),
                    }
                }
                (None, false) => {
                    entity
                        .update(cx, |this, _| this.menu_previous_focus = None)
                        .ok();
                }
                (Some(_), true) => {}
            }
        });
    }

    /// The chrome of a menu that has the keyboard: its focus, key context and
    /// handlers.
    pub(super) fn menu_keyboard_host(
        &self,
        panel: gpui::Stateful<gpui::Div>,
        cx: &Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        panel
            .track_focus(&self.menu_focus)
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(Self::menu_up))
            .on_action(cx.listener(Self::menu_down))
            .on_action(cx.listener(Self::menu_home))
            .on_action(cx.listener(Self::menu_end))
            .on_action(cx.listener(Self::menu_activate))
            .on_action(cx.listener(Self::menu_right))
            .on_action(cx.listener(Self::menu_left))
            .on_action(cx.listener(Self::menu_escape))
            .on_action(cx.listener(Self::menu_tab))
            .on_key_down(cx.listener(Self::menu_typeahead))
    }
}

pub(crate) const ITEM_HEIGHT: f32 = 32.0;
pub(crate) const SEPARATOR_HEIGHT: f32 = 9.0;
/// chrome border + `p-0.5` + panel border + `p-1.5`
pub(crate) const PANEL_INSET: f32 = 1.0 + 2.0 + 1.0 + 6.0;

/// Half a `size-7` / `size-8` trigger plus the 4px `sideOffset`: the inline
/// menus are anchored at the trigger's vertical centre (`top(14)` / `top(16)`).
pub(crate) const INLINE_MENU_SPACER_7: f32 = 18.0;
pub(crate) const INLINE_MENU_SPACER_8: f32 = 20.0;

pub(crate) type Select = Box<dyn Fn(&mut Workspace, &mut gpui::Window, &mut Context<Workspace>)>;
/// A menu item's run, shared between its click and the keyboard.
pub(crate) type Run = Rc<dyn Fn(&mut Workspace, &mut Window, &mut Context<Workspace>)>;
/// Opens (`Some(index)`) or closes (`None`) a menu's submenu.
pub(crate) type SetSub = fn(&mut Workspace, Option<usize>, &mut Context<Workspace>);

pub(crate) enum Trailing {
    None,
    /// `<span className="text-muted-foreground">` after a `flex-1` label.
    Text(SharedString),
    /// A `Check` icon when selected.
    Check(bool),
    /// `DropdownMenuRadioItem`: `pl-8` with the 8px `bg-current` dot in the
    /// `left-2 size-3.5` indicator slot when selected.
    Radio(bool),
    /// `DropdownMenuSubTrigger`'s caret.
    Submenu,
}

/// What a `DropdownMenuSub` opens: another item list, or a custom panel
/// (`DropdownMenuSubContent` wrapping arbitrary content).
pub(crate) enum Submenu {
    Entries(Vec<Entry>),
    Panel {
        width: f32,
        render: fn(&Workspace, &Context<Workspace>) -> AnyElement,
    },
}

pub(crate) enum Entry {
    Item {
        /// Leading icon, drawn at `opacity-70` when `dim_icon`.
        icon: Option<&'static str>,
        dim_icon: bool,
        label: SharedString,
        trailing: Trailing,
        destructive: bool,
        on_select: Option<Select>,
        submenu: Option<Submenu>,
    },
    Separator,
}

impl Entry {
    pub fn height(&self) -> f32 {
        match self {
            Entry::Item { .. } => ITEM_HEIGHT,
            Entry::Separator => SEPARATOR_HEIGHT,
        }
    }
}

pub(crate) struct MenuSpec {
    pub id: &'static str,
    pub width: f32,
    pub entries: Vec<Entry>,
    /// The entry whose submenu is showing.
    pub open_sub: Option<usize>,
    /// Called with an entry index when the pointer enters a submenu trigger
    /// (or `None` over a plain item).
    pub on_hover_sub: fn(&mut Workspace, Option<usize>, &mut Context<Workspace>),
    pub on_close: fn(&mut Workspace, &mut Context<Workspace>),
}

/// Where the menu attaches: `align="start"` hangs the top-left corner off
/// the point, `align="end"` the top-right.
pub(crate) enum Align {
    Start,
    End,
}

impl Workspace {
    /// Renders a menu (and its open submenu) as a deferred overlay.
    pub(crate) fn render_app_menu(
        &self,
        spec: MenuSpec,
        position: Point<Pixels>,
        align: Align,
        window: &gpui::Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let viewport_width = window.viewport_size().width;
        let width = spec.width;
        let open_sub = spec.open_sub;
        let on_hover_sub = spec.on_hover_sub;
        let on_close = spec.on_close;
        let mut submenu_overlay: Option<AnyElement> = None;
        // The sub panel is a separate deferred element, so the parent's
        // `on_mouse_down_out` has to know its bounds to keep clicks inside a
        // custom panel (the participant field) from closing the menu.
        let sub_bounds: std::rc::Rc<std::cell::Cell<Option<gpui::Bounds<Pixels>>>> =
            std::rc::Rc::new(std::cell::Cell::new(None));
        let mut y = PANEL_INSET;
        let panel_left = match align {
            Align::Start => position.x,
            Align::End => position.x - px(width),
        };
        let id = spec.id;
        let mut runtime = MenuRuntime {
            id,
            items: Vec::with_capacity(spec.entries.len()),
            sub_items: None,
            set_sub: Some(on_hover_sub),
        };

        let items = spec
            .entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| {
                let top = y;
                y += entry.height();
                match entry {
                    Entry::Separator => {
                        runtime.items.push(None);
                        div()
                            .mx(px(-4.0))
                            .my_1()
                            .h(px(1.0))
                            .bg(theme.accent)
                            .into_any_element()
                    }
                    Entry::Item {
                        icon: glyph,
                        dim_icon,
                        label,
                        trailing,
                        destructive,
                        on_select,
                        submenu,
                    } => {
                        let is_sub = submenu.is_some();
                        let highlighted = self.menu_keyboard.is_highlighted(id, index);
                        if let (Some(submenu), true) = (submenu, open_sub == Some(index)) {
                            // `DropdownMenuSubContent`: measured against the
                            // app, the sub chrome starts at the parent's right
                            // edge, 2px above the trigger row; Radix flips it
                            // to the left edge when the right side lacks room.
                            let sub_width = match &submenu {
                                Submenu::Entries(_) => 176.0,
                                Submenu::Panel { width, .. } => *width,
                            };
                            // Flipped, the sub's right edge sits 9px inside the
                            // parent chrome (measured against the app).
                            let sub_x = if panel_left + px(width + sub_width + 8.0) > viewport_width
                            {
                                panel_left - px(sub_width - 9.0)
                            } else {
                                panel_left + px(width)
                            };
                            let sub_position = point(sub_x, position.y + px(top) - px(2.0));
                            submenu_overlay = Some(match submenu {
                                Submenu::Entries(entries) => {
                                    let (panel, sub_items) = self.render_menu_panel(
                                        spec.id,
                                        entries,
                                        sub_width,
                                        sub_position,
                                        on_close,
                                        cx,
                                    );
                                    runtime.sub_items = Some(sub_items);
                                    panel
                                }
                                Submenu::Panel { render, .. } => self.render_sub_panel(
                                    sub_width,
                                    sub_position,
                                    render(self, cx),
                                    sub_bounds.clone(),
                                ),
                            });
                        }
                        let color = if destructive {
                            theme.delete_text
                        } else {
                            theme.foreground
                        };
                        let on_select = on_select.map(Rc::new);
                        let radio = matches!(trailing, Trailing::Radio(_));
                        runtime.items.push(Some(RuntimeItem {
                            label: label.clone(),
                            enabled: on_select.is_some() || is_sub,
                            submenu: is_sub,
                            activate: {
                                let on_select = on_select.clone();
                                Rc::new(move |this, window, cx| {
                                    on_close(this, cx);
                                    if let Some(on_select) = &on_select {
                                        on_select(this, window, cx);
                                    }
                                })
                            },
                        }));
                        div()
                            .id((spec.id, index))
                            .relative()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .when(radio, |item| item.pl(px(32.0)))
                            .py(px(6.0))
                            .rounded(px(14.0))
                            .tw_text_sm()
                            .text_color(color)
                            .cursor_pointer()
                            .when(open_sub == Some(index), |item| item.bg(theme.accent))
                            // `data-[highlighted]`: the focused item, which the
                            // pointer and the keys both move.
                            .when(highlighted, |item| {
                                if destructive {
                                    item.bg(theme.delete_hover_background)
                                        .text_color(theme.delete_hover_text)
                                } else {
                                    item.bg(theme.accent)
                                }
                            })
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                if *hovered {
                                    on_hover_sub(this, is_sub.then_some(index), cx);
                                }
                                this.menu_hover(id, false, index, *hovered, cx);
                            }))
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                if is_sub {
                                    return;
                                }
                                on_close(this, cx);
                                if let Some(on_select) = &on_select {
                                    on_select(this, window, cx);
                                }
                            }))
                            .when_some(glyph, |item, glyph| {
                                let tint = if dim_icon {
                                    crate::theme::alpha(color, 0.7)
                                } else {
                                    color
                                };
                                item.child(icon(glyph, px(16.0), tint))
                            })
                            .child(div().flex_1().child(label))
                            .child(match trailing {
                                Trailing::None | Trailing::Submenu => div().into_any_element(),
                                Trailing::Text(text) => div()
                                    .text_color(theme.muted_foreground)
                                    .child(text)
                                    .into_any_element(),
                                Trailing::Check(checked) => {
                                    if checked {
                                        icon("check", px(16.0), color).into_any_element()
                                    } else {
                                        div().into_any_element()
                                    }
                                }
                                Trailing::Radio(selected) => radio_indicator(selected, color),
                            })
                            // `DropdownMenuSubTrigger` always ends with the caret.
                            .when(is_sub, |item| {
                                item.child(icon("caret-right", px(16.0), color))
                            })
                            .into_any_element()
                    }
                }
            })
            .collect::<Vec<_>>();
        *self.menu_runtime.borrow_mut() = Some(runtime);

        let panel = self
            .menu_keyboard_host(menu_chrome(theme, spec.id, width), cx)
            .on_mouse_down_out(cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                if sub_bounds
                    .get()
                    .is_some_and(|bounds| bounds.contains(&event.position))
                {
                    return;
                }
                on_close(this, cx)
            }))
            .child(
                // `AppFloatingPanel`: `rounded-[20px] border` under the panel squircle.
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .p(px(6.0))
                    .child(crate::squircle::squircle(
                        crate::squircle::PANEL_RADIUS,
                        Some(theme.floating_panel),
                        Some((1.0, theme.floating_border)),
                    ))
                    .children(items),
            );

        let corner = match align {
            Align::Start => Corner::TopLeft,
            Align::End => Corner::TopRight,
        };
        div()
            .child(
                deferred(
                    anchored()
                        .anchor(corner)
                        .position(position)
                        .snap_to_window_with_margin(px(8.0))
                        .child(panel),
                )
                .with_priority(1),
            )
            .children(submenu_overlay)
            .into_any_element()
    }

    /// A standalone menu anchored at the element's own layout position (for
    /// dropdown triggers inside scrolling content): the app chrome, items with
    /// hover and click, `on_mouse_down_out` closing it.
    /// Radix menus dismiss on Escape; the same happens when a shortcut swaps
    /// the view under an open menu. Returns whether anything was open.
    pub(crate) fn close_open_menus(&mut self, cx: &mut Context<Self>) -> bool {
        let mut closed = false;
        if self.filter_menu_open {
            self.filter_menu_open = false;
            self.filter_submenu = None;
            closed = true;
        }
        if self.overflow_open {
            self.overflow_open = false;
            self.overflow_submenu = None;
            closed = true;
        }
        if self.open_menu.take().is_some() {
            closed = true;
        }
        if self.edit_context_menu.take().is_some() {
            closed = true;
        }
        if let Some(player) = self.audio_player.as_mut()
            && player.menu_at.take().is_some()
        {
            closed = true;
        }
        closed |= self.close_calendar_context_menu();
        closed |= self.close_contacts_menus();
        closed |= self.close_templates_menus();
        closed |= self.close_automations_menus();
        closed |= self.close_speaker_assign(cx);
        // The transcript selection menu's `useAutoCloser` closes on Escape too.
        closed |= self.clear_text_selection(cx);
        if closed {
            cx.notify();
        }
        closed
    }

    pub(crate) fn render_menu_inline(
        &self,
        spec: MenuSpec,
        align: Align,
        spacer: f32,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let on_close = spec.on_close;
        let (items, runtime_items) = self.menu_item_elements(spec.id, spec.entries, on_close, cx);
        *self.menu_runtime.borrow_mut() = Some(MenuRuntime {
            id: spec.id,
            items: runtime_items,
            sub_items: None,
            set_sub: None,
        });
        let panel = self
            .menu_keyboard_host(menu_chrome(theme, spec.id, spec.width), cx)
            .on_mouse_down_out(
                cx.listener(move |this, _: &MouseDownEvent, _, cx| on_close(this, cx)),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .p(px(6.0))
                    .child(crate::squircle::squircle(
                        crate::squircle::PANEL_RADIUS,
                        Some(theme.floating_panel),
                        Some((1.0, theme.floating_border)),
                    ))
                    .children(items),
            );
        // `DropdownMenuContent sideOffset={4}` under a `size-7` trigger, flipped
        // above it like Radix when it would leave the window: the anchor sits at
        // the trigger's vertical centre and the panel is padded by half the
        // trigger plus the offset on both sides, so the switched corner lands
        // the panel 4px above the trigger instead.
        deferred(
            anchored()
                .anchor(match align {
                    Align::Start => Corner::TopLeft,
                    Align::End => Corner::TopRight,
                })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(div().h(px(spacer)))
                        .child(panel)
                        .child(div().h(px(spacer))),
                ),
        )
        .with_priority(2)
        .into_any_element()
    }

    /// The `rounded-[14px] px-2 py-1.5 text-sm gap-2` rows of a plain panel.
    fn menu_item_elements(
        &self,
        id: &'static str,
        entries: Vec<Entry>,
        on_close: fn(&mut Workspace, &mut Context<Workspace>),
        cx: &Context<Self>,
    ) -> (Vec<AnyElement>, Vec<Option<RuntimeItem>>) {
        let theme = self.theme;
        let mut runtime_items = Vec::with_capacity(entries.len());
        let elements = entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| match entry {
                Entry::Separator => {
                    runtime_items.push(None);
                    div()
                        .mx(px(-4.0))
                        .my_1()
                        .h(px(1.0))
                        .bg(theme.accent)
                        .into_any_element()
                }
                Entry::Item {
                    icon: glyph,
                    dim_icon,
                    label,
                    trailing,
                    destructive,
                    on_select,
                    ..
                } => {
                    let color = if destructive {
                        theme.delete_text
                    } else {
                        theme.foreground
                    };
                    let disabled = on_select.is_none();
                    let on_select = on_select.map(Rc::new);
                    let radio = matches!(trailing, Trailing::Radio(_));
                    let highlighted = !disabled && self.menu_keyboard.is_highlighted(id, index);
                    runtime_items.push(Some(RuntimeItem {
                        label: label.clone(),
                        enabled: !disabled,
                        submenu: false,
                        activate: {
                            let on_select = on_select.clone();
                            Rc::new(move |this, window, cx| {
                                if let Some(on_select) = &on_select {
                                    on_close(this, cx);
                                    on_select(this, window, cx);
                                }
                            })
                        },
                    }));
                    div()
                        .id(SharedString::from(format!("{id}-inline-{index}")))
                        .relative()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .when(radio, |item| item.pl(px(32.0)))
                        .py(px(6.0))
                        .rounded(px(14.0))
                        .tw_text_sm()
                        .text_color(color)
                        // `data-[disabled]:opacity-50`
                        .when(disabled, |item| item.opacity(0.5))
                        .when(!disabled, |item| item.cursor_pointer())
                        .when(highlighted, |item| item.bg(theme.accent))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .when(!disabled, |item| {
                            item.on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                this.menu_hover(id, false, index, *hovered, cx);
                            }))
                        })
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            if let Some(on_select) = &on_select {
                                on_close(this, cx);
                                on_select(this, window, cx);
                            }
                        }))
                        .when_some(glyph, |item, glyph| {
                            let tint = if dim_icon {
                                crate::theme::alpha(color, 0.7)
                            } else {
                                color
                            };
                            item.child(icon(glyph, px(16.0), tint))
                        })
                        .child(div().flex_1().child(label))
                        .child(match trailing {
                            Trailing::Check(true) => {
                                icon("check", px(14.0), color).into_any_element()
                            }
                            Trailing::Radio(selected) => radio_indicator(selected, color),
                            Trailing::Text(text) => div()
                                .text_color(theme.muted_foreground)
                                .child(text)
                                .into_any_element(),
                            _ => div().into_any_element(),
                        })
                        .into_any_element()
                }
            })
            .collect();
        (elements, runtime_items)
    }

    /// A submenu panel: items only, no hover switching of its own.
    fn render_menu_panel(
        &self,
        id: &'static str,
        entries: Vec<Entry>,
        width: f32,
        position: Point<Pixels>,
        on_close: fn(&mut Workspace, &mut Context<Workspace>),
        cx: &Context<Self>,
    ) -> (AnyElement, Vec<Option<RuntimeItem>>) {
        let theme = self.theme;
        // The sub's first enabled item: focused when the keyboard opened it.
        let first = entries.iter().position(|entry| {
            matches!(
                entry,
                Entry::Item {
                    on_select: Some(_),
                    ..
                }
            )
        });
        let mut runtime_items = Vec::with_capacity(entries.len());
        let items = entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| match entry {
                Entry::Separator => {
                    runtime_items.push(None);
                    div()
                        .mx(px(-4.0))
                        .my_1()
                        .h(px(1.0))
                        .bg(theme.accent)
                        .into_any_element()
                }
                Entry::Item {
                    icon: glyph,
                    dim_icon,
                    label,
                    trailing,
                    destructive,
                    on_select,
                    ..
                } => {
                    let color = if destructive {
                        theme.delete_text
                    } else {
                        theme.foreground
                    };
                    let on_select = on_select.map(Rc::new);
                    let highlighted = self.menu_keyboard.is_sub_highlighted(id, index, first);
                    runtime_items.push(Some(RuntimeItem {
                        label: label.clone(),
                        enabled: on_select.is_some(),
                        submenu: false,
                        activate: {
                            let on_select = on_select.clone();
                            Rc::new(move |this, window, cx| {
                                on_close(this, cx);
                                if let Some(on_select) = &on_select {
                                    on_select(this, window, cx);
                                }
                            })
                        },
                    }));
                    div()
                        .id(SharedString::from(format!("{id}-sub-{index}")))
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py(px(6.0))
                        .rounded(px(14.0))
                        .tw_text_sm()
                        .text_color(color)
                        .cursor_pointer()
                        .when(highlighted, |item| item.bg(theme.accent))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                            this.menu_hover(id, true, index, *hovered, cx);
                        }))
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            on_close(this, cx);
                            if let Some(on_select) = &on_select {
                                on_select(this, window, cx);
                            }
                        }))
                        .when_some(glyph, |item, glyph| {
                            let tint = if dim_icon {
                                crate::theme::alpha(color, 0.7)
                            } else {
                                color
                            };
                            item.child(icon(glyph, px(16.0), tint))
                        })
                        .child(div().flex_1().child(label))
                        .child(match trailing {
                            Trailing::Check(true) => {
                                icon("check", px(16.0), color).into_any_element()
                            }
                            Trailing::Text(text) => div()
                                .text_color(theme.muted_foreground)
                                .child(text)
                                .into_any_element(),
                            _ => div().into_any_element(),
                        })
                        .into_any_element()
                }
            });
        let panel = menu_chrome(theme, "submenu", width).child(
            div()
                .relative()
                .flex()
                .flex_col()
                .p(px(6.0))
                .child(crate::squircle::squircle(
                    crate::squircle::PANEL_RADIUS,
                    Some(theme.floating_panel),
                    Some((1.0, theme.floating_border)),
                ))
                .children(items),
        );
        let element = deferred(
            anchored()
                .anchor(Corner::TopLeft)
                .position(position)
                .snap_to_window_with_margin(px(8.0))
                .child(panel),
        )
        .with_priority(2)
        .into_any_element();
        (element, runtime_items)
    }
}

impl Workspace {
    /// `DropdownMenuSubContent variant="app"` around an `AppFloatingPanel`
    /// holding custom content.
    fn render_sub_panel(
        &self,
        width: f32,
        position: Point<Pixels>,
        content: AnyElement,
        bounds_out: std::rc::Rc<std::cell::Cell<Option<gpui::Bounds<Pixels>>>>,
    ) -> AnyElement {
        let theme = self.theme;
        let panel = menu_chrome(theme, "submenu", width)
            .child(
                gpui::canvas(
                    move |bounds, _, _| bounds_out.set(Some(bounds)),
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0()
                .size_full(),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .child(crate::squircle::squircle(
                        crate::squircle::PANEL_RADIUS,
                        Some(theme.floating_panel),
                        Some((1.0, theme.floating_border)),
                    ))
                    .child(content),
            );
        deferred(
            anchored()
                .anchor(Corner::TopLeft)
                .position(position)
                .snap_to_window_with_margin(px(8.0))
                .child(panel),
        )
        .with_priority(2)
        .into_any_element()
    }
}

/// `appFloatingContentClassName` with `shadow-lg`.
pub(super) fn menu_chrome(theme: Theme, id: &'static str, width: f32) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(format!("{id}-chrome")))
        .occlude()
        .w(px(width))
        .p(px(2.0))
        .rounded(px(22.0))
        .border_1()
        .border_color(theme.floating_border)
        .bg(theme.floating_chrome)
        .shadow(vec![
            BoxShadow {
                color: hsla(0.0, 0.0, 0.0, 0.1),
                offset: point(px(0.0), px(10.0)),
                blur_radius: px(15.0),
                spread_radius: px(-3.0),
            },
            BoxShadow {
                color: hsla(0.0, 0.0, 0.0, 0.1),
                offset: point(px(0.0), px(4.0)),
                blur_radius: px(6.0),
                spread_radius: px(-4.0),
            },
        ])
}

/// `DropdownMenuRadioItem`'s indicator: the `left-2 size-3.5` slot with the
/// 8px `bg-current` dot while selected.
fn radio_indicator(selected: bool, color: gpui::Rgba) -> AnyElement {
    div()
        .absolute()
        .left(px(8.0))
        .size(px(14.0))
        .flex()
        .items_center()
        .justify_center()
        .when(selected, |slot| {
            slot.child(div().size(px(8.0)).rounded_full().bg(color))
        })
        .into_any_element()
}
