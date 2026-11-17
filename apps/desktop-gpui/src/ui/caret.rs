use std::time::Duration;

use gpui::{App, AppContext, Context, Entity, FocusHandle, Subscription, Task, Window};

pub struct Caret {
    focus: FocusHandle,
    enabled: bool,
    active: bool,
    visible: bool,
    timer: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl Caret {
    pub fn new<T: 'static>(
        focus: &FocusHandle,
        window: &mut Window,
        cx: &mut Context<T>,
    ) -> Entity<Self> {
        let caret = cx.new(|cx| Self {
            focus: focus.clone(),
            enabled: false,
            active: false,
            visible: false,
            timer: None,
            _subscriptions: vec![
                cx.on_focus(focus, window, Self::sync),
                cx.on_blur(focus, window, Self::sync),
                cx.observe_window_activation(window, Self::sync),
            ],
        });
        cx.observe(&caret, |_, _, cx| cx.notify()).detach();
        caret
    }

    pub fn set_enabled(&mut self, enabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.enabled = enabled;
        self.sync(window, cx);
    }

    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active = self.enabled && window.is_window_active() && self.focus.is_focused(window);
        if self.active != active {
            self.active = active;
            self.timer = None;
            if active {
                self.reset(cx);
            } else {
                self.visible = false;
                cx.notify();
            }
        }
    }

    pub fn reset(&mut self, cx: &mut Context<Self>) {
        if !self.active {
            return;
        }
        self.visible = true;
        self.timer = Some(cx.spawn(async move |entity, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                if entity
                    .update(cx, |this, cx| {
                        this.visible = !this.visible;
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
        cx.notify();
    }

    pub fn visible(&self, window: &Window) -> bool {
        self.visible && window.is_window_active() && self.focus.is_focused(window)
    }

    pub fn reset_entity(caret: &Entity<Self>, cx: &mut App) {
        caret.update(cx, |caret, cx| caret.reset(cx));
    }
}
