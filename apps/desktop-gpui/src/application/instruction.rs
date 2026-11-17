//! Shipping's browser hand-off (`instruction/index.tsx`, `routes/app/instruction.tsx` and
//! `openUrlWithInstruction` in `plugins/windows/js/index.ts`): while sign-in, checkout or an
//! integration continues in the browser, the main window becomes a 340×500 card in the
//! screen's top-right corner, then returns to its saved frame.

use std::time::Duration;

use desktop_runtime::deeplink::DeepLink;
use gpui::{
    AnyElement, AnyWindowHandle, App, Context, Div, Entity, EventEmitter, Focusable, FontWeight,
    Hsla, MouseButton, Pixels, SharedString, Subscription, TextRun, WeakEntity, Window, div, img,
    linear_color_stop, linear_gradient, prelude::*, px, svg,
};
use reqwest::Url;

use crate::ui::a11y::Accessible;
use crate::{
    platform::windows::{self, FrameWindow, SavedFrame},
    product::{account, cloud::host::Handoff},
    ui::{
        input::{InputEvent, TextInput},
        theme::{MEDIUM, Theme, monospace_font, system_font, theme},
    },
};

pub const CARD: (f32, f32) = (340., 500.);
/// The text column: the card less the body's `p-6` and the column's `px-10`.
const COLUMN: f32 = CARD.0 - 2. * 24. - 2. * 40.;

#[derive(Clone, Copy, PartialEq)]
enum Wrap {
    Greedy,
    /// `text-wrap: pretty`: a one-word last line is avoided with the least ragged lines.
    Pretty,
    /// `text-wrap: balance`, which shipping sets on headings.
    Balance,
}

