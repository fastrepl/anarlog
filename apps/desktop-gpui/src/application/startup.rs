//! Shipping's `LongLoadGate`: while the library takes long to open, the main window says why
//! (waiting for the app an update replaced, or migrating), and a database from a newer app
//! shows "Anarlog needs an update". Notes shipping has yet to move from an older storage folder
//! keep the library closed, and the window says where they are.

use std::time::Duration;

use anlg_desktop_identity::vault::UnfinishedMove;
use desktop_runtime::{RuntimeHandle, RuntimeState, StartupStatus};
use futures::{FutureExt, future::select};
use gpui::{
    App, ClipboardItem, Div, Global, MouseButton, SharedString, Window, div, prelude::*, px,
};

use crate::{
    platform::package,
    ui::{
        a11y::Accessible,
        theme::{MEDIUM, Theme, UiFonts, monospace_font, theme},
    },
};

/// Shipping's `LONG_LOAD_SPLASH_DELAY_MS`.
const DELAY: Duration = Duration::from_millis(400);
const NEEDS_UPDATE: &str = "Your data was updated by a newer version of Anarlog, such as Anarlog Nightly, and this version cannot open it yet. Your existing data was left unchanged. Install the latest version of Anarlog, or keep using the newer app until this version catches up.";
const SUPPORT_EMAIL: &str = "team@fastrepl.com";
const SUPPORT_DISCORD_URL: &str = "https://anarlog.so/discord";

#[derive(Clone, Default)]
pub struct StartupGate {
    notice: Option<Notice>,
    copied: bool,
}

impl Global for StartupGate {}

#[derive(Clone, Debug, PartialEq)]
enum Notice {
    Loading(SharedString),
    NeedsUpdate(SharedString),
    UnfinishedMove(UnfinishedMove),
}

struct Refusal {
    title: &'static str,
    detail: SharedString,
    paths: Option<(String, String)>,
    email_url: &'static str,
    copy_text: String,
}

/// Follows the runtime's startup; observe [`StartupGate`] to redraw.
pub fn install(runtime: &RuntimeHandle, cx: &mut App) {
    cx.set_global(StartupGate::default());
    let mut state = runtime.state();
    let mut status = runtime.startup();
    cx.spawn(async move |cx| {
        cx.background_executor().timer(DELAY).await;
        loop {
            let (notice, starting) = {
                let state = state.borrow_and_update();
                (
                    notice(&state, &status.borrow_and_update()),
                    matches!(*state, RuntimeState::Starting),
                )
            };
            cx.update(|cx| {
                let copied = cx
                    .try_global::<StartupGate>()
                    .is_some_and(|gate| gate.notice == notice && gate.copied);
                cx.set_global(StartupGate { notice, copied });
            });
            if !starting {
                return;
            }
            let (changed, _) = select(state.changed().boxed(), status.changed().boxed())
                .await
                .factor_first();
            if changed.is_err() {
                return;
            }
        }
    })
    .detach();
}

fn mark_copied(cx: &mut App) {
    if let Some(gate) = cx.try_global::<StartupGate>().cloned() {
        cx.set_global(StartupGate {
            copied: true,
            ..gate
        });
    }
}

fn notice(state: &RuntimeState, status: &StartupStatus) -> Option<Notice> {
    let detail = match (state, status) {
        (RuntimeState::Closed(Err(error)), _)
            if error
                .to_string()
                .contains("created by a newer version of Anarlog") =>
        {
            return Some(Notice::NeedsUpdate(error.to_string().into()));
        }
        (RuntimeState::Closed(Err(_)), StartupStatus::UnfinishedMove(pending)) => {
            return Some(Notice::UnfinishedMove(pending.clone()));
        }
        (RuntimeState::Starting, StartupStatus::Opening) => {
            "Checking your local database. This is taking longer than expected.".into()
        }
        (RuntimeState::Starting, StartupStatus::WaitingForLock) => {
            "Another Anarlog process is preparing your data, possibly finishing an update. The app will open automatically when it is ready.".into()
        }
        (RuntimeState::Starting, StartupStatus::Migrating { current, total }) => format!(
            "Migrating your local database ({current} of {total}). This may take a few minutes."
        )
        .into(),
        _ => return None,
    };
    Some(Notice::Loading(detail))
}

fn refusal_for(notice: &Notice) -> Option<Refusal> {
    let version = format!(
        "{} ({})",
        package::VERSION,
        package::tray_channel(package::channel())
    );
    let os = format!("{} ({})", std::env::consts::OS, std::env::consts::ARCH);
    match notice {
        Notice::Loading(_) => None,
        Notice::NeedsUpdate(error) => Some(Refusal {
            title: "Anarlog needs an update",
            detail: NEEDS_UPDATE.into(),
            paths: None,
            email_url: "mailto:team@fastrepl.com?subject=Anarlog%20update%20help",
            copy_text: format!(
                "Issue: Database requires a newer version of Anarlog\nVersion: {version}\nOS: {os}\nError: {error}"
            ),
        }),
        Notice::UnfinishedMove(pending) => {
            let from = pending.from.display().to_string();
            let to = pending.to.display().to_string();
            Some(Refusal {
                title: "Anarlog can't open your notes yet",
                detail: format!(
                    "Some notes and recordings are still in {from}, a storage folder from an earlier version of Anarlog, and haven't finished moving into {to}. This version doesn't move files, so both folders were left unchanged. Contact support for help finishing the move."
                )
                .into(),
                copy_text: format!(
                    "Issue: Unfinished storage move\nVersion: {version}\nOS: {os}\nPrevious folder: {from}\nCurrent folder: {to}"
                ),
                paths: Some((from, to)),
                email_url: "mailto:team@fastrepl.com?subject=Anarlog%20storage%20move%20help",
            })
        }
    }
}

