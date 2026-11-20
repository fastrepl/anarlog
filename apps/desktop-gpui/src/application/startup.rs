//! Shipping's `LongLoadGate`: while the library takes long to open, the main window says why
//! (waiting for the app an update replaced, or migrating), and a database from a newer app
//! shows "Anarlog needs an update".

use std::time::Duration;

use desktop_runtime::{RuntimeHandle, RuntimeState, StartupStatus};
use futures::{FutureExt, future::select};
use gpui::{App, Div, Global, MouseButton, SharedString, Window, div, prelude::*, px};

use crate::ui::{a11y::Accessible, theme::theme};

/// Shipping's `LONG_LOAD_SPLASH_DELAY_MS`.
const DELAY: Duration = Duration::from_millis(400);
const NEEDS_UPDATE: &str = "Your data was updated by a newer version of Anarlog, such as Anarlog Nightly, and this version cannot open it yet. Your existing data was left unchanged. Install the latest version of Anarlog, or keep using the newer app until this version catches up.";

#[derive(Default)]
pub struct StartupGate(Option<Notice>);

impl Global for StartupGate {}

#[derive(Clone, Debug, PartialEq)]
enum Notice {
    Loading(SharedString),
    NeedsUpdate,
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
            cx.update(|cx| cx.set_global(StartupGate(notice)));
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

fn notice(state: &RuntimeState, status: &StartupStatus) -> Option<Notice> {
    let detail = match (state, status) {
        (RuntimeState::Closed(Err(error)), _)
            if error
                .to_string()
                .contains("created by a newer version of Anarlog") =>
        {
            return Some(Notice::NeedsUpdate);
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

pub fn gate(window: &Window, cx: &App) -> Option<Div> {
    let notice = cx.try_global::<StartupGate>()?.0.clone()?;
    let colors = theme(window);
    let (title, detail): (Option<&str>, SharedString) = match notice {
        Notice::Loading(detail) => (None, detail),
        Notice::NeedsUpdate => (Some("Anarlog needs an update"), NEEDS_UPDATE.into()),
    };
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
                    .max_w(px(384.))
                    .px_6()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .text_center()
                    .when_some(title, |view, title| {
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
                    ),
            ),
    )
}