/// Wraps `text` at word boundaries by shaped widths, as the browser does (GPUI's wrapper sums
/// unkerned advances and breaks earlier). Balancing keeps the line count at the narrowest
/// width. Each line is its own element, so centered lines keep their underlines.
fn lines(
    text: &str,
    size: Pixels,
    weight: FontWeight,
    wrap: Wrap,
    window: &Window,
    cx: &App,
) -> Vec<SharedString> {
    let font = gpui::Font {
        weight,
        ..gpui::font(system_font(cx))
    };
    let width_of = |line: &str| {
        let run = TextRun {
            len: line.len(),
            font: font.clone(),
            color: Hsla::default(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        window
            .text_system()
            .shape_line(line.to_owned().into(), size, &[run], None)
            .width
    };
    let wrap_at = |width: Pixels| {
        let mut lines: Vec<String> = Vec::new();
        for word in text.split(' ') {
            match lines.last_mut() {
                Some(line) if width_of(&format!("{line} {word}")) <= width => {
                    line.push(' ');
                    line.push_str(word);
                }
                _ => lines.push(word.to_owned()),
            }
        }
        lines
    };
    let mut best = wrap_at(px(COLUMN));
    if wrap == Wrap::Pretty && best.len() > 1 && best.last().is_some_and(|line| !line.contains(' '))
    {
        let words = text.split(' ').collect::<Vec<_>>();
        let slack = |line: &[&str]| {
            let width = width_of(&line.join(" "));
            (width <= px(COLUMN)).then(|| f32::from(px(COLUMN) - width))
        };
        if let Some((_, lines)) = least_ragged(&words, best.len(), &slack) {
            best = lines;
        }
    }
    if wrap == Wrap::Balance && best.len() > 1 {
        let (mut low, mut high) = (px(0.), px(COLUMN));
        while high - low > px(1.) {
            let middle = (low + high) / 2.;
            let candidate = wrap_at(middle);
            if candidate.len() == best.len() {
                high = middle;
                best = candidate;
            } else {
                low = middle;
            }
        }
    }
    best.into_iter().map(Into::into).collect()
}

/// Splits `words` into `count` fitting lines, the last of at least two words, minimizing the
/// squared slack of the other lines.
fn least_ragged(
    words: &[&str],
    count: usize,
    slack: &dyn Fn(&[&str]) -> Option<f32>,
) -> Option<(f32, Vec<String>)> {
    if count == 1 {
        return (words.len() > 1 && slack(words).is_some()).then(|| (0., vec![words.join(" ")]));
    }
    (1..words.len())
        .filter_map(|split| {
            let room = slack(&words[..split])?;
            let (cost, mut lines) = least_ragged(&words[split..], count - 1, slack)?;
            lines.insert(0, words[..split].join(" "));
            Some((cost + room * room, lines))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
}

fn text_block(lines: Vec<SharedString>, line_height: f32) -> Div {
    div().flex().flex_col().items_center().children(
        lines
            .into_iter()
            .map(move |line| div().line_height(px(line_height)).child(line)),
    )
}

pub enum InstructionEvent {
    /// A pasted sign-in callback link, for the deep-link handler.
    Callback(String),
    /// The card closed; a billing card refreshes the account (shipping's `onCleanup`).
    Dismissed(Handoff),
}

pub struct Instruction {
    main: AnyWindowHandle,
    card: Option<(Handoff, Url)>,
    saved: Option<SavedFrame>,
    callback: Entity<TextInput>,
    pasting: bool,
    _subscriptions: [Subscription; 2],
}

impl EventEmitter<InstructionEvent> for Instruction {}

/// The frame to restore after this hand-off: the one an earlier card saved, so Back returns to
/// the real window, or the current one. `None` keeps the window and skips the card, as
/// shipping does when it cannot save the frame.
fn frame_to_restore(earlier: Option<SavedFrame>, window: &dyn FrameWindow) -> Option<SavedFrame> {
    earlier.or_else(|| match windows::save_frame(window) {
        Ok(saved) => Some(saved),
        Err(error) => {
            tracing::warn!(%error, "browser hand-off keeps the window frame");
            None
        }
    })
}

/// Shipping's billing refresh and successful integration callbacks close the card, whichever
/// it shows; sign-in closes once an account is signed in.
pub fn dismissed_by(link: &DeepLink) -> bool {
    match link {
        DeepLink::BillingRefresh => true,
        DeepLink::Integration(callback) => callback.status == "success",
        _ => false,
    }
}

/// Opens `url` with the hand-off card in the main window, or plainly when the window frame
/// cannot be driven.
pub fn hand_off(this: WeakEntity<Instruction>, url: Url, handoff: Handoff, cx: &mut App) {
    cx.spawn(async move |cx| {
        let open = |url: &Url, cx: &mut gpui::AsyncApp| cx.update(|cx| cx.open_url(url.as_str()));
        let Ok((main, earlier)) = this.read_with(cx, |this, _| (this.main, this.saved)) else {
            return open(&url, cx);
        };
        // AppKit resizes synchronously, so the window is driven outside any GPUI update.
        let Some(window) = main
            .update(cx, |_, window, _| windows::frame_window(window))
            .ok()
            .flatten()
        else {
            return open(&url, cx);
        };
        let Some(saved) = frame_to_restore(earlier, &*window) else {
            return open(&url, cx);
        };
        let _ = this.update(cx, |this, cx| {
            this.show(handoff, url.clone(), Some(saved), cx)
        });
        if let Err(error) = windows::show_top_right(&*window, CARD.0, CARD.1) {
            tracing::warn!(%error, "browser hand-off could not move the window");
        }
        open(&url, cx);
    })
    .detach();
}

impl Instruction {
    pub fn new(main: AnyWindowHandle, cx: &mut Context<Self>) -> Self {
        let callback =
            cx.new(|cx| TextInput::new("anarlog://auth/callback?access_token=...", cx).inline());
        let _subscriptions = [
            cx.observe_global::<account::Account>(|this, cx| {
                if matches!(this.card, Some((Handoff::SignIn, _)))
                    && account::current(cx).signed_in()
                {
                    this.dismiss(cx);
                }
            }),
            cx.subscribe(&callback, |this, _, event, cx| match event {
                InputEvent::Submitted => this.submit(cx),
                _ => cx.notify(),
            }),
        ];
        Self {
            main,
            card: None,
            saved: None,
            callback,
            pasting: false,
            _subscriptions,
        }
    }

    pub fn is_active(&self) -> bool {
        self.card.is_some()
    }

    /// Shows the card without moving the window; `saved` is the frame Back restores.
    pub fn show(
        &mut self,
        handoff: Handoff,
        url: Url,
        saved: Option<SavedFrame>,
        cx: &mut Context<Self>,
    ) {
        if self.card.is_none() {
            self.pasting = false;
            self.callback
                .update(cx, |input, cx| input.set_text(String::new(), cx));
        }
        self.card = Some((handoff, url));
        self.saved = self.saved.or(saved);
        cx.notify();
    }

    /// Shipping's `dismissInstruction`: the app returns first, then the window grows back.
    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        let Some((handoff, _)) = self.card.take() else {
            return;
        };
        let saved = self.saved.take();
        cx.emit(InstructionEvent::Dismissed(handoff));
        cx.notify();
        let main = self.main;
        cx.spawn(async move |_, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            if let Some(window) = main
                .update(cx, |_, window, _| windows::frame_window(window))
                .ok()
                .flatten()
            {
                windows::restore_frame(&*window, saved);
            }
        })
        .detach();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let text = self.callback.read(cx).buffer.text.clone();
        if !text.is_empty() {
            cx.emit(InstructionEvent::Callback(text));
        }
    }

    fn back_button(&self, colors: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let hover = colors.muted.opacity(0.7);
        div()
            .id("instruction-back")
            .h(px(36.))
            .px_3()
            .flex()
            .items_center()
            .gap(px(6.))
            .text_color(colors.muted_foreground)
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _, _, cx| this.dismiss(cx)))
            .child(
                svg()
                    .path("workspace/ArrowLeft01Icon.svg")
                    .size(px(16.))
                    .text_color(colors.muted_foreground),
            )
            .child(
                div()
                    .text_xs()
                    .line_height(px(16.))
                    .font_weight(MEDIUM)
                    .child("Back"),
            )
    }

    /// `Button variant="outline"` with `bg-card text-muted-foreground h-10 w-full`.
    fn reopen(&self, label: &'static str, url: &Url, colors: &Theme) -> AnyElement {
        let url = url.to_string();
        let hover = colors.background;
        div()
            .id("instruction-reopen")
            .w_full()
            .h(px(40.))
            .px_4()
            .flex()
            .items_center()
            .justify_center()
            .gap_2()
            .border_1()
            .border_color(colors.border)
            .bg(colors.card)
            .shadow_xs()
            .text_sm()
            .font_weight(MEDIUM)
            .text_color(colors.muted_foreground)
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |_, _, cx| cx.open_url(&url))
            .child(label)
            .child(
                svg()
                    .path("product/ExternalLinkIcon.svg")
                    .size(px(14.))
                    .text_color(colors.muted_foreground),
            )
            .into_any_element()
    }

    fn paste(&self, window: &Window, colors: &Theme, cx: &mut Context<Self>) -> AnyElement {
        if !self.pasting {
            let hover = colors.muted_foreground;
            return div()
                .id("instruction-paste")
                .text_xs()
                .line_height(px(16.))
                .font_weight(MEDIUM)
                .text_color(colors.muted_foreground)
                .cursor_pointer()
                .hover(move |style| style.text_color(hover))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(|this, _, _, cx| {
                    this.pasting = true;
                    cx.notify();
                }))
                .child(
                    div().flex().flex_col().items_center().children(
                        lines(
                            "Browser handoff not working? Paste the callback link instead",
                            px(12.),
                            MEDIUM,
                            Wrap::Greedy,
                            window,
                            cx,
                        )
                        .into_iter()
                        .map(|line| div().underline().child(line)),
                    ),
                )
                .into_any_element();
        }
        let focused = self.callback.focus_handle(cx).is_focused(window);
        let enabled = !self.callback.read(cx).buffer.text.is_empty();
        let hover = colors.primary.opacity(0.9);
        div()
            .w_full()
            .flex()
            .flex_col()
            .items_center()
            .gap_3()
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_2()
                    // `Input` with `h-10 font-mono text-xs`.
                    .child(
                        div()
                            .h(px(40.))
                            .px_3()
                            .flex()
                            .items_center()
                            .border_1()
                            .border_color(if focused { colors.ring } else { colors.input })
                            // `bg-transparent` over the card; GPUI paints shadows under the box.
                            .bg(colors.card)
                            .shadow_xs()
                            .font_family(monospace_font(cx))
                            .text_xs()
                            .text_left()
                            .child(self.callback.clone()),
                    )
                    .child(
                        div()
                            .id("instruction-submit")
                            .a11y_button("Submit callback URL")
                            .h(px(40.))
                            .px_4()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(colors.primary)
                            .text_color(colors.primary_foreground)
                            .shadow_xs()
                            .text_sm()
                            .font_weight(MEDIUM)
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .when(!enabled, |view| view.opacity(0.5))
                            .when(enabled, |view| {
                                view.cursor_pointer()
                                    .hover(move |style| style.bg(hover))
                                    .on_click(cx.listener(|this, _, _, cx| this.submit(cx)))
                            })
                            .child("Submit callback URL"),
                    ),
            )
            .child(
                text_block(
                    lines(
                        "Paste the browser URL here if the browser button did not reopen Anarlog.",
                        px(12.),
                        FontWeight::NORMAL,
                        Wrap::Pretty,
                        window,
                        cx,
                    ),
                    20.,
                )
                .text_xs()
                .text_color(colors.muted_foreground),
            )
            .into_any_element()
    }
}

