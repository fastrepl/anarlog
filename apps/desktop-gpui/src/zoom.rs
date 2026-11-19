//! UI zoom, like shipping's `shared/zoom.ts`: ⌘+ / ⌘− / ⌘0 step one app-wide factor that every
//! window follows and that persists in the profile.
//!
//! Views keep their literal sizes. [`Zoom::application`] wraps the GPUI platform so each window
//! reports `1 / factor` of its real size and `factor` times its display scale, the way a webview
//! zoom works: layout, text rasterization and hit-testing all scale together. The same adapter
//! turns mouse-wheel notches into the distance shipping's web view scrolls.

mod input;
mod platform;

use std::{
    cell::RefCell,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
};

use gpui::{AnyWindowHandle, App, Application, Global, Pixels, Size, Window, px, size};

pub use input::ZoomedInput;

pub const STEPS: [f32; 13] = [
    0.5, 0.67, 0.75, 0.8, 0.9, 1., 1.1, 1.25, 1.5, 1.75, 2., 2.5, 3.,
];
pub const DEFAULT: f32 = 1.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    In,
    Out,
    Reset,
}

/// Shipping's `stepZoomFactor`: the next step past `current`, held at either end.
pub fn step(current: f32, step: Step) -> f32 {
    match step {
        Step::Reset => DEFAULT,
        Step::In => STEPS
            .into_iter()
            .find(|step| *step > current + 1e-6)
            .unwrap_or(STEPS[STEPS.len() - 1]),
        Step::Out => STEPS
            .into_iter()
            .rev()
            .find(|step| *step < current - 1e-6)
            .unwrap_or(STEPS[0]),
    }
}

/// A saved factor; anything missing or not a positive number reads as the default.
pub fn parse(raw: Option<&str>) -> f32 {
    raw.and_then(|raw| raw.trim().parse::<f32>().ok())
        .filter(|factor| factor.is_finite() && *factor > 0.)
        .map_or(DEFAULT, |factor| {
            factor.clamp(STEPS[0], STEPS[STEPS.len() - 1])
        })
}

/// Shipping's `scaleWindowMinSize`: the minimum grows with the zoom but never past the work area.
pub fn min_size(base: Size<Pixels>, factor: f32, work_area: Option<Size<Pixels>>) -> Size<Pixels> {
    let scaled = base.map(|length| px((f32::from(length) * factor).round()));
    match work_area {
        Some(area) => size(
            scaled.width.min(px(f32::from(area.width).floor())),
            scaled.height.min(px(f32::from(area.height).floor())),
        ),
        None => scaled,
    }
}

/// The current factor, shared with the platform adapter (and AccessKit's thread).
#[derive(Clone)]
struct Factor(Arc<AtomicU32>);

impl Factor {
    fn new(factor: f32) -> Self {
        Self(Arc::new(AtomicU32::new(factor.to_bits())))
    }

    fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }

    fn set(&self, factor: f32) {
        self.0.store(factor.to_bits(), Ordering::Relaxed);
    }
}

/// Unzoomed minimum sizes of the windows that have one, recorded as they open.
type MinSizes = Rc<RefCell<Vec<(AnyWindowHandle, Size<Pixels>)>>>;

pub struct Zoom {
    factor: Factor,
    min_sizes: MinSizes,
    path: PathBuf,
    write: Arc<Mutex<()>>,
}

impl Global for Zoom {}

impl Zoom {
    /// Reads the factor saved at `path`, shipping's `anarlog-zoom-factor` local storage item.
    pub fn load(path: PathBuf) -> Self {
        let saved = std::fs::read_to_string(&path).ok();
        Self {
            factor: Factor::new(parse(saved.as_deref())),
            min_sizes: MinSizes::default(),
            path,
            write: Arc::default(),
        }
    }

