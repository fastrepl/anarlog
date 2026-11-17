//! Shipping's live-capture indicators: `DancingSticks` and `Spinner` from `@anlg/ui`, and the
//! macOS floating bar's SwiftUI `DancingBars`.

use std::f32::consts::TAU;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt, ElementId, Hsla, LineCap, PathBuilder, PathStyle, StrokeOptions,
    canvas, div, fill, point, prelude::*, px, size,
};

/// One clock for every indicator; a long synced period keeps phases continuous.
fn clock(
    id: impl Into<ElementId>,
    element: gpui::Div,
    paint: impl Fn(gpui::Div, f32) -> gpui::Div + 'static,
) -> impl IntoElement {
    element.with_animation(
        id,
        Animation::new(Duration::from_secs(3600)).repeat_synced(),
        move |element, delta| paint(element, delta * 3600.),
    )
}

/// JavaScript's `mulberry32`, so every stick keeps shipping's speed and phase.
fn mulberry32(seed: u32) -> impl FnMut() -> f32 {
    let mut value = seed;
    move || {
        value = value.wrapping_add(0x6d2b_79f5);
        let mut t = value;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        ((t ^ (t >> 14)) as f64 / 4_294_967_296.) as f32
    }
}

/// CSS `ease-in-out`: cubic-bezier(0.42, 0, 0.58, 1).
fn ease_in_out(progress: f32) -> f32 {
    let curve = |t: f32, a: f32, b: f32| {
        3. * a * t * (1. - t).powi(2) + 3. * b * t.powi(2) * (1. - t) + t.powi(3)
    };
    let (mut low, mut high) = (0_f32, 1_f32);
    for _ in 0..24 {
        let middle = (low + high) / 2.;
        if curve(middle, 0.42, 0.58) < progress {
            low = middle;
        } else {
            high = middle;
        }
    }
    curve((low + high) / 2., 0., 1.)
}

/// `anarlog-dancing-stick`: scaleY 0.2 → 1 → 0.2, eased within each half.
fn stick_scale(phase: f32) -> f32 {
    let (from, to, progress) = if phase < 0.5 {
        (0.2, 1., phase * 2.)
    } else {
        (1., 0.2, phase * 2. - 1.)
    };
    from + (to - from) * ease_in_out(progress)
}

struct Stick {
    max_scale: f32,
    duration: f32,
    delay: f32,
}

fn sticks(count: usize) -> Vec<Stick> {
    let middle = (count as f32 - 1.) / 2.;
    (0..count)
        .map(|index| {
            let base = if count <= 1 {
                100.
            } else {
                50. + 50. * (1. - (index as f32 - middle).abs() / middle)
            };
            let mut random = mulberry32((index as u32 + 1) * 10007);
            let speed = 4. + random() * 3.;
            let phase = random() * TAU;
            let duration = TAU / speed;
            Stick {
                max_scale: (base / 100.).clamp(0.25, 1.),
                duration,
                delay: -(phase / TAU) * duration,
            }
        })
        .collect()
}

/// Shipping's `DancingSticks`: a flat line while silent, otherwise sticks dancing at a height
/// scaled by the amplitude.
pub fn dancing_sticks(
    id: impl Into<ElementId>,
    amplitude: f32,
    color: Hsla,
    (width, height): (f32, f32),
    (stick_width, gap): (f32, f32),
) -> impl IntoElement {
    let frame = div().flex_shrink_0().w(px(width)).h(px(height));
    if amplitude == 0. {
        return frame
            .flex()
            .items_center()
            .child(div().w(px(width)).h(px(1.)).bg(color))
            .into_any_element();
    }
    let count = (((width + gap) / (stick_width + gap)).floor() as usize).max(1);
    let scale = 0.2 + 0.8 * amplitude.clamp(0., 1.);
    clock(id, frame, move |frame, time| {
        let sticks = sticks(count);
        frame.child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let total = count as f32 * stick_width + (count as f32 - 1.) * gap;
                    let left = bounds.left() + (bounds.size.width - px(total)) / 2.;
                    let middle = bounds.center().y;
                    for (index, stick) in sticks.iter().enumerate() {
                        let phase = ((time - stick.delay) / stick.duration).rem_euclid(1.);
                        let length = height * scale * stick.max_scale * stick_scale(phase);
                        window.paint_quad(fill(
                            gpui::Bounds::new(
                                point(
                                    left + px(index as f32 * (stick_width + gap)),
                                    middle - px(length / 2.),
                                ),
                                size(px(stick_width), px(length)),
                            ),
                            color,
                        ));
                    }
                },
            )
            .size_full(),
        )
    })
    .into_any_element()
}

