use image::{DynamicImage, GrayImage, RgbaImage, imageops::FilterType};

const SIGNATURE_WIDTH: u32 = 32;
const SIGNATURE_HEIGHT: u32 = 18;
const MAX_CAPTURE_WIDTH: u32 = 1920;
const JPEG_QUALITY: u8 = 80;
// Mean per-pixel difference (0..1) on the downscaled grayscale signature.
const STABLE_FRAME_DISTANCE: f32 = 0.02;
const NEW_CONTENT_DISTANCE: f32 = 0.06;

#[derive(Debug, Clone, PartialEq)]
pub struct FrameSignature(Vec<u8>);

impl FrameSignature {
    pub fn of(image: &RgbaImage) -> Self {
        let gray: GrayImage = DynamicImage::ImageRgba8(image.clone())
            .resize_exact(SIGNATURE_WIDTH, SIGNATURE_HEIGHT, FilterType::Triangle)
            .to_luma8();
        Self(gray.into_raw())
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
            .is_none_or(|kept| kept.distance(&signature) >= NEW_CONTENT_DISTANCE);
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