impl super::ApplicationView {
    /// Call whenever the main window hides (tray Hide, close-to-hide), so it re-shows at its
    /// saved frame instead of as the hand-off card.
    pub fn main_window_hidden(&mut self, cx: &mut Context<Self>) {
        self.instruction.update(cx, |card, cx| card.dismiss(cx));
    }

    /// The window while the card shows; quitting still works from the keyboard.
    pub(super) fn instruction_root(&self, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .size_full()
            .capture_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                let modifiers = event.keystroke.modifiers;
                if event.keystroke.key == "q" && (modifiers.platform || modifiers.control) {
                    this.request_quit(cx);
                    cx.stop_propagation();
                }
            }))
            .child(self.instruction.clone())
    }

    pub(super) fn instruction_event(
        &mut self,
        event: &InstructionEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            // Shipping's `handleAuthCallback` accepts only sign-in callbacks.
            InstructionEvent::Callback(raw) => match DeepLink::parse(raw) {
                Ok(DeepLink::Auth(_)) => self.native_event(
                    crate::native_events::NativeEvent::DeepLink {
                        id: 0,
                        raw: raw.clone(),
                    },
                    window,
                    cx,
                ),
                _ => tracing::warn!("The pasted link is not a sign-in callback"),
            },
            InstructionEvent::Dismissed(Handoff::Billing) => {
                let Some(cloud) = self
                    .native
                    .as_ref()
                    .and_then(|native| native.cloud.service.clone())
                else {
                    return;
                };
                cx.spawn(async move |_, cx| {
                    if let Err(error) = cloud.refresh().await {
                        tracing::warn!(%error, "account refresh after checkout failed");
                    }
                    cx.update(|cx| cloud.publish_account(cx));
                })
                .detach();
            }
            InstructionEvent::Dismissed(_) => {}
        }
    }
}

