//! `OnboardingScreen` (`apps/desktop/src/onboarding/*`): the full-surface
//! first-run flow shown while `store.json`'s `OnboardingNeeded2` is not
//! `false`. Linux/Windows step order is `login → calendar → imports → final`
//! (`STEPS_OTHER`); the permissions step is macOS only.

use gpui::{
    AnyElement, ClickEvent, Context, Div, MouseButton, MouseDownEvent, SharedString, Window, div,
    img, linear_color_stop, linear_gradient, prelude::*, px,
};

use super::Workspace;
use crate::theme::alpha;
use crate::ui::{TailwindText as _, icon};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Step {
    Login,
    Calendar,
    Imports,
    Final,
}

const STEPS: [Step; 4] = [Step::Login, Step::Calendar, Step::Imports, Step::Final];

impl Step {
    fn index(self) -> usize {
        STEPS.iter().position(|step| *step == self).unwrap_or(0)
    }

    fn next(self) -> Option<Step> {
        STEPS.get(self.index() + 1).copied()
    }

    fn prev(self) -> Option<Step> {
        self.index()
            .checked_sub(1)
            .and_then(|i| STEPS.get(i).copied())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CalendarProvider {
    Google,
    Outlook,
}

pub(crate) struct OnboardingState {
    step: Step,
    did_skip_login: bool,
    muted: bool,
    /// `FinalSection` status while `finishOnboarding` runs.
    finishing: bool,
    finish_error: bool,
    /// `isOpening` on the sign-in button until the browser hand-off resolves.
    opening_sign_in: bool,
    hovered_calendar: Option<CalendarProvider>,
    import_menu_open: bool,
}

/// `WELCOME_NOTE` in `onboarding/welcome-note.ts`.
pub(crate) const WELCOME_NOTE: &str = "Welcome to Anarlog 👋
This note is a quick way to see how Anarlog works.
Click **Join & record** in the top-right corner. It will open a private, prerecorded demo meeting, so you don't have to worry about your camera or microphone. Anarlog will save the audio. To create a transcript and notes, choose a provider in **Settings → Transcription**; if one is not ready, Anarlog will show you a setup shortcut.
When the video ends, Anarlog will stop listening. If transcription and intelligence are configured, it will start creating your summary automatically.";

pub(crate) const WELCOME_NOTE_DEMO_URL: &str = "https://anarlog.so/onboarding-demo/";
pub(crate) const WELCOME_NOTE_TRACKING_ID: &str = "anarlog-onboarding-demo-v1";

impl Workspace {
    pub(crate) fn onboarding_open(&self) -> bool {
        self.onboarding.is_some()
    }

    /// `OnboardingNeeded2` defaults to `true` when the store has no value.
    pub(crate) fn start_onboarding_if_needed(&mut self) {
        if self.store_file.onboarding_needed() {
            // `sfxCommands.play("BGM")` on mount.
            self.onboarding_bgm = Some(crate::sfx::Sound::play_bgm());
            self.onboarding = Some(OnboardingState {
                step: Step::Login,
                did_skip_login: false,
                muted: false,
                finishing: false,
                finish_error: false,
                opening_sign_in: false,
                hovered_calendar: None,
                import_menu_open: false,
            });
        }
    }

    fn onboarding_step(&mut self, step: Step, cx: &mut Context<Self>) {
        if let Some(state) = self.onboarding.as_mut() {
            state.step = step;
            state.import_menu_open = false;
            cx.notify();
        }
    }

    fn onboarding_next(&mut self, cx: &mut Context<Self>) {
        if let Some(next) = self.onboarding.as_ref().and_then(|state| state.step.next()) {
            self.onboarding_step(next, cx);
        }
    }

    /// `auth.signIn()`: the browser hand-off to `buildWebAppUrl("/auth")`.
    fn onboarding_sign_in(&mut self, cx: &mut Context<Self>) {
        let url = self.auth_url();
        if let Some(state) = self.onboarding.as_mut() {
            state.opening_sign_in = true;
        }
        cx.open_url(&url);
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(600))
                .await;
            this.update(cx, |this, cx| {
                if let Some(state) = this.onboarding.as_mut() {
                    state.opening_sign_in = false;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// `finishOnboarding`: the welcome session, `setOnboardingNeeded(false)`,
    /// then `openCurrent({ type: "sessions", id })`.
    fn finish_onboarding(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.onboarding.as_mut() else {
            return;
        };
        if state.finishing {
            return;
        }
        state.finishing = true;
        state.finish_error = false;
        cx.notify();
        let task = self.store.get_or_create_welcome_session();
        cx.spawn(async move |this, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            cx.background_executor()
                .timer(std::time::Duration::from_millis(100))
                .await;
            this.update(cx, |this, cx| match result {
                Ok(session_id) => {
                    if let Err(error) = this.store_file.set_onboarding_needed(false) {
                        tracing::error!(%error, "failed to persist OnboardingNeeded2");
                        if let Some(state) = this.onboarding.as_mut() {
                            state.finishing = false;
                            state.finish_error = true;
                        }
                        cx.notify();
                        return;
                    }
                    // `sfxCommands.stop("BGM")` in `finishOnboarding`.
                    this.onboarding_bgm = None;
                    this.onboarding = None;
                    this.select(session_id, cx);
                    cx.notify();
                }
                Err(error) => {
                    tracing::error!(%error, "Failed to finish onboarding");
                    if let Some(state) = this.onboarding.as_mut() {
                        state.finishing = false;
                        state.finish_error = true;
                    }
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// The whole window while onboarding: the video backdrop stand-in, the
    /// `h-12` mute row, the `font-hand text-4xl` title, and the sections.
    pub(super) fn render_onboarding(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let theme = self.theme;
        let Some(state) = self.onboarding.as_ref() else {
            return div().into_any_element();
        };
        let viewport = window.viewport_size();
        let width = f32::from(viewport.width);
        let height = f32::from(viewport.height);
        // `object-cover object-bottom` for the 832×464 frame.
        let scale = (width / 832.0).max(height / 464.0);
        let (frame_w, frame_h) = (832.0 * scale, 464.0 * scale);

        let backdrop = div()
            .absolute()
            .inset_0()
            .overflow_hidden()
            .child(
                // The video's opening frame, blurred, at `opacity-28`; GPUI has
                // no video playback or backdrop blur.
                img(super::note::embedded("onboarding-frame.png"))
                    .absolute()
                    .left(px((width - frame_w) / 2.0))
                    .top(px(height - frame_h))
                    .w(px(frame_w))
                    .h(px(frame_h))
                    .opacity(0.28),
            )
            .child(
                // `from-background/8 via-background/18 to-transparent` (to top).
                div().absolute().inset_0().bg(linear_gradient(
                    0.0,
                    linear_color_stop(alpha(theme.background, 0.08), 0.0),
                    linear_color_stop(alpha(theme.background, 0.0), 1.0),
                )),
            )
            // `from-background via-background/97 via-18% via-background/82
            // via-42% to-background/0` over the top 84%, as three segments.
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top_0()
                    .h(px(height * 0.84 * 0.18))
                    .bg(linear_gradient(
                        180.0,
                        linear_color_stop(theme.background, 0.0),
                        linear_color_stop(alpha(theme.background, 0.97), 1.0),
                    )),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(height * 0.84 * 0.18))
                    .h(px(height * 0.84 * 0.24))
                    .bg(linear_gradient(
                        180.0,
                        linear_color_stop(alpha(theme.background, 0.97), 0.0),
                        linear_color_stop(alpha(theme.background, 0.82), 1.0),
                    )),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(height * 0.84 * 0.42))
                    .h(px(height * 0.84 * 0.58))
                    .bg(linear_gradient(
                        180.0,
                        linear_color_stop(alpha(theme.background, 0.82), 0.0),
                        linear_color_stop(alpha(theme.background, 0.0), 1.0),
                    )),
            );

        let muted = state.muted;
        let mute_row = div()
            .id("onboarding-mute-row")
            .relative()
            .flex()
            .h(px(48.0))
            .flex_shrink_0()
            .items_center()
            .justify_end()
            .pr_3()
            .pl_12()
            .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, window, _| {
                window.start_window_move()
            })
            .child(
                div()
                    .id("onboarding-mute")
                    .rounded(px(8.0))
                    .p(px(6.0))
                    .cursor_pointer()
                    .hover(move |style| style.bg(theme.accent))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if let Some(state) = this.onboarding.as_mut() {
                            state.muted = !state.muted;
                            // `setVolume("BGM", isMuted ? 0 : 0.2)`
                            if let Some(bgm) = this.onboarding_bgm.as_ref() {
                                bgm.set_volume(if state.muted {
                                    0.0
                                } else {
                                    crate::sfx::BGM_VOLUME
                                });
                            }
                            cx.notify();
                        }
                    }))
                    .child(icon(
                        if muted { "speaker-x" } else { "speaker-high" },
                        px(16.0),
                        theme.muted_foreground,
                    )),
            );

        let header = div()
            .id("onboarding-header")
            .relative()
            .flex()
            .flex_shrink_0()
            .items_center()
            .px_12()
            .pt_4()
            .pb_8()
            .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, window, _| {
                window.start_window_move()
            })
            .child(
                div()
                    .font_family(super::settings::hand_font_family())
                    .text_size(px(36.0))
                    .line_height(px(36.0))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.foreground)
                    .child("Welcome to Anarlog"),
            );

        let sections = div().flex().flex_col().gap_4().px_12().pb_16().children(
            STEPS
                .iter()
                .filter_map(|step| self.render_onboarding_section(*step, state, window, cx)),
        );

        div()
            .id("onboarding")
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .bg(theme.card)
            .text_color(theme.foreground)
            .child(backdrop)
            .child(mute_row)
            .child(header)
            .child(
                div()
                    .id("onboarding-scroll")
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(sections),
            )
            .into_any_element()
    }

    /// `OnboardingSection`: nothing for upcoming steps, the green check with
    /// the `text-xs` completed title for finished ones, and the `text-xl`
    /// header with Skip, the description, and the content for the active one.
    fn render_onboarding_section(
        &self,
        step: Step,
        state: &OnboardingState,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<Div> {
        let theme = self.theme;
        let current = state.step.index();
        let index = step.index();
        if index > current {
            return None;
        }
        let (title, completed_title, description, skippable): (&str, &str, Option<AnyElement>, bool) = match step {
            Step::Login => (
                "Create account",
                if state.did_skip_login { "Skipped" } else { "Account" },
                Some(
                    div()
                        .child("Sign in to unlock powerful AI models, sync across devices, and personalization.")
                        .into_any_element(),
                ),
                true,
            ),
            Step::Calendar => (
                "Connect calendar",
                "Calendar connected",
                Some(div().child("Anarlog will sync your calendar to get meeting reminders").into_any_element()),
                true,
            ),
            Step::Imports => (
                "Bring your meeting history",
                "Meeting history imported",
                Some(
                    div()
                        .child("Import notes and transcripts from the meeting apps you already use.")
                        .into_any_element(),
                ),
                true,
            ),
            Step::Final => ("Ready to go", "Ready to go", Some(self.render_final_description(cx)), false),
        };

        if index < current {
            return Some(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(icon("check", px(16.0), gpui::rgb(0x00a63e)))
                    .child(
                        div().flex().min_w_0().flex_col().gap_3().child(
                            div()
                                .tw_text_xs()
                                .text_color(alpha(theme.muted_foreground, 0.7))
                                .child(SharedString::from(completed_title.to_string())),
                        ),
                    ),
            );
        }

        let dev = cfg!(debug_assertions);
        let controls = div()
            .flex()
            .items_center()
            .gap_2()
            .when(dev && step.prev().is_some(), |row| {
                row.child(
                    div()
                        .id("onboarding-back")
                        .rounded(px(4.0))
                        .p(px(2.0))
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(prev) = this.onboarding.as_ref().and_then(|s| s.step.prev())
                            {
                                this.onboarding_step(prev, cx);
                            }
                        }))
                        .child(icon("caret-left", px(12.0), theme.muted_foreground)),
                )
            })
            .when(skippable, |row| {
                row.child(
                    div()
                        .id("onboarding-skip")
                        .flex()
                        .items_center()
                        .gap_1()
                        .tw_text_sm()
                        .text_color(theme.muted_foreground)
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if step == Step::Login
                                && let Some(state) = this.onboarding.as_mut()
                            {
                                state.did_skip_login = true;
                            }
                            this.onboarding_next(cx);
                        }))
                        .child("Skip")
                        .child(icon("caret-right", px(12.0), theme.muted_foreground)),
                )
            })
            .when(!skippable && dev, |row| {
                // The final step's `onNext` is `finishOnboarding`.
                row.child(
                    div()
                        .id("onboarding-next")
                        .rounded(px(4.0))
                        .p(px(2.0))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if step == Step::Final {
                                this.finish_onboarding(cx);
                            } else {
                                this.onboarding_next(cx);
                            }
                        }))
                        .child(icon("caret-right", px(12.0), theme.muted_foreground)),
                )
            });

        let content = match step {
            Step::Login => self.render_login_section(state, cx),
            Step::Calendar => self.render_calendar_section(state, cx),
            Step::Imports => self.render_import_section(window, cx),
            Step::Final => self.render_final_section(state, cx),
        };

        Some(
            div()
                .flex()
                .flex_col()
                .child(
                    div().flex().items_center().gap_2().mb_3().pt_4().child(
                        div()
                            .flex()
                            .min_w_0()
                            .flex_col()
                            .gap_3()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_size(px(20.0))
                                            .line_height(px(28.0))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(theme.foreground)
                                            .child(SharedString::from(title.to_string())),
                                    )
                                    .child(controls),
                            )
                            .children(description.map(|description| {
                                div()
                                    .tw_text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child(description)
                            })),
                    ),
                )
                // `-mx-5 -mb-5 px-5 pt-3 pb-5`: the content sits 12px under the header.
                .child(div().pt_3().child(content)),
        )
    }

    /// `OnboardingButton variant="primary"`: `rounded-full border-2 px-6
    /// py-2.5 text-sm font-medium` with the warm shadow.
    fn onboarding_primary_button(
        &self,
        id: &'static str,
        label: &'static str,
        busy: bool,
        on_click: impl Fn(&mut Workspace, &mut Window, &mut Context<Workspace>) + 'static,
        cx: &Context<Self>,
    ) -> gpui::Stateful<Div> {
        // Both callers override the base `py-2.5` with `py-2`.
        let (spinner, disabled) = (busy, busy);
        let theme = self.theme;
        div()
            .id(id)
            .flex()
            .items_center()
            .gap_2()
            .px_6()
            .py_2()
            .rounded(px(8.0))
            .border_2()
            .border_color(theme.primary)
            .bg(theme.primary)
            .text_color(theme.primary_foreground)
            .tw_text_sm()
            .font_weight(gpui::FontWeight::MEDIUM)
            .shadow(vec![
                gpui::BoxShadow {
                    color: gpui::Rgba {
                        r: 87.0 / 255.0,
                        g: 83.0 / 255.0,
                        b: 78.0 / 255.0,
                        a: 0.22,
                    }
                    .into(),
                    offset: gpui::point(px(0.0), px(2.0)),
                    blur_radius: px(6.0),
                    spread_radius: px(0.0),
                },
                gpui::BoxShadow {
                    color: gpui::Rgba {
                        r: 87.0 / 255.0,
                        g: 83.0 / 255.0,
                        b: 78.0 / 255.0,
                        a: 0.65,
                    }
                    .into(),
                    offset: gpui::point(px(0.0), px(10.0)),
                    blur_radius: px(18.0),
                    spread_radius: px(-10.0),
                },
            ])
            .when(disabled, |button| button.opacity(0.7))
            .when(!disabled, |button| {
                button
                    .cursor_pointer()
                    .hover(move |style| style.bg(alpha(theme.primary, 0.9)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        on_click(this, window, cx)
                    }))
            })
            .when(spinner, |button| {
                button.child(icon("circle-notch", px(14.0), theme.primary_foreground))
            })
            .child(label)
    }

    /// `BeforeLogin`: the Get started button (`px-6 py-2`).
    fn render_login_section(&self, state: &OnboardingState, cx: &Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(self.onboarding_primary_button(
                        "onboarding-get-started",
                        "Get started",
                        state.opening_sign_in,
                        |this, _, cx| this.onboarding_sign_in(cx),
                        cx,
                    )),
            )
            .into_any_element()
    }

    /// `CalendarSectionContent` while signed out: the Google and Outlook
    /// `min-w-56 flex-1` buttons whose label slides to `Sign in to connect`
    /// on hover; clicking runs `handleCalendarSignIn` (back to the login step
    /// and the browser hand-off).
    fn render_calendar_section(&self, state: &OnboardingState, cx: &Context<Self>) -> AnyElement {
        let theme = self.theme;
        let provider_button =
            |provider: CalendarProvider, label: &'static str, glyph: &'static str| {
                let hovered = state.hovered_calendar == Some(provider);
                let id: &'static str = match provider {
                    CalendarProvider::Google => "onboarding-calendar-google",
                    CalendarProvider::Outlook => "onboarding-calendar-outlook",
                };
                div()
                    .id(id)
                    .flex()
                    .flex_1()
                    .min_w(px(224.0))
                    .h(px(42.0))
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .px_6()
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(if hovered { theme.primary } else { theme.border })
                    .bg(if hovered { theme.primary } else { theme.muted })
                    .text_color(if hovered {
                        theme.primary_foreground
                    } else {
                        theme.foreground
                    })
                    .tw_text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .cursor_pointer()
                    .shadow(vec![
                        gpui::BoxShadow {
                            color: gpui::Rgba {
                                r: 87.0 / 255.0,
                                g: 83.0 / 255.0,
                                b: 78.0 / 255.0,
                                a: 0.01,
                            }
                            .into(),
                            offset: gpui::point(px(0.0), px(2.0)),
                            blur_radius: px(6.0),
                            spread_radius: px(0.0),
                        },
                        gpui::BoxShadow {
                            color: gpui::Rgba {
                                r: 87.0 / 255.0,
                                g: 83.0 / 255.0,
                                b: 78.0 / 255.0,
                                a: 0.1,
                            }
                            .into(),
                            offset: gpui::point(px(0.0), px(10.0)),
                            blur_radius: px(18.0),
                            spread_radius: px(-10.0),
                        },
                    ])
                    .on_hover(cx.listener(move |this, entered: &bool, _, cx| {
                        if let Some(state) = this.onboarding.as_mut() {
                            state.hovered_calendar = entered.then_some(provider);
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        // `handleCalendarSignIn`
                        this.onboarding_step(Step::Login, cx);
                        this.onboarding_sign_in(cx);
                    }))
                    .when(!hovered, |button| {
                        button
                            .child(img(super::note::embedded(glyph)).size(px(16.0)))
                            .child(
                                div()
                                    .text_size(px(16.0))
                                    .line_height(px(24.0))
                                    .font_weight(gpui::FontWeight::NORMAL)
                                    .child(label),
                            )
                    })
                    .when(hovered, |button| button.child("Sign in to connect"))
            };
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .child(provider_button(
                        CalendarProvider::Google,
                        "Google",
                        "brands/google-calendar.svg",
                    ))
                    .child(provider_button(
                        CalendarProvider::Outlook,
                        "Outlook",
                        "brands/outlook.svg",
                    )),
            )
            .into_any_element()
    }

    /// `MeetingImportScreen compact` with nothing detected: the always
    /// available Google Meet row with its `Connect & import` split button,
    /// then the secondary `Skip for now`.
    fn render_import_section(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let theme = self.theme;
        div()
            .flex()
            .flex_col()
            .items_start()
            .gap_4()
            .max_w(px(768.0))
            .child(self.render_meeting_import_card(true, window, cx))
            .child(
                // `OnboardingButton variant="secondary"` (`px-6 py-2`).
                div()
                    .id("onboarding-skip-for-now")
                    .flex()
                    .w_auto()
                    .px_6()
                    .py_2()
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(alpha(theme.border, 0.6))
                    .bg(alpha(theme.card, 0.55))
                    .text_color(theme.muted_foreground)
                    .tw_text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .cursor_pointer()
                    .hover(move |style| {
                        style
                            .bg(alpha(theme.card, 0.75))
                            .text_color(theme.foreground)
                    })
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.onboarding_next(cx)))
                    .child("Skip for now"),
            )
            .into_any_element()
    }

    /// `MeetingImportScreen` with nothing detected: the `rounded-2xl` card
    /// with the always available Google Meet row (`min-h-16 px-4 py-3`),
    /// its `text-xs` description (a `p`, so `text-wrap: pretty`), and the
    /// `Connect & import` split button; `compact` caps the list at `max-h-80`
    /// inside the onboarding's `max-w-3xl`.
    pub(super) fn render_meeting_import_card(
        &self,
        compact: bool,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let theme = self.theme;
        let import_menu_open = self
            .onboarding
            .as_ref()
            .is_some_and(|state| state.import_menu_open);
        // The card's width: the onboarding's `max-w-3xl`, or the settings
        // content column (`viewport - sidebar - padding`).
        let card_width = if compact {
            768.0
        } else {
            (f32::from(window.viewport_size().width) - 253.0).max(320.0)
        };
        let split = div()
            .relative()
            .flex()
            .flex_shrink_0()
            .w(px(162.0))
            .items_center()
            .rounded(px(8.0))
            .border_1()
            .border_color(theme.border)
            .overflow_hidden()
            .child(
                div()
                    .id("onboarding-import-connect")
                    .flex()
                    .flex_1()
                    .h(px(28.0))
                    .items_center()
                    .gap_2()
                    .px_2()
                    .tw_text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .bg(theme.muted)
                    .text_color(theme.foreground)
                    .cursor_pointer()
                    .hover(move |style| {
                        style.bg(theme.primary).text_color(theme.primary_foreground)
                    })
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| this.onboarding_sign_in(cx)),
                    )
                    .child(icon("plugs-connected", px(14.0), theme.foreground))
                    .child("Connect & import"),
            )
            .child(
                div()
                    .id("onboarding-import-files")
                    .relative()
                    .flex()
                    .w(px(24.0))
                    .h(px(28.0))
                    .items_center()
                    .justify_center()
                    .bg(theme.muted)
                    .cursor_pointer()
                    .hover(move |style| style.bg(theme.accent))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if let Some(state) = this.onboarding.as_mut() {
                            state.import_menu_open = !state.import_menu_open;
                            cx.notify();
                        }
                    }))
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .top(px(6.0))
                            .bottom(px(6.0))
                            .w(px(1.0))
                            .bg(theme.border),
                    )
                    .child(icon("caret-down", px(14.0), theme.foreground))
                    .when(import_menu_open, |trigger| {
                        trigger.child(self.render_import_files_menu(cx))
                    }),
            );

        let row = div()
            .flex()
            .min_h(px(64.0))
            .items_center()
            .gap_3()
            .px_4()
            .py_3()
            .child(
                div()
                    .flex()
                    .size(px(32.0))
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .child(img(super::note::embedded("google-meet.svg")).size(px(32.0))),
            )
            .child(
                // The text column's width follows from the card width: GPUI
                // measures wrapped text at max-content inside `flex-1`.
                div()
                    .w(px(card_width - 2.0 - 32.0 - 32.0 - 24.0 - 162.0))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .tw_text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child("Google Meet"),
                    )
                    .child(div().mt_1().child({
                        let mut style = window.text_style();
                        style.font_size = px(12.0).into();
                        style.color = theme.muted_foreground.into();
                        if let Some(font) = &self.font_family {
                            style.font_family = font.clone();
                        }
                        let text = "Connect once to bring over your Google Meet history and keep new meetings coming in while you switch.";
                        crate::prose_text::ProseText::new(
                            text.to_string(),
                            vec![style.to_run(text.len())],
                            px(12.0),
                            px(16.0),
                        )
                        .pretty()
                        .max_width(px(card_width - 2.0 - 32.0 - 32.0 - 24.0 - 162.0))
                    })),
            )
            .child(div().flex().flex_shrink_0().items_center().gap_1().child(split));

        div()
            .w_full()
            .rounded(px(16.0))
            .border_1()
            .border_color(theme.border)
            .bg(theme.card)
            .overflow_hidden()
            .child(div().when(compact, |list| list.max_h(px(320.0))).child(row))
            .into_any_element()
    }

    /// The `w-40` app menu under the split button's caret with `Use files`.
    fn render_import_files_menu(&self, cx: &Context<Self>) -> AnyElement {
        let theme = self.theme;
        let panel = super::menu::menu_chrome(theme, "onboarding-import-menu", 160.0)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                if let Some(state) = this.onboarding.as_mut() {
                    state.import_menu_open = false;
                    cx.notify();
                }
            }))
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
                    .child(
                        div()
                            .id("onboarding-import-use-files")
                            .relative()
                            .flex()
                            .h(px(32.0))
                            .items_center()
                            .gap_2()
                            .rounded(px(14.0))
                            .px_2()
                            .tw_text_sm()
                            .cursor_pointer()
                            .hover(move |style| style.bg(theme.accent))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                if let Some(state) = this.onboarding.as_mut() {
                                    state.import_menu_open = false;
                                }
                                this.pick_import_files(window, cx);
                            }))
                            .child(icon("download-simple", px(14.0), theme.foreground))
                            .child("Use files"),
                    ),
            );
        div()
            .absolute()
            .top(px(32.0))
            .right_0()
            .child(
                gpui::deferred(
                    gpui::anchored()
                        .anchor(gpui::Corner::TopRight)
                        .snap_to_window_with_margin(px(8.0))
                        .child(panel),
                )
                .with_priority(2),
            )
            .into_any_element()
    }

    /// `Use files`: the native picker; the Google Meet export import itself
    /// needs the imports service, so the selection is logged for now.
    fn pick_import_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picker = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: None,
        });
        cx.spawn_in(window, async move |_, _| {
            if let Ok(Ok(Some(paths))) = picker.await {
                tracing::info!(?paths, "[onboarding] Google Meet export files selected");
            }
        })
        .detach();
    }

    /// `FinalDescription`: the community line with the Discord / GitHub / X
    /// buttons (`size-5`, icons 18/18/14) that open the browser.
    fn render_final_description(&self, cx: &Context<Self>) -> AnyElement {
        let theme = self.theme;
        let socials: [(&'static str, &'static str, f32, &'static str); 3] = [
            (
                "Discord",
                "discord-logo",
                18.0,
                "https://anarlog.so/discord",
            ),
            (
                "GitHub",
                "github-logo",
                18.0,
                "https://github.com/fastrepl/anarlog",
            ),
            ("X", "x-logo", 14.0, "https://x.com/anarlogapp"),
        ];
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_x(px(12.0))
            .gap_y(px(8.0))
            .child("Join our community and stay updated:")
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .children(socials.into_iter().map(|(label, glyph, size, url)| {
                        div()
                            .id(SharedString::from(format!("onboarding-social-{label}")))
                            .flex()
                            .size(px(20.0))
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .cursor_pointer()
                            .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| cx.open_url(url)))
                            .child(icon(glyph, px(size), theme.muted_foreground))
                    })),
            )
            .into_any_element()
    }

    /// `FinalSection`: Open Anarlog (`px-6 py-2`), a spinner while finishing,
    /// and the error line when it fails.
    fn render_final_section(&self, state: &OnboardingState, cx: &Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .items_start()
            .gap_2()
            .child(self.onboarding_primary_button(
                "onboarding-open",
                "Open Anarlog",
                state.finishing,
                |this, _, cx| this.finish_onboarding(cx),
                cx,
            ))
            .when(state.finish_error, |column| {
                column.child(
                    div()
                        .tw_text_sm()
                        .text_color(gpui::rgb(0xfb2c36))
                        .child("Couldn't open Anarlog. Please try again."),
                )
            })
            .into_any_element()
    }
}
