use image::{DynamicImage, RgbImage, RgbaImage, imageops::FilterType};

const SIGNATURE_WIDTH: u32 = 64;
const SIGNATURE_HEIGHT: u32 = 36;
const MAX_CAPTURE_WIDTH: u32 = 1920;
const JPEG_QUALITY: u8 = 80;
// Mean per-channel difference (0..1) on the downscaled colour signature.
const STABLE_FRAME_DISTANCE: f32 = 0.02;
const NEW_CONTENT_DISTANCE: f32 = 0.06;
// Same-template slides that only change text or swap a similarly bright
// colour have a low mean difference, so also count strongly changed cells.
const CHANGED_CELL_DIFFERENCE: u8 = 32;
const NEW_CONTENT_CHANGED_CELLS: f32 = 0.04;

#[derive(Debug, Clone, PartialEq)]
pub struct FrameSignature(Vec<u8>);

impl FrameSignature {
    pub fn of(image: &RgbaImage) -> Self {
        let rgb: RgbImage = DynamicImage::ImageRgba8(image.clone())
            .resize_exact(SIGNATURE_WIDTH, SIGNATURE_HEIGHT, FilterType::Triangle)
            .to_rgb8();
        Self(rgb.into_raw())
    }

    pub fn distance(&self, other: &Self) -> f32 {
        if self.0.len() != other.0.len() || self.0.is_empty() {
            return 1.0;
        }
        let total: u32 = self
            .0
            .iter()
            .zip(&other.0)
            .map(|(a, b)| u32::from(a.abs_diff(*b)))
            .sum();
        total as f32 / (self.0.len() as f32 * 255.0)
    }

    fn changed_cell_fraction(&self, other: &Self) -> f32 {
        if self.0.len() != other.0.len() || self.0.is_empty() {
            return 1.0;
        }
        let cells = self.0.chunks_exact(3).zip(other.0.chunks_exact(3));
        let total = self.0.len() / 3;
        let changed = cells
            .filter(|(a, b)| {
                a.iter()
                    .zip(b.iter())
                    .any(|(a, b)| a.abs_diff(*b) >= CHANGED_CELL_DIFFERENCE)
            })
            .count();
        changed as f32 / total as f32
    }

    fn shows_new_content(&self, kept: &Self) -> bool {
        self.distance(kept) >= NEW_CONTENT_DISTANCE
            || self.changed_cell_fraction(kept) >= NEW_CONTENT_CHANGED_CELLS
    }
}

/// Keeps a frame once it has held still for two samples (skips live video and
/// slide transitions) and differs from the last kept frame (a new slide).
#[derive(Debug, Default)]
pub struct ScreenShareSampler {
    previous: Option<FrameSignature>,
    last_kept: Option<FrameSignature>,
}

impl ScreenShareSampler {
    pub fn observe(&mut self, signature: FrameSignature) -> bool {
        let stable = self
            .previous
            .as_ref()
            .is_some_and(|previous| previous.distance(&signature) <= STABLE_FRAME_DISTANCE);
        let new_content = self
            .last_kept
            .as_ref()
            .is_none_or(|kept| signature.shows_new_content(kept));
        let keep = stable && new_content;
        if keep {
            self.last_kept = Some(signature.clone());
        }
        self.previous = Some(signature);
        keep
    }

