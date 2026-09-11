//! `store/zustand/ai-task/task-configs/enhance-images.ts`: the note's images,
//! read from the session's attachments and squeezed into the prompt budget,
//! as visual context for models that take images.

use std::path::Path;
use std::sync::LazyLock;

use base64::Engine as _;
use regex::Regex;
use serde_json::Value;

const MAX_IMAGE_COUNT: usize = 10;
const MAX_IMAGE_BYTES: usize = 128 * 1024;
const MAX_TOTAL_IMAGE_BYTES: usize = 768 * 1024;
const MAX_SOURCE_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_COMPRESSED_IMAGE_EDGE: u32 = 1280;
const MIN_COMPRESSED_IMAGE_EDGE: u32 = 512;
/// `COMPRESSED_IMAGE_QUALITY_STEPS` as JPEG encoder qualities.
const COMPRESSED_IMAGE_QUALITY_STEPS: [u8; 4] = [82, 72, 62, 52];

/// `IMAGE_CONTEXT_NOTE`
pub const IMAGE_CONTEXT_NOTE: &str = "Attached note images are included as visual context. Use visible text, diagrams, screenshots, and other image content when it materially improves the summary.";

static MARKDOWN_IMAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"!\[[^\]]*]\((<[^>]+>|[^)\s]+)(?:\s+"[^"]*")?\)"#).unwrap());
static DATA_URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)^data:(image/(?:gif|jpe?g|png|webp));base64,(.+)$").unwrap()
});
static TEXT_ONLY_MODEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|[/:\-.])(?:gpt-3\.5|claude-2|claude-instant|davinci|babbage|curie|ada|dall-e|sora|gpt-image|image-generation|embed|embedding|whisper|tts|transcribe|moderation|realtime|computer)(?:$|[/:\-.])").unwrap()
});
static IMAGE_INPUT_MODEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:gpt-4o|gpt-4\.1|gpt-5|chat-latest|claude-3|claude-sonnet|claude-opus|claude-haiku|claude-fable|claude-mythos|gemini|grok-4|kimi-k3|glm-5\.3-flash|pixtral|vision|vl|llava|llama-3\.2-vision|llama3\.2-vision|moondream|minicpm-v|internvl|qwen(?:2|2\.5|3)?-vl|gemma-3|gemma3)").unwrap()
});

/// `EnhanceImageContext`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageContext {
    pub base64: String,
    pub mime_type: String,
    pub filename: Option<String>,
}

/// `ImageReference`: where a note points at an image.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImageReference {
    pub attachment_id: Option<String>,
    pub filename: Option<String>,
    pub data_url: Option<ImageContext>,
}

/// `modelSupportsImageInput`
pub fn model_supports_image_input(provider: Option<&str>, model: Option<&str>) -> bool {
    let (Some(provider), Some(model)) = (provider, model) else {
        return false;
    };
    if provider.is_empty() || model.is_empty() {
        return false;
    }
    if provider == "cloudflare_workers_ai"
        && crate::ai_models::CLOUDFLARE_WORKERS_AI_MODELS.contains(&model)
    {
        return crate::ai_models::CLOUDFLARE_VISION_MODELS.contains(&model);
    }
    if TEXT_ONLY_MODEL.is_match(model) {
        return false;
    }
    if provider == "anarlog" && model == "Auto" {
        return true;
    }
    IMAGE_INPUT_MODEL.is_match(model)
}