/// Startup refused the library and the window says why. Shipping exits then, so closing the
/// window quits: a hidden process would answer the next launch without checking again.
pub fn refused(cx: &App) -> bool {
    cx.try_global::<StartupGate>()
        .and_then(|gate| gate.notice.as_ref())
        .is_some_and(|notice| !matches!(notice, Notice::Loading(_)))
}

fn path_row(label: &'static str, value: String, mono: Option<SharedString>, colors: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_0p5()
        .child(
            div()
                .text_xs()
                .font_weight(MEDIUM)
                .text_color(colors.muted_foreground)
                .child(label),
        )
        .child(
            div()
                .text_xs()
                .text_color(colors.foreground)
                .when_some(mono, |view, font| view.font_family(font))
                .child(value),
        )
}

fn support_link(
    id: &'static str,
    label: &'static str,
    text: &'static str,
    url: &'static str,
    colors: &Theme,
) -> gpui::Stateful<Div> {
    div()
        .id(id)
        .a11y_link(label)
        .underline()
        .cursor_pointer()
        .text_color(colors.foreground)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, _, cx| cx.open_url(url))
        .child(text)
}

pub fn gate(window: &Window, cx: &App) -> Option<Div> {
    let gate = cx.try_global::<StartupGate>()?;
    let notice = gate.notice.as_ref()?;
    let colors = theme(window);
    let mono = cx.try_global::<UiFonts>().map(|_| monospace_font(cx));
    let refusal = refusal_for(notice);
    let detail: SharedString = match (&refusal, notice) {
        (Some(refusal), _) => refusal.detail.clone(),
        (None, Notice::Loading(detail)) => detail.clone(),
        (None, _) => unreachable!(),
    };
    let copied = gate.copied;
    Some(
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .bg(colors.background)
            .text_color(colors.foreground)
            .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
            .child(
                div()
                    .max_w(px(460.))
                    .px_6()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_3()
                    .text_center()
                    .when_some(refusal.as_ref().map(|r| r.title), |view, title| {
                        view.child(
                            div()
                                .id("startup-title")
                                .a11y_heading(title, 1)
                                .text_size(px(16.))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(title),
                        )
                    })
                    .child(
                        div()
                            .id("startup-detail")
                            .a11y_text(detail.clone())
                            .text_sm()
                            .line_height(px(20.))
                            .text_color(colors.muted_foreground)
                            .child(detail),
                    )
                    .when_some(refusal, |view, refusal| {
                        let copy_text = refusal.copy_text;
                        let copy_label = if copied {
                            "Copied details"
                        } else {
                            "Copy details"
                        };
                        view.when_some(refusal.paths, |view, (from, to)| {
                            view.child(
                                div()
                                    .w_full()
                                    .border_1()
                                    .border_color(colors.border)
                                    .bg(colors.muted)
                                    .px_3()
                                    .py_2()
                                    .text_left()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(path_row("Previous folder", from, mono.clone(), &colors))
                                    .child(path_row("Current folder", to, mono.clone(), &colors)),
                            )
                        })
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .items_center()
                                .justify_center()
                                .gap_x_3()
                                .gap_y_1()
                                .text_xs()
                                .text_color(colors.muted_foreground)
                                .child("Support:")
                                .child(support_link(
                                    "startup-support-email",
                                    "Email team@fastrepl.com",
                                    SUPPORT_EMAIL,
                                    refusal.email_url,
                                    &colors,
                                ))
                                .child(support_link(
                                    "startup-support-discord",
                                    "Discord (anarlog.so/discord)",
                                    SUPPORT_DISCORD_URL,
                                    SUPPORT_DISCORD_URL,
                                    &colors,
                                )),
                        )
                        .child(
                            div()
                                .mt_2()
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap_2()
                                .child(
                                    div()
                                        .id("startup-copy-details")
                                        .a11y_button(copy_label)
                                        .h(px(36.))
                                        .px_4()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .border_1()
                                        .border_color(colors.border)
                                        .bg(colors.secondary)
                                        .text_color(colors.secondary_foreground)
                                        .text_sm()
                                        .font_weight(MEDIUM)
                                        .cursor_pointer()
                                        .hover(|style| style.bg(colors.accent))
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation()
                                        })
                                        .on_click(move |_, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                copy_text.clone(),
                                            ));
                                            mark_copied(cx);
                                        })
                                        .child(copy_label),
                                )
                                .child(
                                    div()
                                        .id("startup-quit")
                                        .a11y_button("Quit")
                                        .h(px(36.))
                                        .px_4()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .bg(colors.primary)
                                        .text_color(colors.primary_foreground)
                                        .shadow_xs()
                                        .text_sm()
                                        .font_weight(MEDIUM)
                                        .cursor_pointer()
                                        .hover(|style| style.bg(colors.primary.opacity(0.9)))
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation()
                                        })
                                        .on_click(|_, _, cx| cx.quit())
                                        .child("Quit"),
                                ),
                        )
                        .when(copied, |view| {
                            view.child(
                                div()
                                    .id("startup-copy-status")
                                    .a11y_status("Details copied to clipboard", false)
                                    .text_xs()
                                    .text_color(colors.muted_foreground)
                                    .child("Details copied to clipboard"),
                            )
                        })
                    }),
            ),
    )
}
