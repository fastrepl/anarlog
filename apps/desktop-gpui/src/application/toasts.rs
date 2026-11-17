//! Shipping's app toasts: `@anlg/ui` `toast.*` as `AppToaster` shows them on desktop
//! (compact, stacked bottom-right). Any view posts one with [`show`] or a shorthand such as
//! [`error`]; the main window renders the stack.

use std::{
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use gpui::{
    Animation, AnimationExt, App, AppContext, Context, ElementId, Entity, FontWeight, Global, Hsla,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Task, Transformation, Window, div, img, percentage,
    prelude::FluentBuilder, px, relative, rgb, svg,
};

use crate::ui::theme::{MEDIUM, system_font, theme};

/// `TOAST_DURATIONS` in `@anlg/ui` toast.tsx.
const SUCCESS: Duration = Duration::from_millis(3_000);
const INFO: Duration = Duration::from_millis(4_000);
const WARNING: Duration = Duration::from_millis(6_000);
const ERROR: Duration = Duration::from_millis(5_000);
const EXIT: Duration = Duration::from_millis(220);
const VISIBLE: usize = 4;
const STACK_OFFSET: f32 = 10.;
const STACK_SCALE_STEP: f32 = 0.035;
const WIDTH: f32 = 320.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Info,
    Success,
    Warning,
    Error,
}

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(Clone)]
pub struct Toast {
    id: SharedString,
    message: SharedString,
    description: Option<SharedString>,
    tone: Tone,
    loading: bool,
    image: Option<SharedString>,
    action: Option<(SharedString, Handler)>,
    duration: Option<Duration>,
    dismissible: bool,
    gauge: Option<(Instant, Duration)>,
    on_dismiss: Option<Handler>,
}

impl Toast {
    fn new(tone: Tone, message: impl Into<SharedString>, duration: Option<Duration>) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self {
            id: format!("notification-{}", NEXT.fetch_add(1, Ordering::Relaxed) + 1).into(),
            message: message.into(),
            description: None,
            tone,
            loading: false,
            image: None,
            action: None,
            duration,
            dismissible: true,
            gauge: None,
            on_dismiss: None,
        }
    }

    pub fn info(message: impl Into<SharedString>) -> Self {
        Self::new(Tone::Info, message, Some(INFO))
    }

    pub fn success(message: impl Into<SharedString>) -> Self {
        Self::new(Tone::Success, message, Some(SUCCESS))
    }

    pub fn warning(message: impl Into<SharedString>) -> Self {
        Self::new(Tone::Warning, message, Some(WARNING))
    }

    pub fn error(message: impl Into<SharedString>) -> Self {
        Self::new(Tone::Error, message, Some(ERROR))
    }

    /// `toast.loading`: a spinner and no timer.
    pub fn loading(message: impl Into<SharedString>) -> Self {
        Self {
            loading: true,
            ..Self::new(Tone::Info, message, None)
        }
    }

    /// Posting a toast with a visible toast's id replaces it in place.
    pub fn id(mut self, id: impl Into<SharedString>) -> Self {
        self.id = id.into();
        self
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// The primary button; clicking it runs `handler` and dismisses the toast.
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some((label.into(), Rc::new(handler)));
        self
    }

    /// Like shipping, an error toast never stays longer than five seconds unless it is sticky.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(if self.tone == Tone::Error {
            duration.min(ERROR)
        } else {
            duration
        });
        self
    }

    /// `duration: Infinity`: the toast stays until dismissed.
    pub fn sticky(mut self) -> Self {
        self.duration = None;
        self
    }

    /// Without it the toast has no Dismiss button (`closeButton: false`).
    pub fn dismissible(mut self, dismissible: bool) -> Self {
        self.dismissible = dismissible;
        self
    }

    /// An image in place of the tone icon.
    pub fn image(mut self, path: impl Into<SharedString>) -> Self {
        self.image = Some(path.into());
        self
    }

    /// The undo toast's draining bar along the bottom edge.
    pub fn gauge(mut self, started: Instant, total: Duration) -> Self {
        self.gauge = Some((started, total));
        self
    }

    /// Runs when the user dismisses the toast with its Dismiss button.
    pub fn on_dismiss(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }

    pub fn id_ref(&self) -> &SharedString {
        &self.id
    }
}