/// `collectEnhanceImageContext`: the referenced images in document order,
/// each within `MAX_IMAGE_BYTES` and all within `MAX_TOTAL_IMAGE_BYTES`,
/// at most `MAX_IMAGE_COUNT` (sampled evenly when there are more).
pub fn collect_enhance_image_context(session_dir: &Path, contents: &[&str]) -> Vec<ImageContext> {
    let references: Vec<ImageReference> = contents
        .iter()
        .flat_map(|content| collect_image_references(content))
        .collect();
    let candidates: Vec<(usize, ImageReference)> = references
        .into_iter()
        .enumerate()
        .filter(|(_, reference)| {
            reference.data_url.is_some()
                || reference.attachment_id.is_some()
                || reference.filename.is_some()
        })
        .collect();
    let mut images: Vec<(usize, ImageContext)> = Vec::new();
    let mut total = 0usize;
    let mut seen = std::collections::HashSet::new();
    let mut attachments: Option<Vec<anlg_fs_sync_core::AttachmentInfo>> = None;

    for (index, reference) in prioritize(candidates) {
        let source = if let Some(data_url) = reference.data_url {
            Some(data_url)
        } else {
            let list = attachments.get_or_insert_with(|| {
                anlg_fs_sync_core::attachments::list(session_dir).unwrap_or_else(|error| {
                    tracing::warn!(%error, "[enhance] failed to list image attachments");
                    Vec::new()
                })
            });
            let attachment = reference
                .attachment_id
                .as_deref()
                .and_then(|id| list.iter().find(|a| a.attachment_id == id))
                .or_else(|| {
                    reference
                        .filename
                        .as_deref()
                        .and_then(|name| list.iter().find(|a| a.attachment_id == name))
                })
                .or_else(|| {
                    reference.filename.as_deref().and_then(|name| {
                        list.iter().find(|a| {
                            path_filename(&a.path)
                                .as_deref()
                                .unwrap_or(&a.attachment_id)
                                == name
                        })
                    })
                });
            let Some(attachment) = attachment else {
                continue;
            };
            if !seen.insert(attachment.attachment_id.clone()) {
                continue;
            }
            read_image_attachment(session_dir, attachment)
        };
        let Some(source) = source else {
            continue;
        };
        let Some(budgeted) = prepare_for_budget(source, total) else {
            continue;
        };
        total += base64_byte_length(&budgeted.base64);
        images.push((index, budgeted));
        if images.len() >= MAX_IMAGE_COUNT {
            break;
        }
    }
    images.sort_by_key(|(index, _)| *index);
    images.into_iter().map(|(_, image)| image).collect()
}

/// `prioritizeImageReferences`: past the cap, ten evenly spaced references
/// come first, then the rest in order.
fn prioritize(references: Vec<(usize, ImageReference)>) -> Vec<(usize, ImageReference)> {
    if references.len() <= MAX_IMAGE_COUNT {
        return references;
    }
    let last = references.len() - 1;
    let mut selected: Vec<usize> = Vec::new();
    for i in 0..MAX_IMAGE_COUNT {
        let index = ((i * last) as f64 / (MAX_IMAGE_COUNT - 1) as f64).round() as usize;
        if !selected.contains(&index) {
            selected.push(index);
        }
    }
    let mut ordered: Vec<(usize, ImageReference)> = selected
        .iter()
        .map(|index| references[*index].clone())
        .collect();
    ordered.extend(
        references
            .iter()
            .enumerate()
            .filter(|(index, _)| !selected.contains(index))
            .map(|(_, reference)| reference.clone()),
    );
    ordered
}

/// `prepareImageForBudget`: keep, compress to fit, or drop.
fn prepare_for_budget(image: ImageContext, total: usize) -> Option<ImageContext> {
    let target = MAX_IMAGE_BYTES.min(MAX_TOTAL_IMAGE_BYTES.saturating_sub(total));
    if target == 0 {
        return None;
    }
    if base64_byte_length(&image.base64) <= target {
        return Some(image);
    }
    compress(&image, target)
}

/// `compressImageContext`: re-encode as JPEG, shrinking the longest edge from
/// 1280 down by quarters to 512 and stepping the quality down at each size.
fn compress(image: &ImageContext, target: usize) -> Option<ImageContext> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(image.base64.replace(char::is_whitespace, ""))
        .ok()?;
    let decoded = image::load_from_memory(&bytes).ok()?;
    let (width, height) = (decoded.width(), decoded.height());
    let mut max_edge = MAX_COMPRESSED_IMAGE_EDGE;
    while max_edge >= MIN_COMPRESSED_IMAGE_EDGE {
        let scale = (max_edge as f64 / width.max(height) as f64).min(1.0);
        let scaled_width = ((width as f64 * scale).round() as u32).max(1);
        let scaled_height = ((height as f64 * scale).round() as u32).max(1);
        let resized = decoded
            .resize_exact(
                scaled_width,
                scaled_height,
                image::imageops::FilterType::Triangle,
            )
            .into_rgb8();
        for quality in COMPRESSED_IMAGE_QUALITY_STEPS {
            let mut out = Vec::new();
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
            if encoder
                .encode(
                    resized.as_raw(),
                    scaled_width,
                    scaled_height,
                    image::ExtendedColorType::Rgb8,
                )
                .is_err()
            {
                return None;
            }
            if out.len() <= target {
                return Some(ImageContext {
                    base64: base64::engine::general_purpose::STANDARD.encode(&out),
                    mime_type: "image/jpeg".to_string(),
                    filename: image.filename.clone(),
                });
            }
        }
        max_edge = (max_edge as f64 * 0.75).floor() as u32;
    }
    None
}