    /// Forget the in-progress sample but keep the last kept frame so a share
    /// that pauses and resumes on the same slide is not captured twice.
    pub fn pause(&mut self) {
        self.previous = None;
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn capture_meeting_window(pid: i32, window_title: Option<&str>) -> Result<RgbaImage, String> {
    let pid = u32::try_from(pid).map_err(|_| "invalid meeting process id".to_string())?;
    let windows = xcap::Window::all().map_err(|error| error.to_string())?;
    let window_title = window_title
        .map(str::trim)
        .filter(|title| !title.is_empty());

    let window = windows
        .into_iter()
        .filter(|window| window.pid().is_ok_and(|window_pid| window_pid == pid))
        .filter(|window| !window.is_minimized().unwrap_or(false))
        .max_by_key(|window| {
            let title_match = window_title
                .is_some_and(|expected| window.title().is_ok_and(|title| title.trim() == expected));
            let area =
                u64::from(window.width().unwrap_or(0)) * u64::from(window.height().unwrap_or(0));
            (title_match, area)
        })
        .ok_or_else(|| "meeting window not found".to_string())?;

    window.capture_image().map_err(|error| error.to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn capture_meeting_window(_pid: i32, _window_title: Option<&str>) -> Result<RgbaImage, String> {
    Err("screen share capture is not supported on this platform".to_string())
}

pub fn encode_capture_jpeg(image: &RgbaImage) -> Result<(Vec<u8>, u32, u32), String> {
    let mut image = DynamicImage::ImageRgba8(image.clone());
    if image.width() > MAX_CAPTURE_WIDTH {
        image = image.resize(MAX_CAPTURE_WIDTH, u32::MAX, FilterType::Triangle);
    }
    let rgb = image.to_rgb8();
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, JPEG_QUALITY)
        .encode_image(&rgb)
        .map_err(|error| error.to_string())?;
    Ok((bytes, rgb.width(), rgb.height()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slide(shade: u8) -> RgbaImage {
        RgbaImage::from_fn(320, 180, |x, _| {
            if x < 160 {
                image::Rgba([shade, shade, shade, 255])
            } else {
                image::Rgba([255 - shade, 255 - shade, 255 - shade, 255])
            }
        })
    }

    #[test]
    fn keeps_each_slide_once_after_it_settles() {
        let mut sampler = ScreenShareSampler::default();
        let first = FrameSignature::of(&slide(20));
        let second = FrameSignature::of(&slide(200));

        assert!(!sampler.observe(first.clone()), "first sample is unsettled");
        assert!(sampler.observe(first.clone()), "settled slide is kept");
        assert!(!sampler.observe(first.clone()), "same slide is not re-kept");
        assert!(!sampler.observe(second.clone()), "transition is unsettled");
        assert!(sampler.observe(second.clone()), "next slide is kept");

        sampler.pause();
        assert!(!sampler.observe(second.clone()));
        assert!(
            !sampler.observe(second),
            "resuming on the same slide is not re-kept"
        );
    }

    fn template_slide(background: [u8; 3], text_rows: std::ops::Range<u32>) -> RgbaImage {
        RgbaImage::from_fn(320, 180, |x, y| {
            if (20..300).contains(&x) && text_rows.contains(&y) {
                image::Rgba([255, 255, 255, 255])
            } else {
                image::Rgba([background[0], background[1], background[2], 255])
            }
        })
    }

    fn keeps_after_settling(sampler: &mut ScreenShareSampler, image: &RgbaImage) -> bool {
        let signature = FrameSignature::of(image);
        sampler.observe(signature.clone());
        sampler.observe(signature)
    }

    #[test]
    fn keeps_similar_looking_slides_from_one_deck() {
        let mut sampler = ScreenShareSampler::default();
        assert!(keeps_after_settling(
            &mut sampler,
            &template_slide([0xac, 0x24, 0x1a], 20..40)
        ));
        assert!(
            keeps_after_settling(&mut sampler, &template_slide([0x58, 0x21, 0x8f], 20..40)),
            "equally bright background in another colour"
        );
        assert!(
            keeps_after_settling(&mut sampler, &template_slide([0x58, 0x21, 0x8f], 60..80)),
            "same template with only the text moved"
        );
    }

    #[test]
    fn skips_frames_that_never_settle() {
        let mut sampler = ScreenShareSampler::default();
        for shade in [0, 120, 240, 60, 180] {
            assert!(!sampler.observe(FrameSignature::of(&slide(shade))));
        }
    }

    #[test]
    fn encodes_downscaled_jpeg() {
        let wide = RgbaImage::from_pixel(3840, 2160, image::Rgba([10, 20, 30, 255]));
        let (bytes, width, height) = encode_capture_jpeg(&wide).unwrap();
        assert_eq!((width, height), (1920, 1080));
        assert_eq!(&bytes[..2], &[0xFF, 0xD8]);
    }
}