    /// The native application with every window scaled by this zoom. Install `self` as a global
    /// once it runs. Windows has no zoom yet, so its windows stay at 100% and the adapter only
    /// converts wheel notches there.
    pub fn application(&self) -> Application {
        let factor = if cfg!(target_os = "windows") {
            Factor::new(DEFAULT)
        } else {
            self.factor.clone()
        };
        Application::with_platform(Rc::new(platform::ZoomPlatform::new(
            gpui_platform::current_platform(false),
            factor,
            self.min_sizes.clone(),
        )))
    }
}

pub fn factor(cx: &App) -> f32 {
    cx.try_global::<Zoom>()
        .map_or(DEFAULT, |zoom| zoom.factor.get())
}

pub fn apply(direction: Step, cx: &mut App) {
    set(step(factor(cx), direction), cx);
}

/// Sets the factor for every window and saves it.
pub fn set(factor: f32, cx: &mut App) {
    // Without an adapter a new factor would only resize the content-sized windows.
    if cfg!(target_os = "windows") {
        return;
    }
    let Some(zoom) = cx.try_global::<Zoom>() else {
        return;
    };
    if zoom.factor.get() == factor {
        return;
    }
    zoom.factor.set(factor);
    let (path, saved, write) = (zoom.path.clone(), zoom.factor.clone(), zoom.write.clone());
    let min_sizes = zoom.min_sizes.clone();
    cx.background_executor()
        .spawn(async move {
            // Writes run in order of the lock, and each saves the latest factor.
            let _write = write
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Err(error) = std::fs::write(&path, saved.get().to_string()) {
                tracing::warn!(%error, "failed to save the zoom factor");
            }
        })
        .detach();
    // Keys and menu items run inside the key window's update, which `update` cannot re-enter.
    cx.defer(move |cx| {
        let factor = self::factor(cx);
        let windows = cx.windows();
        min_sizes
            .borrow_mut()
            .retain(|(handle, _)| windows.contains(handle));
        for handle in windows {
            let base = min_sizes
                .borrow()
                .iter()
                .find_map(|(window, base)| (*window == handle).then_some(*base));
            let _ = handle.update(cx, |_, window, cx| {
                // Re-reads the adapter's size and scale, then lays out and draws again.
                window.bounds_changed(cx);
                if let Some(base) = base {
                    fit_min_size(window, base, factor, cx);
                }
            });
        }
    });
}

/// Shipping's `syncWindowMinSize`: rescale the minimum and grow a window that is now below it.
fn fit_min_size(window: &mut Window, base: Size<Pixels>, factor: f32, cx: &App) {
    let area = window
        .display(cx)
        .map(|display| display.visible_bounds().size);
    let minimum = min_size(base, factor, area);
    if let Err(error) = crate::platform::windows::set_min_size(window, minimum) {
        tracing::debug!(%error, "window minimum size is unchanged");
    }
    // The viewport is in zoomed points.
    let needed = minimum.map(|length| length / factor);
    let viewport = window.viewport_size();
    if viewport.width < needed.width || viewport.height < needed.height {
        window.resize(size(
            viewport.width.max(needed.width),
            viewport.height.max(needed.height),
        ));
    }
}

/// A content-sized window's real size, such as the floating and dictation bars, so it opens
/// already fitting its zoomed content.
pub fn window_size(content: Size<Pixels>, cx: &App) -> Size<Pixels> {
    let factor = factor(cx);
    content.map(|length| length * factor)
}

/// The left gutter that clears the macOS traffic lights. They stay in window points while the
/// content zooms, so shipping divides the 76px gutter by the factor.
pub fn window_controls_gutter(cx: &App) -> Pixels {
    if cfg!(target_os = "macos") {
        px(76. / factor(cx))
    } else {
        px(8.)
    }
}

/// Shipping's sidebar minimum on macOS, which keeps its chrome row beside the traffic lights.
pub fn sidebar_min_width(cx: &App) -> Pixels {
    if cfg!(target_os = "macos") {
        (window_controls_gutter(cx) + px(124.)).max(px(200.))
    } else {
        px(200.)
    }
}

