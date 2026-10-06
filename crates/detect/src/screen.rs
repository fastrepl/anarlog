use std::collections::VecDeque;

use image::{DynamicImage, RgbImage, RgbaImage, imageops::FilterType};

use crate::MeetingAccessibilityInspection;

const SIGNATURE_WIDTH: u32 = 64;
const SIGNATURE_HEIGHT: u32 = 36;
const MAX_CAPTURE_WIDTH: u32 = 1920;
const JPEG_QUALITY: u8 = 80;
// Going back to an earlier slide should not save it again.
const MAX_REMEMBERED_FRAMES: usize = 200;
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

    // Only the inner area counts, so meeting chrome along the window edges
    // (auto-hiding toolbars, participant strips) does not look like a new slide.
    fn shows_new_content(&self, kept: &Self) -> bool {
        if self.0.len() != kept.0.len() || self.0.is_empty() {
            return true;
        }
        let (margin_x, margin_y) = (SIGNATURE_WIDTH / 8, SIGNATURE_HEIGHT / 8);
        let (mut cells, mut changed_cells, mut total_difference) = (0u32, 0u32, 0u32);
        for y in margin_y..SIGNATURE_HEIGHT - margin_y {
            for x in margin_x..SIGNATURE_WIDTH - margin_x {
                let start = ((y * SIGNATURE_WIDTH + x) * 3) as usize;
                let differences = self.0[start..start + 3]
                    .iter()
                    .zip(&kept.0[start..start + 3])
                    .map(|(a, b)| a.abs_diff(*b));
                let mut changed = false;
                for difference in differences {
                    total_difference += u32::from(difference);
                    changed |= difference >= CHANGED_CELL_DIFFERENCE;
                }
                cells += 1;
                changed_cells += u32::from(changed);
            }
        }
        let mean = total_difference as f32 / (cells as f32 * 3.0 * 255.0);
        mean >= NEW_CONTENT_DISTANCE
            || changed_cells as f32 / cells as f32 >= NEW_CONTENT_CHANGED_CELLS
    }
}

/// Keeps a frame once it has held still for two samples (skips live video and
/// slide transitions) and differs from every frame kept so far (a new slide).
#[derive(Debug, Default)]
pub struct ScreenShareSampler {
    previous: Option<FrameSignature>,
    kept: VecDeque<FrameSignature>,
    followed: Option<MeetingAccessibilityInspection>,
}

impl ScreenShareSampler {
    /// Picks the meeting whose remote share should be captured. Zoom drops most
    /// of its accessibility tree (share label and Leave button included) while
    /// its controls auto-hide, so a share stays followed until the meeting is
    /// seen again without it or its window can no longer be captured.
    pub fn follow(
        &mut self,
        inspections: &[MeetingAccessibilityInspection],
    ) -> Option<MeetingAccessibilityInspection> {
        if let Some(sharing) = inspections
            .iter()
            .find(|inspection| inspection.remote_screen_share)
        {
            self.followed = Some(sharing.clone());
        } else if let Some(followed) = &self.followed
            && inspections
                .iter()
                .any(|inspection| inspection.pid == followed.pid && inspection.active_call)
        {
            self.followed = None;
        }
        self.followed.clone()
    }

    pub fn unfollow(&mut self) {
        self.followed = None;
    }

    pub fn observe(&mut self, signature: FrameSignature) -> bool {
        let stable = self
            .previous
            .as_ref()
            .is_some_and(|previous| previous.distance(&signature) <= STABLE_FRAME_DISTANCE);
        let new_content = self
            .kept
            .iter()
            .all(|kept| signature.shows_new_content(kept));
        let keep = stable && new_content;
        if keep {
            if self.kept.len() == MAX_REMEMBERED_FRAMES {
                self.kept.pop_front();
            }
            self.kept.push_back(signature.clone());
        }
        self.previous = Some(signature);
        keep
    }

    /// Forget the in-progress sample but remember kept frames so a share
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
        assert!(
            !keeps_after_settling(&mut sampler, &template_slide([0xac, 0x24, 0x1a], 20..40)),
            "going back to an earlier slide"
        );
    }

    #[test]
    fn ignores_meeting_chrome_along_the_window_edges() {
        for background in [[0xac, 0x24, 0x1a], [0xff, 0xff, 0xff]] {
            let slide = template_slide(background, 60..80);
            let mut with_chrome = slide.clone();
            for (x, y, pixel) in with_chrome.enumerate_pixels_mut() {
                if y >= 160 || (x >= 260 && y < 40) {
                    *pixel = image::Rgba([30, 30, 30, 255]);
                }
            }

            let mut sampler = ScreenShareSampler::default();
            assert!(keeps_after_settling(&mut sampler, &slide));
            assert!(!keeps_after_settling(&mut sampler, &with_chrome));
            assert!(!keeps_after_settling(&mut sampler, &slide));
        }
    }

    fn inspection(
        pid: i32,
        active_call: bool,
        remote_screen_share: bool,
    ) -> MeetingAccessibilityInspection {
        MeetingAccessibilityInspection {
            active_call,
            app: crate::MeetingApp {
                id: "us.zoom.xos".to_string(),
                name: "zoom.us".to_string(),
            },
            pid,
            platform: crate::MeetingPlatform::Zoom,
            surface: crate::MeetingSurface::Native,
            accessibility_trusted: true,
            window_title: Some("Zoom Meeting".to_string()),
            remote_screen_share,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn follows_a_share_while_the_meeting_hides_its_controls() {
        let mut sampler = ScreenShareSampler::default();
        assert!(sampler.follow(&[inspection(7, false, false)]).is_none());
        assert_eq!(
            sampler.follow(&[inspection(7, true, true)]).map(|i| i.pid),
            Some(7)
        );
        assert_eq!(
            sampler
                .follow(&[inspection(7, false, false)])
                .map(|i| i.pid),
            Some(7),
            "controls hidden: keep following"
        );
        assert!(
            sampler.follow(&[inspection(7, true, false)]).is_none(),
            "visible meeting without a share ends it"
        );
        assert!(sampler.follow(&[]).is_none());
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