/// Opacity keyframes of `ios-opacity-spin`.
const SPIN: [(f32, f32); 13] = [
    (0., 1.),
    (1. / 12., 0.9),
    (2. / 12., 0.8),
    (3. / 12., 0.7),
    (4. / 12., 0.6),
    (5. / 12., 0.5),
    (6. / 12., 0.4),
    (7. / 12., 0.35),
    (8. / 12., 0.3),
    (9. / 12., 0.25),
    (10. / 12., 0.2),
    (11. / 12., 0.15),
    (1., 1.),
];

fn spin_opacity(phase: f32) -> f32 {
    SPIN.windows(2)
        .find(|pair| phase <= pair[1].0)
        .map(|pair| {
            let progress = (phase - pair[0].0) / (pair[1].0 - pair[0].0);
            pair[0].1 + (pair[1].1 - pair[0].1) * progress
        })
        .unwrap_or(1.)
}

/// Shipping's iOS-style `Spinner`: twelve segments fading in turn once a second.
pub fn spinner(id: impl Into<ElementId>, diameter: f32, color: Hsla) -> impl IntoElement {
    clock(
        id,
        div().flex_shrink_0().size(px(diameter)),
        move |frame, time| {
            frame.child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let width = diameter * 0.1;
                        let center = bounds.center();
                        for index in 0..12 {
                            let angle = index as f32 * TAU / 12.;
                            let (sin, cos) = angle.sin_cos();
                            let at = |distance: f32| {
                                point(center.x + px(sin * distance), center.y - px(cos * distance))
                            };
                            let mut path = PathBuilder::stroke(px(width)).with_style(
                                PathStyle::Stroke(StrokeOptions {
                                    line_width: width,
                                    line_cap: LineCap::Round,
                                    ..Default::default()
                                }),
                            );
                            path.move_to(at(diameter / 2. - width / 2.));
                            path.line_to(at(diameter / 2. - diameter * 0.26 + width / 2.));
                            let phase = (time - index as f32 / 12.).rem_euclid(1.);
                            if let Ok(path) = path.build() {
                                window.paint_path(path, color.opacity(spin_opacity(phase)));
                            }
                        }
                    },
                )
                .size_full(),
            )
        },
    )
}

/// The macOS floating bar's `DancingBars`: five capsules on a sine wave driven by amplitude.
pub fn dancing_bars(id: impl Into<ElementId>, amplitude: f32, color: Hsla) -> impl IntoElement {
    const COUNT: usize = 5;
    clock(
        id,
        div().flex_shrink_0().w(px(26.)).h(px(20.)),
        move |frame, time| {
            frame.child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let normalized = amplitude.clamp(0., 1.) as f64;
                        let center = (COUNT as f64 - 1.) / 2.;
                        let total = COUNT as f32 * 3. + (COUNT as f32 - 1.) * 2.;
                        let left = bounds.left() + (bounds.size.width - px(total)) / 2.;
                        for index in 0..COUNT {
                            let distance = (index as f64 - center).abs() / center.max(1.);
                            let envelope = 1. - distance * 0.42;
                            let wave = (time as f64 * 8.5 + index as f64 * 0.68).sin() * 0.5 + 0.5;
                            let drive = 0.4 + normalized * 0.9;
                            let height =
                                (20. * drive * envelope * (0.4 + wave * 0.6)).clamp(4., 20.) as f32;
                            window.paint_quad(fill(
                                gpui::Bounds::new(
                                    point(
                                        left + px(index as f32 * 5.),
                                        bounds.center().y - px(height / 2.),
                                    ),
                                    size(px(3.), px(height)),
                                ),
                                color,
                            ));
                        }
                    },
                )
                .size_full(),
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sticks_follow_shipping_pattern_and_seeded_motion() {
        let sticks = sticks(5);
        let scales: Vec<f32> = sticks.iter().map(|stick| stick.max_scale).collect();
        assert_eq!(scales, [0.5, 0.75, 1., 0.75, 0.5]);
        assert!(sticks.iter().all(|stick| {
            let speed = TAU / stick.duration;
            (4. ..7.).contains(&speed) && stick.delay <= 0. && stick.delay > -stick.duration
        }));
        assert!((stick_scale(0.) - 0.2).abs() < 1e-4 && (stick_scale(0.5) - 1.).abs() < 1e-4);
        assert!((spin_opacity(0.5) - 0.4).abs() < 1e-4);
    }
}