/// `getIntegrationInstruction`: the integrations with their own name and logo.
fn integration(id: &str, colors: &Theme) -> Option<(&'static str, AnyElement)> {
    let logo = |path: &'static str| img(path).size(px(56.)).into_any_element();
    Some(match id {
        "google-calendar" => ("Google Calendar", logo("workspace/GoogleCalendar.svg")),
        "outlook" => ("Outlook", logo("workspace/OutlookIcon.svg")),
        "github" => (
            "GitHub",
            svg()
                .path("product/GithubLogo.svg")
                .size(px(56.))
                .text_color(colors.foreground)
                .into_any_element(),
        ),
        "slack" => ("Slack", logo("sharing/SlackLogo.svg")),
        _ => return None,
    })
}

impl Render for Instruction {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let Some((handoff, url)) = self.card.clone() else {
            return div().id("instruction");
        };
        let (title, description, icon, action, extra): (
            SharedString,
            &str,
            Option<AnyElement>,
            Option<AnyElement>,
            Option<AnyElement>,
        ) = match &handoff {
            Handoff::SignIn => (
                "Sign in to your account".into(),
                "Complete sign-in in your browser, then return to Anarlog.",
                None,
                None,
                Some(self.paste(window, &colors, cx)),
            ),
            Handoff::Billing => (
                "Upgrade to Pro".into(),
                "Finish checkout in your browser to unlock more, then return to Anarlog.",
                None,
                Some(self.reopen("Reopen checkout page", &url, &colors)),
                None,
            ),
            Handoff::Integration(id) => {
                let known = integration(id, &colors);
                (
                    known
                        .as_ref()
                        .map_or("Connect your integration".into(), |(name, _)| {
                            format!("Connect {name}").into()
                        }),
                    "Authorize access in your browser, then return to Anarlog.",
                    known.map(|(_, icon)| icon),
                    Some(self.reopen("Reopen in browser", &url, &colors)),
                    None,
                )
            }
        };
        div()
            .id("instruction")
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(linear_gradient(
                180.,
                linear_color_stop(colors.background, 0.),
                linear_color_stop(colors.card, 0.5),
            ))
            .text_color(colors.foreground)
            // Shipping makes the whole card a window drag region; its controls stop the drag.
            .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(128.))
                    .bg(linear_gradient(
                        180.,
                        linear_color_stop(colors.muted.opacity(0.4), 0.),
                        linear_color_stop(colors.muted.opacity(0.), 1.),
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .px_3()
                    .pt_12()
                    .child(self.back_button(&colors, cx)),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .p_6()
                    .child(
                        div()
                            .w_full()
                            .max_w(px(384.))
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_6()
                            .px_10()
                            .pb_10()
                            .text_center()
                            .child(icon.unwrap_or_else(|| {
                                img("product/AnarlogIcon.png")
                                    .size(px(56.))
                                    .into_any_element()
                            }))
                            .child(
                                div()
                                    .max_w_full()
                                    .flex()
                                    .flex_col()
                                    .gap_3()
                                    .child(
                                        text_block(
                                            lines(
                                                &title,
                                                px(22.),
                                                FontWeight::SEMIBOLD,
                                                Wrap::Balance,
                                                window,
                                                cx,
                                            ),
                                            25.3,
                                        )
                                        .text_size(px(22.))
                                        .font_weight(FontWeight::SEMIBOLD),
                                    )
                                    .child(
                                        text_block(
                                            lines(
                                                description,
                                                px(14.),
                                                FontWeight::NORMAL,
                                                Wrap::Pretty,
                                                window,
                                                cx,
                                            ),
                                            24.,
                                        )
                                        .text_sm()
                                        .text_color(colors.muted_foreground),
                                    ),
                            )
                            .when_some(action, |view, action| view.child(action))
                            .when_some(extra, |view, extra| {
                                view.child(
                                    div()
                                        .w_full()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .gap_3()
                                        .child(extra),
                                )
                            }),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use desktop_runtime::{Result, ServiceError, deeplink::DeepLink};

    use super::*;
    use crate::platform::windows::Frame;

    /// An AppKit-like window: zooming fills the screen and remembers the frame to return to.
    struct FakeWindow {
        frame: Cell<Frame>,
        unzoomed: Cell<Option<Frame>>,
        on_top: Cell<bool>,
        full_screen: bool,
        moves: RefCell<Vec<Frame>>,
    }

    const SCREEN: Frame = Frame {
        x: 0.,
        y: 0.,
        width: 1512.,
        height: 945.,
    };

    impl FakeWindow {
        fn new(frame: Frame) -> Self {
            Self {
                frame: Cell::new(frame),
                unzoomed: Cell::new(None),
                on_top: Cell::new(false),
                full_screen: false,
                moves: RefCell::default(),
            }
        }
    }

    impl FrameWindow for FakeWindow {
        fn frame(&self) -> Result<Frame> {
            if self.full_screen {
                return Err(ServiceError::Unsupported("full screen".into()));
            }
            Ok(self.frame.get())
        }
        fn visible_frame(&self) -> Result<Frame> {
            Ok(SCREEN)
        }
        fn is_maximized(&self) -> bool {
            self.unzoomed.get().is_some()
        }
        fn set_maximized(&self, maximized: bool) {
            if maximized {
                self.unzoomed.set(Some(self.frame.replace(SCREEN)));
            } else if let Some(frame) = self.unzoomed.take() {
                self.frame.set(frame);
            }
        }
        fn set_frame_animated(&self, frame: Frame) {
            self.frame.set(frame);
            self.moves.borrow_mut().push(frame);
        }
        fn set_always_on_top(&self, on_top: bool) {
            self.on_top.set(on_top);
        }
    }

    #[test]
    fn repeated_hand_offs_restore_the_window_saved_by_the_first() {
        let original = Frame {
            x: 200.,
            y: 150.,
            width: 910.,
            height: 600.,
        };
        let window = FakeWindow::new(original);
        window.set_maximized(true);

        let saved = frame_to_restore(None, &window).unwrap();
        windows::show_top_right(&window, CARD.0, CARD.1).unwrap();
        let card = Frame {
            x: SCREEN.width - 340. - 8.,
            y: if cfg!(target_os = "macos") {
                SCREEN.height - 500. - 8.
            } else {
                8.
            },
            width: 340.,
            height: 500.,
        };
        assert_eq!(window.frame.get(), card);
        assert!(window.on_top.get());

        // A second hand-off while the card shows must not save the card as the window.
        assert_eq!(frame_to_restore(Some(saved), &window), Some(saved));
        windows::restore_frame(&window, Some(saved));
        assert!(window.is_maximized());
        assert!(!window.on_top.get());
        window.set_maximized(false);
        assert_eq!(window.frame.get(), original);
    }

    #[test]
    fn a_window_without_a_frame_opens_the_browser_without_the_card() {
        let window = FakeWindow {
            full_screen: true,
            ..FakeWindow::new(SCREEN)
        };
        assert_eq!(frame_to_restore(None, &window), None);
        assert!(window.moves.borrow().is_empty());
        assert!(!window.on_top.get());
    }

    #[test]
    fn billing_and_connected_integrations_close_the_card() {
        let link = |raw: &str| DeepLink::parse(raw).unwrap();
        assert!(dismissed_by(&link("anarlog://billing/refresh")));
        assert!(dismissed_by(&link(
            "anarlog://integration/callback?integration_id=slack&status=success"
        )));
        assert!(!dismissed_by(&link(
            "anarlog://integration/callback?integration_id=slack&status=error"
        )));
    }
}