struct Item {
    toast: Toast,
    /// Time left while paused, or the full duration before the first tick.
    remaining: Option<Duration>,
    deadline: Option<Instant>,
    expanded: bool,
    leaving: Option<Instant>,
}

impl Item {
    fn remaining(&self, now: Instant) -> Option<Duration> {
        match self.deadline {
            Some(deadline) => Some(deadline.saturating_duration_since(now)),
            None => self.remaining,
        }
    }

    fn has_details(&self) -> bool {
        self.toast.duration.is_some()
            || self.toast.description.is_some()
            || self.toast.gauge.is_some()
    }
}

pub struct Toasts {
    items: Vec<Item>,
    hovered: bool,
    ticker: Option<Task<()>>,
}

struct ToastStore(Entity<Toasts>);

impl Global for ToastStore {}

/// Creates the app-wide stack; the main window renders the returned view.
pub fn install(cx: &mut App) -> Entity<Toasts> {
    let toasts = cx.new(|_| Toasts {
        items: Vec::new(),
        hovered: false,
        ticker: None,
    });
    cx.set_global(ToastStore(toasts.clone()));
    toasts
}

pub fn show(toast: Toast, cx: &mut App) {
    let Some(store) = cx.try_global::<ToastStore>().map(|store| store.0.clone()) else {
        return;
    };
    store.update(cx, |toasts, cx| {
        toasts.insert(toast, Instant::now());
        toasts.watch(cx);
        cx.notify();
    });
}

pub fn dismiss(id: &str, cx: &mut App) {
    let Some(store) = cx.try_global::<ToastStore>().map(|store| store.0.clone()) else {
        return;
    };
    store.update(cx, |toasts, cx| {
        if toasts.dismiss(id, Instant::now()) {
            toasts.watch(cx);
            cx.notify();
        }
    });
}

/// Shorthands key the toast by its text, so repeating a message replaces it.
pub fn error(message: impl Into<SharedString>, cx: &mut App) {
    let message = message.into();
    show(Toast::error(message.clone()).id(message), cx);
}

pub fn success(message: impl Into<SharedString>, cx: &mut App) {
    let message = message.into();
    show(Toast::success(message.clone()).id(message), cx);
}

pub fn info(message: impl Into<SharedString>, cx: &mut App) {
    let message = message.into();
    show(Toast::info(message.clone()).id(message), cx);
}

impl Toasts {
    fn insert(&mut self, toast: Toast, now: Instant) {
        let item = Item {
            remaining: toast.duration,
            deadline: toast.duration.map(|duration| now + duration),
            expanded: true,
            leaving: None,
            toast,
        };
        match self
            .items
            .iter_mut()
            .find(|existing| existing.toast.id == item.toast.id)
        {
            Some(existing) => {
                *existing = Item {
                    expanded: existing.expanded,
                    ..item
                }
            }
            None => self.items.push(item),
        }
    }

    fn dismiss(&mut self, id: &str, now: Instant) -> bool {
        match self
            .items
            .iter_mut()
            .find(|item| item.toast.id.as_ref() == id && item.leaving.is_none())
        {
            Some(item) => {
                item.deadline = None;
                item.leaving = Some(now);
                true
            }
            None => false,
        }
    }

    /// Starts expired toasts leaving and drops those whose exit finished.
    fn expire(&mut self, now: Instant) {
        for item in &mut self.items {
            if item.leaving.is_none() && item.deadline.is_some_and(|deadline| deadline <= now) {
                item.deadline = None;
                item.leaving = Some(now);
            }
        }
        self.items
            .retain(|item| item.leaving.is_none_or(|left| now < left + EXIT));
    }

    fn toggle_timer(&mut self, id: &str, now: Instant) {
        let Some(item) = self
            .items
            .iter_mut()
            .find(|item| item.toast.id.as_ref() == id && item.leaving.is_none())
        else {
            return;
        };
        match item.deadline.take() {
            Some(deadline) => item.remaining = Some(deadline.saturating_duration_since(now)),
            None => item.deadline = item.remaining.map(|remaining| now + remaining),
        }
    }

    fn busy(&self) -> bool {
        self.items
            .iter()
            .any(|item| item.deadline.is_some() || item.leaving.is_some())
    }

