//! The onboarding chrome and each step's section, from `onboarding/index.tsx`, `shared.tsx`
//! and the step components.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, App, BoxShadow, ClickEvent, Context, Div, ElementId,
    FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    Stateful, StatefulInteractiveElement, Styled, Transformation, Window, div, img,
    linear_color_stop, linear_gradient, percentage, point, prelude::FluentBuilder, px, relative,
    rgb, rgba,
};

use super::OnboardingView;
use crate::{
    product::{
        controls::{HAND_FONT, dropdown, icon, menu_panel},
        onboarding::{
            Access, Permission, Section, Step, TrialPhase, WORK_EMAIL_TRIAL_DAYS,
            login_completed_title, section,
        },
    },
    ui::theme::{Theme, system_font, theme},
};

const GREEN_600: u32 = 0x00a63e;
const EMERALD_600: u32 = 0x009966;
const AMBER_800: u32 = 0x973c00;
const AMBER_900: u32 = 0x7b3306;
const RED_400: u32 = 0xff6467;
const RED_500: u32 = 0xfb2c36;
/// Shipping's warm shadow tint, `rgba(87,83,78,…)`.
const SHADOW: u32 = 0x57534e;
/// `onboarding-video.mp4`'s frame size.
const FRAME: (f32, f32) = (832., 464.);

fn shadow(color: u32, alpha: f32, y: f32, blur: f32, spread: f32) -> BoxShadow {
    BoxShadow {
        color: Hsla::from(rgb(color)).opacity(alpha),
        offset: point(px(0.), px(y)),
        blur_radius: px(blur),
        spread_radius: px(spread),
        inset: false,
    }
}

fn text(size: f32, line: f32) -> Div {
    div().text_size(px(size)).line_height(px(line))
}

fn spinner(id: impl Into<ElementId>, size: f32, color: Hsla) -> impl IntoElement {
    icon("product/LoaderCircleIcon.svg", size, color).with_animation(
        id,
        Animation::new(Duration::from_secs(1)).repeat(),
        |icon, delta| icon.with_transformation(Transformation::rotate(percentage(delta))),
    )
}

/// `OnboardingButton` (`shared.tsx`) before its variant: `rounded-full` is 8px in shipping's CSS.
fn button(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .gap_2()
        .px_6()
        .py(px(10.))
        .rounded(px(8.))
        .text_size(px(14.))
        .line_height(px(20.))
        .font_weight(crate::ui::theme::MEDIUM)
        .cursor_pointer()
}

fn primary_button(id: impl Into<ElementId>, colors: &Theme) -> Stateful<Div> {
    button(id)
        .border_2()
        .border_color(colors.primary)
        .bg(colors.primary)
        .text_color(colors.primary_foreground)
        .shadow(vec![
            shadow(SHADOW, 0.22, 2., 6., 0.),
            shadow(SHADOW, 0.65, 10., 18., -10.),
        ])
        .hover(|style| style.bg(colors.primary.opacity(0.9)))
}

fn secondary_button(id: impl Into<ElementId>, colors: &Theme) -> Stateful<Div> {
    let muted = colors.muted_foreground;
    let foreground = colors.foreground;
    let card = colors.card;
    div()
        .id(id)
        .flex_none()
        .px_6()
        .py_2()
        .rounded(px(8.))
        .border_1()
        .border_color(colors.border.opacity(0.6))
        .bg(card.opacity(0.55))
        .text_color(muted)
        .text_size(px(14.))
        .line_height(px(20.))
        .font_weight(crate::ui::theme::MEDIUM)
        .shadow(vec![BoxShadow {
            color: rgba(0xffffff8c).into(),
            offset: point(px(0.), px(1.)),
            blur_radius: px(0.),
            spread_radius: px(0.),
            inset: true,
        }])
        .cursor_pointer()
        .hover(move |style| style.bg(card.opacity(0.75)).text_color(foreground))
}