/// Top padding that centers a 28px control row on the traffic lights' 23px baseline.
pub fn window_controls_top(cx: &App) -> Pixels {
    px((23. / factor(cx) - 14.).max(0.))
}

#[cfg(test)]
mod tests {
    use gpui::{Context, IntoElement, Render, TestAppContext, div};

    use super::*;

    #[test]
    fn steps_through_shippings_factors_and_holds_at_the_ends() {
        for (from, direction, expected) in [
            (1., Step::In, 1.1),
            (1., Step::Out, 0.9),
            (3., Step::In, 3.),
            (0.5, Step::Out, 0.5),
            (2., Step::Reset, 1.),
            (1.05, Step::In, 1.1),
            (1.05, Step::Out, 1.),
        ] {
            assert_eq!(step(from, direction), expected, "{from} {direction:?}");
        }
        let mut factor = 1.;
        for _ in 0..20 {
            factor = step(factor, Step::In);
        }
        assert_eq!(factor, 3.);
        for _ in 0..20 {
            factor = step(factor, Step::Out);
        }
        assert_eq!(factor, 0.5);
    }

    #[test]
    fn reads_only_positive_saved_factors_within_the_steps() {
        for (saved, expected) in [
            (None, 1.),
            (Some("1.25"), 1.25),
            (Some("bogus"), 1.),
            (Some("-2"), 1.),
            (Some("NaN"), 1.),
            (Some("7"), 3.),
        ] {
            assert_eq!(parse(saved), expected, "{saved:?}");
        }
    }

    #[test]
    fn minimum_size_scales_but_stays_within_the_work_area() {
        let base = size(px(500.), px(500.));
        assert_eq!(min_size(base, 1.5, None), size(px(750.), px(750.)));
        assert_eq!(
            min_size(base, 3., Some(size(px(1440.), px(900.5)))),
            size(px(1440.), px(900.))
        );
    }

    struct Probe {
        bounds_changes: usize,
    }

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    fn probe(cx: &mut TestAppContext) -> gpui::WindowHandle<Probe> {
        cx.add_window(|window, cx| {
            cx.observe_window_bounds(window, |probe: &mut Probe, _, _| probe.bounds_changes += 1)
                .detach();
            Probe { bounds_changes: 0 }
        })
    }

    #[gpui::test]
    fn every_window_follows_a_change_and_the_next_launch_restores_it(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("zoom.json");
        cx.update(|cx| cx.set_global(Zoom::load(path.clone())));
        let (main, note) = (probe(cx), probe(cx));

        cx.update(|cx| {
            apply(Step::In, cx);
            apply(Step::In, cx);
        });
        cx.run_until_parked();

        assert_eq!(cx.update(|cx| factor(cx)), 1.25);
        for window in [main, note] {
            let changes = window
                .update(cx, |probe, _, _| probe.bounds_changes)
                .unwrap();
            assert_eq!(changes, 2, "each step re-reads every window's geometry");
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "1.25");
        assert_eq!(Zoom::load(path.clone()).factor.get(), 1.25);

        cx.update(|cx| apply(Step::In, cx));
        cx.update(|cx| apply(Step::Reset, cx));
        cx.run_until_parked();
        assert_eq!(Zoom::load(path).factor.get(), 1.);
        let changes = main.update(cx, |probe, _, _| probe.bounds_changes).unwrap();
        assert_eq!(changes, 4);
    }

    #[gpui::test]
    fn holding_the_last_step_does_not_redraw(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("zoom.json");
        std::fs::write(&path, "3").unwrap();
        cx.update(|cx| cx.set_global(Zoom::load(path)));
        let window = probe(cx);
        cx.update(|cx| apply(Step::In, cx));
        cx.run_until_parked();
        assert_eq!(cx.update(|cx| factor(cx)), 3.);
        let changes = window
            .update(cx, |probe, _, _| probe.bounds_changes)
            .unwrap();
        assert_eq!(changes, 0);
    }
}