fn read_image_attachment(
    session_dir: &Path,
    attachment: &anlg_fs_sync_core::AttachmentInfo,
) -> Option<ImageContext> {
    let mime_type = image_mime_type(Some(&attachment.extension))?;
    let bytes = match anlg_fs_sync_core::attachments::read(session_dir, &attachment.attachment_id) {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::warn!(%error, "[enhance] failed to read image attachment");
            return None;
        }
    };
    if bytes.len() > MAX_SOURCE_IMAGE_BYTES {
        return None;
    }
    Some(ImageContext {
        base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
        mime_type: mime_type.to_string(),
        filename: Some(attachment.attachment_id.clone()),
    })
}

/// `collectImageReferences`: a TipTap JSON document's `image` and image
/// `fileAttachment` nodes, or a markdown text's `![…](src)` images.
pub fn collect_image_references(raw: &str) -> Vec<ImageReference> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    if trimmed.starts_with('{') {
        return match serde_json::from_str::<Value>(trimmed) {
            Ok(value) => {
                let mut references = Vec::new();
                visit_json(&value, &mut references);
                references
            }
            Err(_) => Vec::new(),
        };
    }
    MARKDOWN_IMAGE
        .captures_iter(trimmed)
        .map(|captures| reference_from_src(&unwrap_markdown_url(&captures[1])))
        .collect()
}

fn visit_json(node: &Value, references: &mut Vec<ImageReference>) {
    let Some(object) = node.as_object() else {
        return;
    };
    let kind = object.get("type").and_then(Value::as_str).unwrap_or("");
    if kind == "image" || kind == "fileAttachment" {
        let attrs = object.get("attrs");
        let attr = |name: &str| {
            attrs
                .and_then(|attrs| attrs.get(name))
                .and_then(Value::as_str)
                .unwrap_or("")
        };
        let src = attr("src");
        let is_image = kind == "image"
            || attr("mimeType").starts_with("image/")
            || image_mime_type(path_extension(src).as_deref()).is_some();
        if is_image {
            let mut reference = reference_from_src(src);
            let attachment_id = attr("attachmentId");
            reference.attachment_id =
                (!attachment_id.is_empty()).then(|| attachment_id.to_string());
            if reference.filename.is_none() {
                reference.filename = attachment_filename(attr("path"));
            }
            references.push(reference);
        }
    }
    if let Some(content) = object.get("content").and_then(Value::as_array) {
        for child in content {
            visit_json(child, references);
        }
    }
}

fn reference_from_src(src: &str) -> ImageReference {
    if let Some(data_url) = parse_image_data_url(src) {
        return ImageReference {
            data_url: Some(data_url),
            ..ImageReference::default()
        };
    }
    ImageReference {
        filename: attachment_filename(src),
        ..ImageReference::default()
    }
}

fn parse_image_data_url(src: &str) -> Option<ImageContext> {
    let captures = DATA_URL.captures(src)?;
    let mime_type = captures[1].to_ascii_lowercase();
    let base64 = captures[2].to_string();
    if base64_byte_length(&base64) > MAX_SOURCE_IMAGE_BYTES {
        return None;
    }
    Some(ImageContext {
        base64,
        mime_type: if mime_type == "image/jpg" {
            "image/jpeg".to_string()
        } else {
            mime_type
        },
        filename: None,
    })
}

fn unwrap_markdown_url(src: &str) -> String {
    let unwrapped = src
        .strip_prefix('<')
        .and_then(|rest| rest.strip_suffix('>'))
        .unwrap_or(src);
    unwrapped.replace("\\(", "(").replace("\\)", ")")
}