    fn watch(&mut self, cx: &mut Context<Self>) {
        if self.ticker.is_some() || !self.busy() {
            return;
        }
        self.ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let busy = this.update(cx, |this, cx| {
                    this.expire(Instant::now());
                    cx.notify();
                    let busy = this.busy();
                    if !busy {
                        this.ticker = None;
                    }
                    busy
                });
                if !busy.unwrap_or(false) {
                    break;
                }
            }
        }));
    }

    fn close(&mut self, id: SharedString, cx: &mut Context<Self>) {
        if self.dismiss(&id, Instant::now()) {
            self.watch(cx);
            cx.notify();
        }
    }
}

fn tone_icon(tone: Tone, colors: &crate::ui::theme::Theme) -> (&'static str, Hsla) {
    match tone {
        Tone::Success => ("product/CheckmarkCircle02Icon.svg", rgb(0x009966).into()),
        Tone::Warning => ("product/AlertCircleIcon.svg", rgb(0xe17100).into()),
        Tone::Error => ("product/AlertCircleIcon.svg", colors.destructive),
        Tone::Info => ("InfoIcon.svg", rgb(0x155dfc).into()),
    }
}

/// A bar along the card's rounded bottom edge. GPUI clips to rectangles, so it is the bottom
/// border of a box with the card's inner radius, which follows the curve like shipping's clip.
fn bottom_bar(fraction: f32, color: Hsla) -> gpui::Div {
    div()
        .absolute()
        .left_0()
        .bottom_0()
        .h(px(14.))
        .w(relative(fraction))
        .rounded_bl(px(13.))
        .when(fraction >= 0.999, |bar| bar.rounded_br(px(13.)))
        .border_color(color)
}

