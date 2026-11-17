//! Shipping's ⌘Q interceptor (`crates/intercept`): the first ⌘Q shows "Press ⌘Q again to Quit"
//! and only a second press quits. macOS only, like shipping.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt, App, AppContext, Bounds, Context, IntoElement, Render, Window,
    WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind, WindowOptions, div, hsla,
    point, prelude::*, px, size,
};

use crate::ui::theme::{MEDIUM, system_font};

pub const QUIT_OVERLAY_DURATION: Duration = Duration::from_millis(1500);
const FADE: Duration = Duration::from_millis(70);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Press {
    #[default]
    Idle,
    First,
    Awaiting,
}

/// Shipping's `QuitInterceptor` state machine.
#[derive(Debug, Default)]
pub struct QuitKeys {
    press: Press,
    generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuitSignal {
    Show,
    Hide,
    Quit,
    /// Q or Command was released; the overlay times out unless a newer press happened.
    Released(u64),
}

impl QuitKeys {
    /// A ⌘Q key-down. The caller swallows every one, like shipping.
    pub fn press(&mut self, shift: bool, repeat: bool) -> Option<QuitSignal> {
        if shift {
            self.press = Press::Idle;
            return Some(QuitSignal::Quit);
        }
        if repeat {
            return None;
        }
        match self.press {
            Press::Idle => {
                self.press = Press::First;
                Some(QuitSignal::Show)
            }
            // The swallowed first press may never deliver its key-up, so a second press
            // while Command is still held confirms too.
            Press::First | Press::Awaiting => {
                self.press = Press::Idle;
                Some(QuitSignal::Quit)
            }
        }
    }

    pub fn release(&mut self) -> Option<u64> {
        (self.press == Press::First).then(|| {
            self.press = Press::Awaiting;
            self.generation += 1;
            self.generation
        })
    }

    pub fn timeout(&mut self, generation: u64) -> bool {
        let expired = self.press == Press::Awaiting && self.generation == generation;
        if expired {
            self.press = Press::Idle;
        }
        expired
    }
}

/// "Press ⌘Q again to Quit" (`QuitOverlayConfig.swift`): 360×84, 15% above screen center.
#[derive(Default)]
pub struct QuitOverlay(Option<WindowHandle<QuitOverlayView>>);

pub struct QuitOverlayView {
    closing: bool,
    shown: u64,
}

impl QuitOverlay {
    pub fn show(&mut self, cx: &mut App) {
        if let Some(window) = &self.0
            && window
                .update(cx, |view, _, cx| {
                    view.shown += 1;
                    view.closing = false;
                    cx.notify();
                })
                .is_ok()
        {
            return;
        }
        let Some(display) = cx.primary_display() else {
            return;
        };
        let screen = display.bounds();
        let size = crate::zoom::window_size(size(px(360.), px(84.)), cx);
        let origin = point(
            screen.center().x - size.width / 2.,
            screen.center().y - size.height / 2. - screen.size.height * 0.15,
        );
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(origin, size))),
            titlebar: None,
            focus: false,
            show: true,
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            window_background: WindowBackgroundAppearance::Transparent,
            app_id: Some("com.anarlog.gpui.sandbox".into()),
            ..Default::default()
        };
        self.0 = cx
            .open_window(options, |_, cx| {
                cx.new(|_| QuitOverlayView {
                    closing: false,
                    shown: 0,
                })
            })
            .ok();
    }

    pub fn hide(&mut self, cx: &mut App) {
        let Some(window) = self.0.take() else {
            return;
        };
        let shown = window
            .update(cx, |view, _, cx| {
                view.closing = true;
                cx.notify();
                view.shown
            })
            .ok();
        cx.spawn(async move |cx| {
            cx.background_executor().timer(FADE).await;
            let _ = window.update(cx, |view, window, _| {
                if view.closing && Some(view.shown) == shown {
                    window.remove_window();
                }
            });
        })
        .detach();
    }
}

impl Render for QuitOverlayView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let closing = self.closing;
        div()
            .size_full()
            .bg(hsla(0., 0., 0.12, 0.88))
            .flex()
            .items_center()
            .justify_center()
            .font_family(system_font(cx))
            .text_size(px(22.))
            .font_weight(MEDIUM)
            .text_color(gpui::white())
            .child("Press ⌘Q again to Quit")
            .with_animation(
                ("quit-overlay", self.shown * 2 + u64::from(closing)),
                Animation::new(FADE),
                move |view, delta| view.opacity(if closing { 1. - delta } else { delta }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_q_needs_a_second_press_and_only_a_stale_overlay_times_out() {
        let mut keys = QuitKeys::default();
        assert_eq!(keys.press(false, false), Some(QuitSignal::Show));
        assert_eq!(keys.press(false, true), None, "held key repeats never quit");
        let first = keys.release().unwrap();
        assert_eq!(keys.release(), None);
        assert_eq!(keys.press(false, false), Some(QuitSignal::Quit));
        assert!(!keys.timeout(first), "a quit cancels the dismissal");

        assert_eq!(keys.press(false, false), Some(QuitSignal::Show));
        let second = keys.release().unwrap();
        assert!(!keys.timeout(first));
        assert!(keys.timeout(second));
        assert_eq!(keys.press(false, false), Some(QuitSignal::Show));
        assert_eq!(
            keys.press(true, false),
            Some(QuitSignal::Quit),
            "⌘⇧Q quits at once"
        );
    }
}
