//! Contact avatars: shipping's generated `Avatar` from `@anlg/ui/lib/avatar.ts` (a dithered
//! gradient with initials), uploaded photos (`ContactImage`) and the organization placeholder.

use std::{cell::RefCell, collections::HashMap, f64::consts::PI, sync::Arc};

use base64::Engine;
use gpui::{
    Hsla, Image, ImageFormat, ObjectFit, RenderImage, SharedString, div, img, prelude::*, px, rgba,
    svg,
};

use crate::ui::theme::Theme;

const RASTER: usize = 64;
const BAYER: [f64; 16] = [
    0., 8., 2., 10., 12., 4., 14., 6., 3., 11., 1., 9., 15., 7., 13., 5.,
];

type Rgb = [f64; 3];

fn hash(value: &str) -> u32 {
    value.chars().fold(2_166_136_261u32, |hash, character| {
        (hash ^ character as u32).wrapping_mul(16_777_619)
    })
}

/// `mulberry32`.
struct Random(u32);

impl Random {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let mut value = self.0;
        value = (value ^ (value >> 15)).wrapping_mul(value | 1);
        value ^= value.wrapping_add((value ^ (value >> 7)).wrapping_mul(value | 61));
        f64::from(value ^ (value >> 14)) / 4_294_967_296.
    }
}

fn hsl(hue: f64, saturation: f64, lightness: f64) -> Rgb {
    let (s, l) = (saturation / 100., lightness / 100.);
    let chroma = (1. - (2. * l - 1.).abs()) * s;
    let segment = hue.rem_euclid(360.) / 60.;
    let secondary = chroma * (1. - ((segment % 2.) - 1.).abs());
    let [r, g, b] = match segment {
        s if s < 1. => [chroma, secondary, 0.],
        s if s < 2. => [secondary, chroma, 0.],
        s if s < 3. => [0., chroma, secondary],
        s if s < 4. => [0., secondary, chroma],
        s if s < 5. => [secondary, 0., chroma],
        _ => [chroma, 0., secondary],
    };
    let offset = l - chroma / 2.;
    [
        (r + offset) * 255.,
        (g + offset) * 255.,
        (b + offset) * 255.,
    ]
}

fn palette(random: &mut Random, count: usize) -> Vec<Rgb> {
    let base = random.next() * 360.;
    let spread = [32., 52., 138., 208.][((random.next() * 4.) as usize).min(3)];
    (0..count)
        .map(|index| {
            let hue = (base + spread * index as f64 + (random.next() - 0.5) * 18.) % 360.;
            let saturation = 58. + random.next() * 24.;
            let lightness = 48. + random.next() * 24.;
            hsl(hue, saturation, lightness)
        })
        .collect()
}

fn quantize(value: f64) -> u8 {
    let steps = 6.;
    // Uint8ClampedArray rounds half to even.
    ((value.clamp(0., 255.) / 255. * steps).round() * (255. / steps))
        .clamp(0., 255.)
        .round_ties_even() as u8
}

/// `createAvatarPixels` with the `Avatar` defaults: four colours, four spheres, 0.3 dither.
pub fn pixels(seed: &str) -> Vec<[u8; 3]> {
    let mut random = Random(hash(seed));
    let colors = palette(&mut random, 4);
    let angle = random.next() * PI * 2.;
    let spheres = (0..4)
        .map(|index| {
            let x = -0.1 + random.next() * 1.2;
            let y = -0.1 + random.next() * 1.2;
            let radius = 0.24 + random.next() * 0.42;
            (x, y, radius, colors[(index + 1) % colors.len()])
        })
        .collect::<Vec<_>>();
    let mut pixels = Vec::with_capacity(RASTER * RASTER);
    for y in 0..RASTER {
        for x in 0..RASTER {
            let nx = (x as f64 + 0.5) / RASTER as f64;
            let ny = (y as f64 + 0.5) / RASTER as f64;
            let position =
                (0.5 + (nx - 0.5) * angle.cos() + (ny - 0.5) * angle.sin()).clamp(0., 1.);
            let scaled = position * (colors.len() - 1) as f64;
            let left = scaled.floor() as usize;
            let right = (left + 1).min(colors.len() - 1);
            let amount = scaled - left as f64;
            let mut color: Rgb = std::array::from_fn(|c| {
                colors[left][c] + (colors[right][c] - colors[left][c]) * amount
            });
            for (sx, sy, radius, sphere) in &spheres {
                let distance = (nx - sx).powi(2) + (ny - sy).powi(2);
                let influence = (-distance / (2. * radius * radius)).exp() * 0.72;
                for channel in 0..3 {
                    color[channel] += (sphere[channel] - color[channel]) * influence;
                }
            }
            let threshold = (BAYER[(y % 4) * 4 + x % 4] / 16. - 0.5) * 255.;
            pixels.push(std::array::from_fn(|c| {
                quantize(color[c] + threshold * 0.3)
            }));
        }
    }
    pixels
}

/// `avatarInitials`.
pub fn initials(label: &str) -> String {
    label
        .split_whitespace()
        .filter_map(|part| part.chars().find(|character| character.is_alphanumeric()))
        .take(2)
        .flat_map(char::to_uppercase)
        .collect()
}

struct Generated {
    image: Arc<RenderImage>,
    /// White initials under `mix-blend-mode: overlay`, resolved against the avatar's centre.
    initials: Hsla,
}

thread_local! {
    static GENERATED: RefCell<HashMap<String, Arc<Generated>>> = RefCell::default();
}