fn button(
    id: SharedString,
    label: SharedString,
    primary: bool,
    colors: &crate::ui::theme::Theme,
) -> gpui::Stateful<gpui::Div> {
    let button = div()
        .id(id)
        .min_h(px(28.))
        .min_w_0()
        .px(px(10.))
        .py(px(4.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .text_xs()
        .line_height(px(16.))
        .font_weight(MEDIUM)
        .cursor_pointer()
        .child(label);
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
}

impl Toasts {
    fn card(
        &self,
        item: &Item,
        depth: usize,
        stacked: bool,
        now: Instant,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let colors = theme(window);
        let toast = &item.toast;
        let id = toast.id.clone();
        let behind = stacked && depth > 0;
        let expanded = item.expanded && !behind;
        let remaining = item.remaining(now);
        let (icon, color) = tone_icon(toast.tone, &colors);
        let leading = if let Some(image) = &toast.image {
            img(image.clone()).size(px(20.)).into_any_element()
        } else if toast.loading {
            svg()
                .path("product/LoaderCircleIcon.svg")
                .size(px(16.))
                .text_color(colors.popover_foreground)
                .with_animation(
                    ElementId::Name(format!("toast-spin-{id}").into()),
                    Animation::new(Duration::from_secs(1)).repeat(),
                    |icon, delta| {
                        icon.with_transformation(Transformation::rotate(percentage(delta)))
                    },
                )
                .into_any_element()
        } else {
            svg()
                .path(icon)
                .size(px(16.))
                .text_color(color)
                .into_any_element()
        };
        let toggle = {
            let id = id.clone();
            cx.listener(move |this: &mut Self, _, _, cx| {
                if let Some(item) = this
                    .items
                    .iter_mut()
                    .find(|item| item.toast.id == id && item.has_details())
                {
                    item.expanded = !item.expanded;
                    cx.notify();
                }
            })
        };
        // Handlers run outside this store's update: they may post or dismiss toasts.
        let store = cx.entity().downgrade();
        let run = move |id: SharedString, handler: Option<Handler>| {
            let store = store.clone();
            move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
                cx.stop_propagation();
                if let Some(handler) = &handler {
                    handler(window, cx);
                }
                let _ = store.update(cx, |this, cx| this.close(id.clone(), cx));
            }
        };
        let mut actions = Vec::new();
        if !behind {
            if let Some((label, handler)) = toast.action.clone() {
                actions.push(
                    button(format!("toast-action-{id}").into(), label, true, &colors)
                        .on_click(run(id.clone(), Some(handler))),
                );
            }
            if toast.dismissible {
                actions.push(
                    button(
                        format!("toast-dismiss-{id}").into(),
                        "Dismiss".into(),
                        false,
                        &colors,
                    )
                    .on_click(run(id.clone(), toast.on_dismiss.clone())),
                );
            }
        }
        let gauge = toast.gauge.filter(|_| expanded).map(|(started, total)| {
            let left = total.saturating_sub(now.saturating_duration_since(started));
            left.as_secs_f32() / total.as_secs_f32().max(f32::EPSILON)
        });
        let footer = remaining.filter(|_| expanded).map(|remaining| {
            let total = toast.duration.unwrap_or(remaining).as_secs_f32();
            let progress = (remaining.as_secs_f32() / total.max(f32::EPSILON)).clamp(0., 1.);
            let seconds = remaining.as_millis().div_ceil(1_000);
            let paused = item.deadline.is_none();
            let bar = match toast.tone {
                Tone::Error => colors.destructive,
                Tone::Success => rgb(0x00bc7d).into(),
                _ => rgb(0x2b7fff).into(),
            };
            let id = id.clone();
            div()
                .id(SharedString::from(format!("toast-timer-{id}")))
                .relative()
                .w_full()
                .min_h(px(32.))
                .px(px(10.))
                .py(px(6.))
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .rounded_b(px(13.))
                .border_t_1()
                .border_color(colors.border.opacity(0.6))
                .bg(colors.muted.opacity(0.5))
                .text_color(colors.muted_foreground)
                .text_size(px(11.))
                .line_height(px(16.5))
                .cursor_pointer()
                .hover(|style| style.bg(colors.muted))
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.toggle_timer(&id, Instant::now());
                    this.watch(cx);
                    cx.notify();
                }))
                .child(
                    div()
                        .flex()
                        .gap(px(3.))
                        .child(if paused { "Timer paused" } else { "Closing in" })
                        .child(
                            div()
                                .text_color(colors.foreground)
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(seconds.to_string()),
                        )
                        .child(if seconds == 1 { "second" } else { "seconds" }),
                )
                .child(
                    div()
                        .text_color(colors.foreground)
                        .font_weight(MEDIUM)
                        .child(if paused { "Resume" } else { "Pause" }),
                )
                .child(bottom_bar(progress, bar).border_b_2())
        });
        let scale = (1. - depth as f32 * STACK_SCALE_STEP).max(0.88);
        let width = (window.viewport_size().width - px(32.)).min(px(WIDTH));
        let card = div()
            .id(SharedString::from(format!("toast-{id}")))
            .relative()
            .w(if behind { width * scale } else { width })
            .rounded(px(14.))
            .border_1()
            .border_color(colors.border.opacity(0.7))
            .bg(colors.popover)
            .text_color(colors.popover_foreground)
            .shadow_md()
            .when(!behind, |card| card.on_click(toggle))
            .child(
                div()
                    .flex()
                    .items_center()
                    .min_h(px(44.))
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(6.))
                    .when(item.has_details(), |header| header.cursor_pointer())
                    .child(
                        div()
                            .size(px(32.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(leading),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(13.))
                            .line_height(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(toast.message.clone()),
                    ),
            )
            .when_some(
                toast.description.clone().filter(|_| expanded),
                |card, text| {
                    card.child(
                        div()
                            .px(px(10.))
                            .pb(px(10.))
                            .text_xs()
                            .line_height(px(16.))
                            .text_color(colors.muted_foreground)
                            .child(text),
                    )
                },
            )
            // The undo gauge sits in the description slot, whose padding remains.
            .when(gauge.is_some(), |card| card.child(div().h(px(10.))))
            .when(!actions.is_empty(), |card| {
                card.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .px(px(10.))
                        .pb(px(10.))
                        .children(actions),
                )
            })
            .children(footer)
            .when_some(gauge, |card, progress| {
                card.child(bottom_bar(1., colors.muted).border_b_4())
                    .child(bottom_bar(progress, colors.primary).border_b_4())
            });
        let leaving = item.leaving.map(|left| now.saturating_duration_since(left));
        let distance = f32::from(window.viewport_size().width) + 256.;
        let wrapper = div()
            .relative()
            .flex()
            .justify_center()
            .when(stacked, |wrapper| {
                wrapper
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(-STACK_OFFSET * depth as f32))
            })
            .when(stacked && depth == 0, |wrapper| wrapper.relative().top_0())
            .child(card);
        match leaving {
            Some(elapsed) => {
                // Exits slide right as they fade, eased in like shipping's `[0.4, 0, 1, 1]`.
                let start = (elapsed.as_secs_f32() / EXIT.as_secs_f32()).min(1.);
                wrapper
                    .with_animation(
                        ElementId::Name(format!("toast-leave-{id}").into()),
                        Animation::new(EXIT.saturating_sub(elapsed).max(Duration::from_millis(1))),
                        move |wrapper, delta| {
                            let t = start + (1. - start) * delta;
                            wrapper
                                .left(px(distance * t * t))
                                .opacity(1. - (t * EXIT.as_secs_f32() / 0.18).min(1.))
                        },
                    )
                    .into_any_element()
            }
            None => wrapper
                .with_animation(
                    ElementId::Name(format!("toast-enter-{id}").into()),
                    Animation::new(Duration::from_millis(240)).with_easing(gpui::ease_out_quint()),
                    move |wrapper, delta| {
                        let wrapper = wrapper.opacity((delta * 240. / 180.).min(1.));
                        if stacked && depth > 0 {
                            wrapper
                        } else {
                            wrapper.top(px(20. * (1. - delta)))
                        }
                    },
                )
                .into_any_element(),
        }
    }
}