fn image_mime_type(extension: Option<&str>) -> Option<&'static str> {
    match extension?.to_ascii_lowercase().as_str() {
        "gif" => Some("image/gif"),
        "jpeg" | "jpg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

fn path_extension(path: &str) -> Option<String> {
    let filename = path_filename(path)?;
    let dot = filename.rfind('.')?;
    Some(filename[dot + 1..].to_string())
}

fn path_filename(path: &str) -> Option<String> {
    let normalized = normalize_path_like(path);
    normalized
        .split(['/', '\\'])
        .rfind(|part| !part.is_empty())
        .map(decode_path_part)
}

/// `getAttachmentFilename`: the file name of an `asset:` / `file:` URL or of
/// a path under an `attachments` folder.
fn attachment_filename(src: &str) -> Option<String> {
    let trimmed = src.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(url) = url::Url::parse(trimmed) {
        return match url.scheme() {
            "asset" | "file" => path_filename(trimmed),
            _ => None,
        };
    }
    let normalized = normalize_path_like(trimmed);
    if !normalized.contains("/attachments/") && !normalized.contains("\\attachments\\") {
        return None;
    }
    path_filename(&normalized)
}

fn normalize_path_like(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if let Ok(url) = url::Url::parse(trimmed)
        && matches!(url.scheme(), "asset" | "file")
    {
        return decode_path_part(url.path());
    }
    decode_path_part(trimmed)
}

/// `decodeURIComponent`, keeping the input when it is not valid UTF-8.
fn decode_path_part(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3])
            && let Ok(byte) = u8::from_str_radix(hex, 16)
        {
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| value.to_string())
}