/// `StepRow`.
fn step_row(id: &'static str, status: StepStatus, label: &'static str, colors: &Theme) -> Div {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(match status {
            StepStatus::Done => icon(
                "product/CheckmarkCircle02Icon.svg",
                16.,
                rgb(EMERALD_600).into(),
            )
            .into_any_element(),
            StepStatus::Active => spinner(id, 16., colors.muted_foreground).into_any_element(),
            StepStatus::Failed => {
                icon("product/CancelCircleIcon.svg", 16., rgb(RED_400).into()).into_any_element()
            }
        })
        .child(
            text(14., 20.)
                .text_color(if status == StepStatus::Failed {
                    rgb(RED_500).into()
                } else {
                    colors.muted_foreground
                })
                .child(label),
        )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StepStatus {
    Done,
    Active,
    Failed,
}

/// `TrialOfferCard` (`account/offer-card.tsx`).
fn offer_card(
    days: u32,
    icon_path: Option<&'static str>,
    title: &'static str,
    subtitle: Option<SharedString>,
    colors: &Theme,
) -> Div {
    let amber = Hsla::from(rgb(AMBER_800));
    div()
        .w_full()
        .max_w(px(400.))
        .flex()
        .items_center()
        .gap_5()
        .py_4()
        .pr_6()
        .pl_5()
        .rounded(px(20.))
        .border_1()
        .border_color(Hsla::from(rgb(AMBER_900)).opacity(0.1))
        // `bg-white/75` over the page, made opaque: GPUI paints box shadows under translucent
        // fills, where CSS clips them to the outside of the box.
        .bg(colors.background.blend(gpui::white().opacity(0.75)))
        .shadow(vec![
            BoxShadow {
                color: rgba(0xffffffcc).into(),
                offset: point(px(0.), px(1.)),
                blur_radius: px(0.),
                spread_radius: px(0.),
                inset: true,
            },
            shadow(0x78350f, 0.35, 12., 32., -16.),
        ])
        .child(
            div()
                .w(px(56.))
                .flex_none()
                .flex()
                .flex_col()
                .items_center()
                .child(
                    div()
                        .h(px(40.))
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            text(44., 44.)
                                .font_family(HAND_FONT)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(amber)
                                .child(days.to_string()),
                        ),
                )
                // `tracking-[0.22em]` with `pl-[0.22em]`: GPUI has no letter spacing.
                .child(
                    text(10., 15.)
                        .mt_1()
                        .pl(px(2.2))
                        .flex()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(amber.opacity(0.7))
                        .children("DAYS".chars().map(|letter| {
                            div()
                                .mr(px(2.2))
                                .child(SharedString::from(letter.to_string()))
                        })),
                ),
        )
        .child(
            div()
                .h(px(44.))
                .w(px(1.))
                .flex_none()
                .bg(Hsla::from(rgb(AMBER_900)).opacity(0.1)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .min_w_0()
                .child(
                    text(15., 22.5)
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors.foreground)
                        .when_some(icon_path, |row, path| {
                            row.child(icon(path, 16., rgb(EMERALD_600).into()))
                        })
                        .child(title),
                )
                .when_some(subtitle, |column, subtitle| {
                    column.child(
                        text(13., 19.5)
                            .truncate()
                            .text_color(colors.muted_foreground)
                            .child(subtitle),
                    )
                }),
        )
}

/// `OnboardingSection`'s heading props.
struct Heading {
    title: &'static str,
    completed: &'static str,
    description: Option<AnyElement>,
    skippable: bool,
}