impl Render for Toasts {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        let start = self.items.len().saturating_sub(VISIBLE);
        let count = self.items.len() - start;
        let stacked = count > 1 && !self.hovered;
        if self.items[start..]
            .iter()
            .any(|item| item.toast.gauge.is_some() || item.deadline.is_some())
        {
            window.request_animation_frame();
        }
        let cards = (start..self.items.len())
            .map(|index| {
                let depth = self.items.len() - index - 1;
                self.card(&self.items[index], depth, stacked, now, window, cx)
            })
            .collect::<Vec<_>>();
        div()
            .absolute()
            .right(px(16.))
            .bottom(px(16.))
            .font_family(system_font(cx))
            .when(count > 0, |root| {
                root.child(
                    div()
                        .id("toast-stack")
                        .relative()
                        .flex()
                        .flex_col()
                        .when(!stacked, |stack| stack.gap(px(8.)))
                        .occlude()
                        .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                            if this.hovered != *hovered {
                                this.hovered = *hovered;
                                cx.notify();
                            }
                        }))
                        .children(cards),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Toasts {
        Toasts {
            items: Vec::new(),
            hovered: false,
            ticker: None,
        }
    }

    #[test]
    fn toasts_expire_by_tone_pause_on_request_and_replace_by_id() {
        let start = Instant::now();
        let mut toasts = store();
        toasts.insert(Toast::success("Saved").id("a"), start);
        toasts.insert(
            Toast::error("Failed")
                .id("b")
                .duration(Duration::from_secs(60)),
            start,
        );
        toasts.insert(Toast::info("Loading").id("c").sticky(), start);
        toasts.insert(Toast::warning("Careful").id("d"), start);
        toasts.toggle_timer("d", start + Duration::from_secs(1));

        // Errors are capped at five seconds; success lasts three.
        let leaving = |toasts: &Toasts, id: &str| {
            toasts
                .items
                .iter()
                .any(|item| item.toast.id.as_ref() == id && item.leaving.is_some())
        };
        toasts.expire(start + Duration::from_secs(3));
        assert!(leaving(&toasts, "a"));
        assert!(!leaving(&toasts, "b"));
        toasts.expire(start + Duration::from_secs(5));
        assert!(leaving(&toasts, "b"));
        toasts.expire(start + Duration::from_secs(10));
        let ids = toasts
            .items
            .iter()
            .map(|item| item.toast.id.as_ref())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["c", "d"], "sticky and paused toasts stay");
        assert_eq!(
            toasts.items[1].remaining(start + Duration::from_secs(60)),
            Some(Duration::from_secs(5))
        );

        toasts.insert(
            Toast::info("Replaced").id("c"),
            start + Duration::from_secs(10),
        );
        assert_eq!(toasts.items.len(), 2);
        assert_eq!(toasts.items[0].toast.message.as_ref(), "Replaced");
        assert!(toasts.dismiss("c", start + Duration::from_secs(10)));
        assert!(!toasts.dismiss("c", start + Duration::from_secs(10)));
    }
}