/// `getBase64ByteLength`
pub fn base64_byte_length(base64: &str) -> usize {
    let normalized: String = base64.chars().filter(|c| !c.is_whitespace()).collect();
    let padding = if normalized.ends_with("==") {
        2
    } else if normalized.ends_with('=') {
        1
    } else {
        0
    };
    (normalized.len() * 3 / 4).saturating_sub(padding)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_capabilities_follow_the_desktop_rules() {
        assert!(model_supports_image_input(
            Some("openai"),
            Some("gpt-4o-mini")
        ));
        assert!(model_supports_image_input(
            Some("anthropic"),
            Some("claude-sonnet-4")
        ));
        assert!(model_supports_image_input(
            Some("google"),
            Some("gemini-2.5-flash")
        ));
        assert!(model_supports_image_input(Some("anarlog"), Some("Auto")));
        assert!(!model_supports_image_input(
            Some("openai"),
            Some("gpt-3.5-turbo")
        ));
        assert!(!model_supports_image_input(
            Some("openai"),
            Some("text-embedding-3-small")
        ));
        assert!(!model_supports_image_input(
            Some("ollama"),
            Some("llama3.1")
        ));
        assert!(model_supports_image_input(
            Some("ollama"),
            Some("llama3.2-vision")
        ));
        assert!(!model_supports_image_input(None, Some("gpt-4o")));
        assert!(!model_supports_image_input(Some("custom"), Some("")));
    }

    #[test]
    fn references_come_from_json_nodes_and_markdown_images() {
        let json = r#"{"type":"doc","content":[{"type":"paragraph"},{"type":"image","attrs":{"src":"asset://localhost/%2Fv%2Fs%2Fattachments%2Fimage.png","attachmentId":"image.png"}},{"type":"fileAttachment","attrs":{"attachmentId":"notes.pdf","mimeType":"application/pdf","src":"asset://x/notes.pdf","path":"/v/attachments/notes.pdf"}},{"type":"fileAttachment","attrs":{"attachmentId":"shot.jpg","mimeType":"image/jpeg","src":"","path":"/v/s/attachments/shot.jpg"}},{"type":"image","attrs":{"src":"data:image/png;base64,AAAA"}}]}"#;
        let references = collect_image_references(json);
        assert_eq!(references.len(), 3);
        assert_eq!(references[0].attachment_id.as_deref(), Some("image.png"));
        assert_eq!(references[0].filename.as_deref(), Some("image.png"));
        assert_eq!(references[1].attachment_id.as_deref(), Some("shot.jpg"));
        assert_eq!(references[1].filename.as_deref(), Some("shot.jpg"));
        assert_eq!(
            references[2]
                .data_url
                .as_ref()
                .map(|d| d.mime_type.as_str()),
            Some("image/png")
        );

        let markdown = "Intro\n\n![Diagram](asset://localhost/%2Fv%2Fs%2Fattachments%2Fdiagram%20v2.png \"t\")\n\n![](<https://example.com/a.png>)\n\n![x](data:image/jpg;base64,QUJD)";
        let references = collect_image_references(markdown);
        assert_eq!(references.len(), 3);
        assert_eq!(references[0].filename.as_deref(), Some("diagram v2.png"));
        assert_eq!(references[1].filename, None);
        assert_eq!(
            references[2]
                .data_url
                .as_ref()
                .map(|d| d.mime_type.as_str()),
            Some("image/jpeg")
        );
        assert!(collect_image_references("  ").is_empty());
        assert!(collect_image_references("{not json").is_empty());
    }

    #[test]
    fn sampling_keeps_ten_evenly_spaced_references_first() {
        let references: Vec<(usize, ImageReference)> = (0..23)
            .map(|index| {
                (
                    index,
                    ImageReference {
                        filename: Some(format!("{index}.png")),
                        ..ImageReference::default()
                    },
                )
            })
            .collect();
        let ordered = prioritize(references);
        let first: Vec<usize> = ordered.iter().take(10).map(|(index, _)| *index).collect();
        assert_eq!(first, vec![0, 2, 5, 7, 10, 12, 15, 17, 20, 22]);
        assert_eq!(ordered.len(), 23);
        assert_eq!(ordered[10].0, 1);
    }

    #[test]
    fn budget_and_lengths() {
        assert_eq!(base64_byte_length("QUJD"), 3);
        assert_eq!(base64_byte_length("QUI="), 2);
        assert_eq!(base64_byte_length("QQ=="), 1);
        assert_eq!(base64_byte_length("QU JD\n"), 3);
        let small = ImageContext {
            base64: "QUJD".into(),
            mime_type: "image/png".into(),
            filename: None,
        };
        assert_eq!(prepare_for_budget(small.clone(), 0), Some(small.clone()));
        assert_eq!(prepare_for_budget(small, MAX_TOTAL_IMAGE_BYTES), None);
    }

    #[test]
    fn large_images_are_recompressed_as_jpeg() {
        // A 1600×1200 noisy PNG is well over 128 KB; the JPEG fits.
        let mut pixels = image::RgbImage::new(1600, 1200);
        for (x, y, pixel) in pixels.enumerate_pixels_mut() {
            let v = ((x * 7 + y * 13) % 251) as u8;
            *pixel = image::Rgb([v, v.wrapping_mul(3), v.wrapping_add(90)]);
        }
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(pixels)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        assert!(png.len() > MAX_IMAGE_BYTES);
        let source = ImageContext {
            base64: base64::engine::general_purpose::STANDARD.encode(&png),
            mime_type: "image/png".into(),
            filename: Some("big.png".into()),
        };
        let budgeted = prepare_for_budget(source, 0).expect("compresses");
        assert_eq!(budgeted.mime_type, "image/jpeg");
        assert_eq!(budgeted.filename.as_deref(), Some("big.png"));
        assert!(base64_byte_length(&budgeted.base64) <= MAX_IMAGE_BYTES);
        let decoded = image::load_from_memory(
            &base64::engine::general_purpose::STANDARD
                .decode(&budgeted.base64)
                .unwrap(),
        )
        .unwrap();
        assert!(decoded.width() <= MAX_COMPRESSED_IMAGE_EDGE);
    }

    #[test]
    fn attachments_are_read_and_deduplicated() {
        let dir = tempfile::tempdir().unwrap();
        let attachments = dir.path().join("attachments");
        std::fs::create_dir_all(&attachments).unwrap();
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(image::RgbImage::new(4, 4))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        std::fs::write(attachments.join("image.png"), &png).unwrap();
        std::fs::write(attachments.join("notes.txt"), b"text").unwrap();
        let memo = r#"{"type":"doc","content":[{"type":"image","attrs":{"src":"asset://localhost/x","attachmentId":"image.png"}},{"type":"image","attrs":{"src":"asset://localhost/%2Fs%2Fattachments%2Fimage.png","attachmentId":null}},{"type":"fileAttachment","attrs":{"attachmentId":"notes.txt","mimeType":"text/plain","src":"","path":""}},{"type":"image","attrs":{"src":"asset://localhost/%2Fs%2Fattachments%2Fmissing.png"}}]}"#;
        let images = collect_enhance_image_context(
            dir.path(),
            &[memo, "![a](data:image/gif;base64,R0lGODdh)"],
        );
        assert_eq!(images.len(), 2);
        assert_eq!(images[0].mime_type, "image/png");
        assert_eq!(images[0].filename.as_deref(), Some("image.png"));
        assert_eq!(
            images[0].base64,
            base64::engine::general_purpose::STANDARD.encode(&png)
        );
        assert_eq!(images[1].mime_type, "image/gif");
        assert_eq!(images[1].base64, "R0lGODdh");
    }
}
