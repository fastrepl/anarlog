use gpui::{Pixels, ScrollEvent, ScrollHandle, Window, point, px};

const ANCHOR_TOLERANCE: Pixels = px(1.);

/// Bottom-anchored scroll state matching CSS `overflow-anchor`: follows newly appended
/// content while pinned to the bottom anchor, releases immediately on upward user scroll,
/// and re-pins when scrolling back to the bottom or jumping to recent content.
pub struct BottomAnchor {
    handle: ScrollHandle,
    pinned: bool,
}

impl BottomAnchor {
    pub fn new(pinned: bool) -> Self {
        Self {
            handle: ScrollHandle::new(),
            pinned,
        }
    }

    pub fn handle(&self) -> &ScrollHandle {
        &self.handle
    }

    pub fn is_pinned(&self) -> bool {
        self.pinned
    }

    pub fn distance_from_bottom(&self) -> Pixels {
        let max = self.handle.max_offset().y;
        let offset = self.handle.offset().y.clamp(-max, px(0.));
        (max + offset).max(px(0.))
    }

    pub fn is_at_bottom(&self) -> bool {
        self.distance_from_bottom() <= ANCHOR_TOLERANCE
    }

    pub fn pin(&mut self) {
        self.pinned = true;
    }

    pub fn unpin(&mut self) {
        self.pinned = false;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.pinned = true;
        self.handle.scroll_to_bottom();
    }

    pub fn scroll_to_top(&mut self) {
        self.pinned = false;
        self.handle.set_offset(point(px(0.), px(0.)));
    }

    pub fn scroll_to_item(&mut self, ix: usize) {
        self.pinned = false;
        self.handle.scroll_to_item(ix);
    }

    /// Requests a frame-end scroll-to-bottom while following and pinned so newly grown
    /// content stays anchored in the same layout pass.
    pub fn sync_render(&self, follow: bool) {
        if follow && self.pinned {
            self.handle.scroll_to_bottom();
        }
    }

    pub fn on_scroll(&mut self, event: &ScrollEvent, window: &Window) -> bool {
        let delta_y = event.delta.pixel_delta(window.line_height()).y;
        self.apply_scroll_delta(delta_y)
    }

    pub(crate) fn apply_scroll_delta(&mut self, delta_y: Pixels) -> bool {
        if delta_y == px(0.) || self.handle.max_offset().y <= px(0.) {
            return false;
        }
        // GPUI's `paint_scroll_listener` applies `delta` before element `.on_scroll` listeners
        // run during the bubble phase: positive `delta_y` scrolled up toward `0`, negative
        // `delta_y` scrolled down toward `-max_offset.y`.
        let next = if delta_y > px(0.) {
            false
        } else {
            self.pinned || self.is_at_bottom()
        };
        let changed = self.pinned != next;
        self.pinned = next;
        changed
    }
}

#[cfg(test)]
mod tests {
    use gpui::{TestAppContext, div, point, prelude::*, px};

    use super::*;

    struct TestView {
        anchor: BottomAnchor,
        content_height: f32,
    }

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
            self.anchor.sync_render(true);
            div().w(px(300.)).h(px(200.)).child(
                div()
                    .id("test-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(self.anchor.handle())
                    .on_scroll(cx.listener(|this, event: &ScrollEvent, window, cx| {
                        if this.anchor.on_scroll(event, window) {
                            cx.notify();
                        }
                    }))
                    .child(div().w_full().h(px(self.content_height)))
                    .child(div().id("test-scroll-anchor").h(px(1.)).flex_shrink_0()),
            )
        }
    }

    fn mount(
        content_height: f32,
        cx: &mut TestAppContext,
    ) -> (gpui::Entity<TestView>, &mut gpui::VisualTestContext) {
        let (view, visual) = cx.add_window_view(|_, _| TestView {
            anchor: BottomAnchor::new(true),
            content_height,
        });
        visual.update(|window, cx| window.draw(cx).clear(cx));
        visual.run_until_parked();
        (view, visual)
    }

    #[gpui::test]
    fn upward_scroll_unpins_only_when_overflowing_and_downward_repins_at_bottom(
        cx: &mut TestAppContext,
    ) {
        let (view, cx) = mount(100., cx);
        view.update(cx, |view, _| {
            assert_eq!(view.anchor.handle().max_offset().y, px(0.));
            assert!(!view.anchor.apply_scroll_delta(px(12.)));
            assert!(view.anchor.is_pinned());
            view.content_height = 600.;
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));

        view.read_with(cx, |view, _| {
            assert!(view.anchor.handle().max_offset().y > px(300.));
            assert!(view.anchor.is_at_bottom());
        });

        cx.simulate_event(ScrollEvent {
            position: point(px(50.), px(50.)),
            delta: gpui::ScrollDelta::Pixels(point(px(0.), px(20.))),
            ..Default::default()
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        view.read_with(cx, |view, _| {
            assert!(!view.anchor.is_pinned());
            assert!(!view.anchor.is_at_bottom());
        });

        // Scrolling partway down does not re-pin before reaching the bottom anchor.
        cx.simulate_event(ScrollEvent {
            position: point(px(50.), px(50.)),
            delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-10.))),
            ..Default::default()
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        view.read_with(cx, |view, _| {
            assert!(!view.anchor.is_pinned());
        });

        // Scrolling all the way down to the bottom re-pins.
        cx.simulate_event(ScrollEvent {
            position: point(px(50.), px(50.)),
            delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-20.))),
            ..Default::default()
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        view.read_with(cx, |view, _| {
            assert!(view.anchor.is_pinned());
            assert!(view.anchor.is_at_bottom());
        });

        view.update(cx, |view, _| {
            view.anchor.scroll_to_top();
            assert!(!view.anchor.is_pinned());
            view.anchor.scroll_to_bottom();
            assert!(view.anchor.is_pinned());
        });
    }
}