impl OnboardingView {
    fn section(
        &self,
        step: Step,
        heading: Heading,
        colors: &Theme,
        cx: &mut Context<Self>,
        content: impl FnOnce(&Self, &Theme, &mut Context<Self>) -> AnyElement,
    ) -> Option<AnyElement> {
        let Heading {
            title,
            completed: completed_title,
            description,
            skippable,
        } = heading;
        let status = section(self.steps, step, self.current)?;
        if status == Section::Upcoming {
            return None;
        }
        let active = status == Section::Active;
        let header = div()
            .flex()
            .items_center()
            .gap_2()
            .when(active, |row| row.mb_3().pt_4())
            .when(!active, |row| {
                row.child(icon(
                    "product/CheckIconBold.svg",
                    16.,
                    rgb(GREEN_600).into(),
                ))
            })
            .child(
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
                            .child(if active {
                                text(20., 28.)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(colors.foreground)
                                    .child(title)
                            } else {
                                text(12., 16.)
                                    .text_color(colors.muted_foreground.opacity(0.7))
                                    .child(completed_title)
                            })
                            .when(active && skippable, |row| {
                                row.child(
                                    div()
                                        .id(SharedString::from(format!("onboarding-skip-{step:?}")))
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .text_size(px(14.))
                                        .line_height(px(20.))
                                        .text_color(colors.muted_foreground)
                                        .cursor_pointer()
                                        .on_click(
                                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                                this.skip(cx)
                                            }),
                                        )
                                        .child("Skip")
                                        .child(icon(
                                            "product/ArrowRight01Icon.svg",
                                            12.,
                                            colors.muted_foreground,
                                        )),
                                )
                            }),
                    )
                    .when(active, |column| {
                        column.when_some(description, |column, description| {
                            column.child(
                                text(14., 20.)
                                    .text_color(colors.muted_foreground)
                                    .child(description),
                            )
                        })
                    }),
            );
        Some(
            div()
                .flex_none()
                .child(header)
                .when(active, |section| {
                    section.child(div().pt_3().child(content(self, colors, cx)))
                })
                .into_any_element(),
        )
    }

    fn permissions(&self, colors: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let block = |permission: Permission,
                     enabled: &'static str,
                     enable: &'static str,
                     path: &'static str,
                     cx: &mut Context<Self>| {
            let access = match permission {
                Permission::Microphone => self.state.microphone,
                Permission::SystemAudio => self.state.system_audio,
                _ => self.state.accessibility,
            };
            let authorized = access == Access::Authorized;
            let pending = self.state.pending == Some(permission);
            let primary = colors.primary;
            div()
                .id(SharedString::from(format!(
                    "onboarding-permission-{}",
                    permission.id()
                )))
                .group("permission")
                .w_full()
                .min_w_0()
                .flex()
                .items_center()
                .gap_3()
                .px_3()
                .py(px(10.))
                .rounded(px(12.))
                .border_1()
                .map(|block| {
                    if authorized {
                        block.border_color(colors.border).bg(colors.card)
                    } else {
                        block
                            .border_color(primary)
                            .bg(primary)
                            .shadow(vec![shadow(SHADOW, 0.18, 4., 14., 0.)])
                            .cursor_pointer()
                            .hover(move |style| style.bg(primary.opacity(0.9)))
                    }
                })
                .when(pending, |block| block.opacity(0.5))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.act_on_permission(permission, cx)
                }))
                .child(
                    div()
                        .size(px(24.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.))
                        .when(!authorized, |icon| {
                            icon.bg(colors.primary_foreground.opacity(0.1))
                        })
                        .child(if authorized {
                            icon("product/CheckIcon.svg", 14., rgb(GREEN_600).into())
                        } else {
                            icon(path, 14., colors.primary_foreground)
                        }),
                )
                .child(
                    text(14., 20.)
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_weight(crate::ui::theme::MEDIUM)
                        .text_color(if authorized {
                            colors.foreground
                        } else {
                            colors.primary_foreground
                        })
                        .child(if authorized { enabled } else { enable }),
                )
                .when(!authorized, |block| {
                    block.child(
                        icon(
                            "product/ArrowRight02Icon.svg",
                            16.,
                            colors.primary_foreground.opacity(0.7),
                        )
                        .group_hover("permission", |style| style.ml(px(2.)).mr(px(-2.))),
                    )
                })
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(block(
                Permission::Microphone,
                "Anarlog can hear your voice",
                "Help Anarlog listen to you",
                "product/Mic01Icon.svg",
                cx,
            ))
            .child(block(
                Permission::SystemAudio,
                "Anarlog can hear others",
                "Help Anarlog listen to others",
                "product/VolumeHighIcon.svg",
                cx,
            ))
            .child(block(
                Permission::Accessibility,
                "Anarlog can read meeting details",
                "Help Anarlog read meeting activity",
                "product/Cursor01Icon.svg",
                cx,
            ))
            .into_any_element()
    }

    fn login(&self, colors: &Theme, cx: &mut Context<Self>) -> AnyElement {
        if !self.state.signed_in {
            return div()
                .flex()
                .flex_col()
                .items_start()
                .gap_4()
                .child(offer_card(
                    WORK_EMAIL_TRIAL_DAYS,
                    None,
                    "Free Pro with a work email",
                    None,
                    colors,
                ))
                .child(
                    primary_button("onboarding-sign-in", colors)
                        .py_2()
                        .when(self.state.signing_in, |button| {
                            button.opacity(0.7).child(spinner(
                                "onboarding-sign-in-spinner",
                                14.,
                                colors.primary_foreground,
                            ))
                        })
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.sign_in(cx)))
                        .child("Get started"),
                )
                .into_any_element();
        }
        let Some(phase) = self.state.trial else {
            return div()
                .flex()
                .items_center()
                .gap_2()
                .child(icon(
                    "product/CheckmarkCircle02Icon.svg",
                    16.,
                    rgb(EMERALD_600).into(),
                ))
                .child(
                    text(14., 20.)
                        .text_color(rgb(EMERALD_600))
                        .child("Signed in"),
                )
                .into_any_element();
        };
        if let TrialPhase::Started { days: Some(days) } = phase {
            let ends = chrono::Local::now() + chrono::Duration::days(days.into());
            return offer_card(
                days,
                Some("product/CheckmarkCircle02Icon.svg"),
                "Pro trial started",
                Some(format!("Until {}", ends.format("%b %-d")).into()),
                colors,
            )
            .into_any_element();
        }
        let (status, label) = match phase {
            TrialPhase::Checking => (StepStatus::Active, "Checking trial eligibility..."),
            TrialPhase::Starting => (StepStatus::Active, "Starting your trial..."),
            TrialPhase::AlreadyPaid => (StepStatus::Done, "You have an active plan"),
            TrialPhase::AlreadyTrialing => (StepStatus::Done, "You're on a Pro trial"),
            TrialPhase::Started { .. } => (StepStatus::Done, "Pro trial started"),
            TrialPhase::NotEligible => (StepStatus::Done, "Continuing without trial"),
            TrialPhase::Failed => (StepStatus::Failed, "Could not start trial"),
        };
        let started = matches!(phase, TrialPhase::Started { .. });
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .when(!started, |rows| {
                rows.child(step_row(
                    "trial-signed-in",
                    StepStatus::Done,
                    "Signed in",
                    colors,
                ))
            })
            .child(step_row("trial-phase", status, label, colors))
            .into_any_element()
    }

    fn calendar(&self, colors: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let signed_in = self.state.signed_in;
        let outline = |id: &'static str| {
            let accent = colors.accent;
            button(id)
                .w_full()
                .h_full()
                .justify_center()
                .gap_3()
                .border_1()
                .border_color(colors.border)
                .bg(colors.card)
                .text_color(colors.foreground)
                .shadow(vec![
                    shadow(SHADOW, 0.08, 2., 6., 0.),
                    shadow(SHADOW, 0.22, 10., 18., -10.),
                ])
                .hover(move |style| style.bg(accent))
        };
        let provider = |id: &'static str,
                        name: &'static str,
                        connect: &'static str,
                        connected: bool,
                        logo: AnyElement,
                        cx: &mut Context<Self>| {
            let primary = colors.primary;
            let primary_foreground = colors.primary_foreground;
            let opening = self.state.calendar_opening == Some(id);
            let control = if signed_in {
                outline(if id == "google" {
                    "onboarding-calendar-google"
                } else {
                    "onboarding-calendar-outlook"
                })
                .when(opening, |button| button.opacity(0.6))
                .child(if opening {
                    spinner(
                        SharedString::from(format!("calendar-{id}-spinner")),
                        16.,
                        colors.foreground,
                    )
                    .into_any_element()
                } else {
                    logo
                })
                .child(if connected {
                    "Add another account"
                } else {
                    connect
                })
            } else {
                // Signed out, hovering swaps the provider for "Sign in to connect".
                let group = SharedString::from(format!("calendar-{id}"));
                button(SharedString::from(format!("onboarding-calendar-{id}")))
                    .group(group.clone())
                    .w_full()
                    .h_full()
                    .justify_center()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.muted)
                    .text_color(colors.foreground)
                    .shadow(vec![
                        shadow(SHADOW, 0.01, 2., 6., 0.),
                        shadow(SHADOW, 0.1, 10., 18., -10.),
                    ])
                    .hover(move |style| {
                        style
                            .border_color(primary)
                            .bg(primary)
                            .text_color(primary_foreground)
                    })
                    .child(
                        div()
                            .relative()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(div().invisible().child("Sign in to connect"))
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .gap_3()
                                    .group_hover(group.clone(), |style| style.invisible())
                                    .child(logo)
                                    .child(
                                        div()
                                            .font_weight(FontWeight::NORMAL)
                                            .text_color(colors.foreground)
                                            .child(name),
                                    ),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .invisible()
                                    .group_hover(group, |style| style.visible())
                                    .child("Sign in to connect"),
                            ),
                    )
            };
            div()
                .flex()
                .flex_1()
                .min_w(px(224.))
                .child(control.on_click(
                    cx.listener(move |this, _: &ClickEvent, _, cx| this.connect_provider(id, cx)),
                ))
        };
        let apple_authorized = self.state.calendar == Access::Authorized;
        let apple_pending = self.state.pending == Some(Permission::Calendar);
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_stretch()
                    .gap_3()
                    .when(cfg!(target_os = "macos"), |row| {
                        row.child(
                            div().flex().flex_1().min_w(px(224.)).child(
                                outline("onboarding-calendar-apple")
                                    .when(apple_pending, |button| button.opacity(0.6))
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.connect_apple(cx)
                                    }))
                                    .child(
                                        img("workspace/AppleCalendar.png")
                                            .size(px(24.))
                                            .rounded(px(4.)),
                                    )
                                    .child("Connect calendar"),
                            ),
                        )
                    })
                    .child(provider(
                        "google",
                        "Google",
                        "Connect Google Calendar",
                        self.state.google_connected,
                        img("workspace/GoogleCalendar.svg")
                            .w(px(15.))
                            .h(px(16.))
                            .into_any_element(),
                        cx,
                    ))
                    .child(provider(
                        "outlook",
                        "Outlook",
                        "Connect Outlook",
                        self.state.outlook_connected,
                        img("workspace/OutlookIcon.svg")
                            .w(px(16.))
                            .h(px(15.))
                            .into_any_element(),
                        cx,
                    )),
            )
            .when(self.state.enabled_calendars > 0, |column| {
                // `w-fit`: the button keeps its own width in the column.
                column.child(
                    div().flex().child(
                        primary_button("onboarding-calendar-continue", colors)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.advance(cx)))
                            .child("Continue"),
                    ),
                )
            })
            .when(self.state.troubleshooting && !apple_authorized, |column| {
                column.child(self.troubleshooting(colors, cx))
            })
            .into_any_element()
    }

    /// `TroubleShootingLink`; the native permission service has no reset.
    fn troubleshooting(&self, colors: &Theme, cx: &mut Context<Self>) -> Div {
        let foreground = colors.foreground;
        let link = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .underline()
                .cursor_pointer()
                .hover(move |style| style.text_color(foreground))
                .child(label)
        };
        let row = text(14., 20.).text_color(colors.muted_foreground);
        if !self.troubleshooting_actions {
            return row.child(
                link("onboarding-calendar-trouble", "Having trouble?").on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        this.troubleshooting_actions = true;
                        cx.notify();
                    },
                )),
            );
        }
        row.flex()
            .flex_wrap()
            .gap_1()
            .child("You can")
            .child(
                link("onboarding-calendar-request", "Request").on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        if this.live && this.state.pending.is_none() {
                            this.request_calendar(cx)
                        }
                    },
                )),
            )
            .child("or")
            .child(link("onboarding-calendar-open", "Open").on_click(
                cx.listener(|this, _: &ClickEvent, _, cx| this.open_calendar_settings(cx)),
            ))
            .child("permission panel.")
            .child(
                link("onboarding-calendar-back", "Back").on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        this.troubleshooting_actions = false;
                        cx.notify();
                    },
                )),
            )
    }

    /// The compact `MeetingImportScreen`. Without installed-app detection only the
    /// always-available Google Meet import is listed, as shipping does when nothing is found.
    fn imports(&self, colors: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let signed_in = self.state.signed_in;
        let muted = colors.muted_foreground;
        let (group_bg, text_color) = if signed_in {
            (colors.primary, colors.primary_foreground)
        } else {
            (colors.muted, colors.foreground)
        };
        let primary = colors.primary;
        let primary_foreground = colors.primary_foreground;
        let connect = div()
            .w(px(160.))
            .h(px(28.))
            .flex_none()
            .flex()
            .rounded(px(8.))
            .overflow_hidden()
            .bg(group_bg)
            .when(!signed_in, |group| {
                group.border_1().border_color(colors.input)
            })
            .child(
                div()
                    .id("onboarding-import-connect")
                    .group("import-connect")
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .px_2()
                    .text_size(px(12.))
                    .font_weight(crate::ui::theme::MEDIUM)
                    .text_color(text_color)
                    .cursor_pointer()
                    .map(|button| {
                        if signed_in {
                            button.hover(move |style| style.bg(primary_foreground.opacity(0.1)))
                        } else {
                            button.hover(move |style| {
                                style.bg(primary).text_color(primary_foreground)
                            })
                        }
                    })
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.connect_import(cx)))
                    .map(|button| {
                        if signed_in || self.state.signing_in {
                            return button
                                .child(if self.state.signing_in {
                                    spinner("import-sign-in", 14., text_color).into_any_element()
                                } else {
                                    icon("product/ElectricPlugsIcon.svg", 14., text_color)
                                        .into_any_element()
                                })
                                .child(if self.state.signing_in {
                                    "Opening…"
                                } else {
                                    "Connect"
                                });
                        }
                        button
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .group_hover("import-connect", |style| style.invisible())
                                    .child(icon(
                                        "product/ElectricPlugsIcon.svg",
                                        14.,
                                        colors.foreground,
                                    ))
                                    .child("Connect"),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .invisible()
                                    .group_hover("import-connect", |style| style.visible())
                                    .child("Sign in to connect"),
                            )
                    }),
            )
            .child(
                div()
                    .id("onboarding-import-files")
                    .w(px(24.))
                    .h_full()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .relative()
                    .cursor_pointer()
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .top(px(6.))
                            .bottom(px(6.))
                            .w(px(1.))
                            .bg(if signed_in {
                                colors.primary_foreground.opacity(0.2)
                            } else {
                                colors.border
                            }),
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.import_menu = !this.import_menu;
                        cx.notify();
                    }))
                    .child(icon("product/ArrowDown01Icon.svg", 14., text_color)),
            );
        let menu = self.import_menu.then(|| {
            dropdown(
                menu_panel(160., colors).child(
                    div()
                        .id("onboarding-import-use-files")
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py(px(6.))
                        .rounded(px(8.))
                        .text_size(px(14.))
                        .line_height(px(20.))
                        .cursor_pointer()
                        .hover(|style| style.bg(colors.accent))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.import_files(cx)))
                        .child(icon("product/Download01Icon.svg", 16., colors.foreground))
                        .child("Use files"),
                ),
                true,
            )
        });
        div()
            .max_w(px(768.))
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .rounded(px(16.))
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.card)
                    .child(
                        div()
                            .min_h(px(64.))
                            .flex()
                            .items_center()
                            .gap_3()
                            .px_4()
                            .py_3()
                            .child(
                                div()
                                    .id("onboarding-import-google-meet")
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.imports_open = !this.imports_open;
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .size(px(32.))
                                            .flex_none()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(img("product/GoogleMeet.png").size(px(32.))),
                                    )
                                    .child(
                                        text(14., 20.)
                                            .truncate()
                                            .font_weight(crate::ui::theme::MEDIUM)
                                            .child("Google Meet"),
                                    )
                                    .child(icon("product/ArrowDown01Icon.svg", 14., muted).when(
                                        self.imports_open,
                                        |icon| {
                                            icon.with_transformation(Transformation::rotate(
                                                percentage(0.5),
                                            ))
                                        },
                                    )),
                            )
                            .child(div().relative().child(connect).children(menu)),
                    )
                    .when(self.imports_open, |card| {
                        card.child(
                            text(12., 16.)
                                .px_4()
                                .pb_4()
                                .pl(px(60.))
                                .mt_1()
                                .text_color(muted)
                                .child(
                                    self.state
                                        .imported
                                        .clone()
                                        .unwrap_or_else(|| "No imports yet.".into()),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .when(self.state.imported.is_some(), |row| {
                        row.child(
                            primary_button("onboarding-imports-continue", colors)
                                .on_click(
                                    cx.listener(|this, _: &ClickEvent, _, cx| this.advance(cx)),
                                )
                                .child("Continue"),
                        )
                    })
                    .child(
                        secondary_button("onboarding-imports-skip", colors)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.skip(cx)))
                            .child("Skip for now"),
                    ),
            )
            .into_any_element()
    }

    fn final_description(&self, colors: &Theme) -> AnyElement {
        let social = |id: &'static str, path: &'static str, size: f32, url: &'static str| {
            div()
                .id(id)
                .size(px(20.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(6.))
                .cursor_pointer()
                .on_click(move |_, _, cx: &mut App| cx.open_url(url))
                .child(icon(path, size, colors.muted_foreground))
        };
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_x_3()
            .gap_y_2()
            .child("Join our community and stay updated:")
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(social(
                        "onboarding-discord",
                        "product/DiscordLogo.svg",
                        18.,
                        "https://anarlog.so/discord",
                    ))
                    .child(social(
                        "onboarding-github",
                        "product/GithubLogo.svg",
                        18.,
                        "https://github.com/fastrepl/anarlog",
                    ))
                    .child(social(
                        "onboarding-x",
                        "product/XLogo.svg",
                        14.,
                        "https://x.com/anarlogapp",
                    )),
            )
            .into_any_element()
    }

    fn finish_section(&self, colors: &Theme, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .items_start()
            .gap_2()
            .child(
                primary_button("onboarding-finish", colors)
                    .py_2()
                    .when(self.state.finishing, |button| {
                        button.opacity(0.7).child(spinner(
                            "onboarding-finish-spinner",
                            16.,
                            colors.primary_foreground,
                        ))
                    })
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.finish(cx)))
                    .child("Open Anarlog"),
            )
            .when(self.state.finish_failed, |column| {
                column.child(
                    text(14., 20.)
                        .text_color(rgb(RED_500))
                        .child("Couldn't open Anarlog. Please try again."),
                )
            })
            .into_any_element()
    }

    /// The video layer and its fades. The frame covers the window from the bottom like
    /// `object-cover object-bottom`; its blur is baked into the image.
    fn backdrop(&self, window: &Window, colors: &Theme) -> Div {
        let size = window.viewport_size();
        let (width, height) = (f32::from(size.width), f32::from(size.height));
        let scale = (width / FRAME.0).max(height / FRAME.1);
        let (frame_width, frame_height) = (FRAME.0 * scale, FRAME.1 * scale);
        let background = colors.background;
        let fade = |angle: f32, from: f32, to: f32| {
            linear_gradient(
                angle,
                linear_color_stop(background.opacity(from), 0.),
                linear_color_stop(background.opacity(to), 1.),
            )
        };
        div()
            .absolute()
            .inset_0()
            .overflow_hidden()
            .child(
                img("product/OnboardingBackground.png")
                    .absolute()
                    .left(px((width - frame_width) / 2.))
                    .top(px(height - frame_height))
                    .w(px(frame_width))
                    .h(px(frame_height))
                    .opacity(0.28),
            )
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .flex_col()
                    .child(div().h(relative(0.5)).bg(fade(0., 0.18, 0.)))
                    .child(div().flex_1().bg(fade(0., 0.08, 0.18))),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(relative(0.84))
                    .flex()
                    .flex_col()
                    .child(div().h(relative(0.42)).bg(fade(180., 1., 0.97)))
                    .child(div().flex_1().bg(fade(180., 0.97, 0.))),
            )
    }
}