fn generated(seed: &str) -> Arc<Generated> {
    GENERATED.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(found) = cache.get(seed) {
            return found.clone();
        }
        let pixels = pixels(seed);
        let mut bgra = image::RgbaImage::new(RASTER as u32, RASTER as u32);
        for (index, [r, g, b]) in pixels.iter().enumerate() {
            bgra.put_pixel(
                (index % RASTER) as u32,
                (index / RASTER) as u32,
                image::Rgba([*b, *g, *r, 255]),
            );
        }
        let centre = pixels
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                (16..48).contains(&(index % RASTER)) && (16..48).contains(&(index / RASTER))
            })
            .fold([0f32; 3], |sum, (_, pixel)| {
                std::array::from_fn(|c| sum[c] + pixel[c] as f32)
            });
        let count = 32. * 32. * 255.;
        let overlay = centre.map(|sum| {
            let base = sum / count;
            if base < 0.5 { 2. * base } else { 1. }
        });
        let generated = Arc::new(Generated {
            image: Arc::new(RenderImage::new(vec![image::Frame::new(bgra)])),
            initials: gpui::Rgba {
                r: overlay[0],
                g: overlay[1],
                b: overlay[2],
                a: 1.,
            }
            .into(),
        });
        if cache.len() >= 512 {
            cache.clear();
        }
        cache.insert(seed.to_owned(), generated.clone());
        generated
    })
}

fn photo(data_url: &str) -> Option<Arc<Image>> {
    thread_local! {
        static PHOTOS: RefCell<HashMap<SharedString, Option<Arc<Image>>>> = RefCell::default();
    }
    PHOTOS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(found) = cache.get(data_url) {
            return found.clone();
        }
        let decoded = data_url
            .strip_prefix("data:image/")
            .and_then(|rest| rest.split_once(";base64,"))
            .and_then(|(format, data)| {
                let format = match format {
                    "png" => ImageFormat::Png,
                    "jpeg" | "jpg" => ImageFormat::Jpeg,
                    "webp" => ImageFormat::Webp,
                    "gif" => ImageFormat::Gif,
                    _ => return None,
                };
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .ok()?;
                Some(Arc::new(Image::from_bytes(format, bytes)))
            });
        if cache.len() >= 64 {
            cache.clear();
        }
        cache.insert(SharedString::new(data_url), decoded.clone());
        decoded
    })
}

/// `ContactFacehash` / `Avatar`.
pub fn facehash(name: &str, size: f32) -> gpui::Div {
    let generated = generated(name);
    let initials = initials(name);
    div()
        .size(px(size))
        .flex_shrink_0()
        .relative()
        .shadow(vec![gpui::BoxShadow {
            color: rgba(0x00000014).into(),
            offset: gpui::point(px(0.), px(1.)),
            blur_radius: px(2.),
            spread_radius: px(0.),
            inset: false,
        }])
        .child(
            img(generated.image.clone())
                .absolute()
                .inset_0()
                .size_full()
                .object_fit(ObjectFit::Cover),
        )
        .when(!initials.is_empty(), |view| {
            view.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px((size * 0.38).max(7.)))
                    .line_height(px((size * 0.38).max(7.)))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(generated.initials)
                    .child(initials),
            )
        })
        .child(
            div()
                .absolute()
                .inset_0()
                .border_1()
                .border_color(rgba(0x0000001a)),
        )
}

/// `ContactImage`: an uploaded photo, falling back to the generated avatar when it cannot be
/// decoded.
pub fn contact_image(data_url: &str, fallback: &str, size: f32) -> gpui::Div {
    let Some(image) = photo(data_url) else {
        return facehash(fallback, size);
    };
    div()
        .size(px(size))
        .flex_shrink_0()
        .relative()
        .shadow(vec![gpui::BoxShadow {
            color: rgba(0x0000000d).into(),
            offset: gpui::point(px(0.), px(1.)),
            blur_radius: px(2.),
            spread_radius: px(0.),
            inset: false,
        }])
        .child(
            img(image)
                .absolute()
                .inset_0()
                .size_full()
                .object_fit(ObjectFit::Cover),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .border_1()
                .border_color(rgba(0x0000001a)),
        )
}

pub fn person(avatar: Option<&SharedString>, name: &str, size: f32) -> gpui::Div {
    match avatar {
        Some(avatar) => contact_image(avatar, name, size),
        None => facehash(name, size),
    }
}

/// The `Buildings` placeholder; `muted` behind it in lists, `accent` on the details page.
pub fn organization(
    avatar: Option<&SharedString>,
    name: &str,
    size: f32,
    background: Hsla,
    colors: Theme,
) -> gpui::Div {
    if let Some(avatar) = avatar {
        return contact_image(avatar, name, size);
    }
    div()
        .size(px(size))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(background)
        .child(
            svg()
                .path("contacts/Building03Icon.svg")
                .size(px(size / 2.))
                .text_color(colors.muted_foreground),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_avatars_match_shipping_pixels_and_initials() {
        // Reference values from shipping's `createAvatarPixels` for the `Avatar` defaults.
        let pixels = pixels("Maya Chen");
        let checksum = pixels
            .iter()
            .map(|[r, g, b]| *r as u64 * 3 + *g as u64 * 5 + *b as u64 * 7)
            .sum::<u64>();
        assert_eq!(checksum, 8_522_465);
        assert_eq!(pixels[0], [85, 170, 42]);
        assert_eq!(pixels[32 * 64 + 32], [128, 85, 85]);
        assert_eq!(pixels[4095], [170, 212, 42]);
        assert_eq!(initials("  maya   chen lee "), "MC");
        assert_eq!(initials("taylor@fabrikam.example"), "T");
        assert_eq!(initials("(alex) 김민"), "A김");
        assert_eq!(initials(""), "");
    }
}