impl Render for OnboardingView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let muted = colors.muted_foreground;
        let macos = cfg!(target_os = "macos");
        let description = self.final_description(&colors);
        let sections = [
            self.section(
                Step::Permissions,
                Heading {
                    title: "Start with permissions",
                    completed: "Permissions granted",
                    description: Some(
                        if macos {
                            "Anarlog needs microphone and system audio to transcribe your meetings, plus Accessibility to read meeting controls, visible chat."
                        } else {
                            "Anarlog needs access to your microphone and system audio to record and transcribe your meetings"
                        }
                        .into_any_element(),
                    ),
                    skippable: false,
                },
                &colors,
                cx,
                |this, colors, cx| this.permissions(colors, cx),
            ),
            self.section(
                Step::Login,
                Heading {
                    title: "Create account",
                    completed: login_completed_title(self.state.signed_in, self.state.skipped_login),
                    description: None,
                    skippable: true,
                },
                &colors,
                cx,
                |this, colors, cx| this.login(colors, cx),
            ),
            self.section(
                Step::Calendar,
                Heading {
                    title: "Connect calendar",
                    completed: "Calendar connected",
                    description: Some(
                        "Anarlog will sync your calendar to get meeting reminders".into_any_element(),
                    ),
                    skippable: true,
                },
                &colors,
                cx,
                |this, colors, cx| this.calendar(colors, cx),
            ),
            self.section(
                Step::Imports,
                Heading {
                    title: "Bring your meeting history",
                    completed: "Meeting history imported",
                    description: Some(
                        "Import notes and transcripts from the meeting apps you already use."
                            .into_any_element(),
                    ),
                    skippable: true,
                },
                &colors,
                cx,
                |this, colors, cx| this.imports(colors, cx),
            ),
            self.section(
                Step::Final,
                Heading {
                    title: "Ready to go",
                    completed: "Ready to go",
                    description: Some(description),
                    skippable: false,
                },
                &colors,
                cx,
                |this, colors, cx| this.finish_section(colors, cx),
            ),
        ];
        let backdrop = self.backdrop(window, &colors);
        div()
            .id("onboarding")
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(colors.card)
            .text_color(colors.foreground)
            .font_family(system_font(cx))
            .child(if self.live {
                // Shipping fades the backdrop in and lifts the video into place.
                backdrop
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .bg(colors.background)
                            .with_animation(
                                "onboarding-reveal",
                                Animation::new(Duration::from_millis(1100)).with_easing(|delta| {
                                    ease_out(((delta * 1.1 - 0.1) / 1.).max(0.))
                                }),
                                |cover, delta| cover.opacity(1. - delta),
                            ),
                    )
                    .into_any_element()
            } else {
                backdrop.into_any_element()
            })
            .child(
                div()
                    .relative()
                    .h(px(48.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_end()
                    .pr_3()
                    .pl_12()
                    .child(
                        div()
                            .id("onboarding-mute")
                            .p(px(6.))
                            .rounded(px(8.))
                            .cursor_pointer()
                            .hover(|style| style.bg(colors.accent))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_muted(cx)))
                            .child(icon(
                                if self.state.muted {
                                    "product/VolumeXIcon.svg"
                                } else {
                                    "product/VolumeHighIcon.svg"
                                },
                                16.,
                                muted,
                            )),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex_none()
                    .flex()
                    .items_center()
                    .px_12()
                    .pt_4()
                    .pb_8()
                    .child(
                        text(36., 36.)
                            .font_family(HAND_FONT)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(colors.foreground)
                            .child("Welcome to Anarlog"),
                    ),
            )
            .child(
                div()
                    .id("onboarding-sections")
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .flex()
                    .flex_col()
                    .gap_4()
                    .px_12()
                    .pb_16()
                    .children(sections.into_iter().flatten()),
            )
    }
}

/// Approximates shipping's `[0.22, 1, 0.36, 1]` ease-out curve.
fn ease_out(delta: f32) -> f32 {
    1. - (1. - delta.min(1.)).powi(4)
}
