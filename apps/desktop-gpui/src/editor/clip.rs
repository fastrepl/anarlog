//! `packages/editor/src/plugins/clip-paste.ts`: a pasted YouTube link, embed
//! snippet or clip link becomes a `clip` block atom holding the embed URL.
//! The node has no view or styles in the desktop app, so it renders as an
//! empty block there and is skipped here; the stored JSON is what matters.

use std::sync::LazyLock;

use regex::Regex;
use url::Url;

static CLIP_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:youtube\.com|youtu\.be)/clip/([a-zA-Z0-9_-]+)").unwrap());
static CLIP_TAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)^<Clip\b[^>]*\bsrc\s*=\s*["']([^"']+)["'][^>]*(?:/>|></Clip>)"#).unwrap()
});
static IFRAME_TAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)^<iframe\b[^>]*\bsrc\s*=\s*["']([^"']+)["'][^>]*>\s*</iframe>"#).unwrap()
});
static IFRAME_START: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^<iframe\b").unwrap());
static SRC_ATTR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)\bsrc\s*=\s*["']([^"']+)["']"#).unwrap());
static VIDEO_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""videoId":"([a-zA-Z0-9_-]+)""#).unwrap());

/// `parseYouTubeClipId`
pub fn parse_youtube_clip_id(url: &str) -> Option<String> {
    CLIP_ID
        .captures(url.trim())
        .map(|captures| captures[1].to_string())
}

/// `normalizeYouTubeTime`: `90s` → `90`.
fn normalize_time(value: Option<String>) -> Option<String> {
    value.map(|value| value.strip_suffix('s').unwrap_or(&value).to_string())
}

fn query_param(url: &Url, name: &str) -> Option<String> {
    url.query_pairs()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.into_owned())
}

/// `buildYouTubeEmbedUrl`: `clip`, `clipt` and the `t` / `start` offset are
/// carried over, serialised like `URLSearchParams`.
fn build_embed_url(video_id: &str, url: &Url) -> String {
    let mut params = url::form_urlencoded::Serializer::new(String::new());
    if let Some(clip) = query_param(url, "clip") {
        params.append_pair("clip", &clip);
    }
    if let Some(clipt) = query_param(url, "clipt") {
        params.append_pair("clipt", &clipt);
    }
    let start = normalize_time(query_param(url, "t"))
        .filter(|start| !start.is_empty())
        .or_else(|| normalize_time(query_param(url, "start")).filter(|start| !start.is_empty()));
    if let Some(start) = start {
        params.append_pair("start", &start);
    }
    let query = params.finish();
    if query.is_empty() {
        format!("https://www.youtube.com/embed/{video_id}")
    } else {
        format!("https://www.youtube.com/embed/{video_id}?{query}")
    }
}

/// `parseYouTubeUrl`: the embed URL for a watch / short / embed / `youtu.be`
/// link; clip links resolve separately.
pub fn parse_youtube_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if parse_youtube_clip_id(trimmed).is_some() {
        return None;
    }
    let parsed = Url::parse(trimmed).ok()?;
    let hostname = parsed.host_str()?.to_ascii_lowercase();
    let hostname = hostname.strip_prefix("www.").unwrap_or(&hostname);
    let parts: Vec<&str> = parsed
        .path_segments()
        .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
        .unwrap_or_default();
    let video_id = match hostname {
        "youtu.be" => parts.first().copied().unwrap_or_default().to_string(),
        "youtube.com" | "m.youtube.com" | "youtube-nocookie.com" => match parts.first().copied() {
            Some("watch") => query_param(&parsed, "v").unwrap_or_default(),
            Some("embed" | "shorts") => parts.get(1).copied().unwrap_or_default().to_string(),
            _ => String::new(),
        },
        _ => String::new(),
    };
    if video_id.is_empty() {
        return None;
    }
    Some(build_embed_url(&video_id, &parsed))
}

/// `parseYouTubeEmbedSnippet`: a `<Clip src>` / `<iframe src>` snippet whose
/// source is a YouTube link.
pub fn parse_youtube_embed_snippet(snippet: &str) -> Option<String> {
    let trimmed = snippet.trim();
    if trimmed.is_empty() {
        return None;
    }
    for pattern in [&*CLIP_TAG, &*IFRAME_TAG] {
        if let Some(captures) = pattern.captures(trimmed)
            && let Some(embed) = parse_youtube_url(&captures[1])
        {
            return Some(embed);
        }
    }
    if !IFRAME_START.is_match(trimmed) {
        return None;
    }
    SRC_ATTR
        .captures(trimmed)
        .and_then(|captures| parse_youtube_url(&captures[1]))
}

/// `resolveYouTubeClipUrl`: the clip page names its video.
pub async fn resolve_youtube_clip_url(clip_id: String) -> Option<String> {
    let html = reqwest::get(format!("https://www.youtube.com/clip/{clip_id}"))
        .await
        .ok()?
        .text()
        .await
        .ok()?;
    VIDEO_ID
        .captures(&html)
        .map(|captures| format!("https://www.youtube.com/embed/{}", &captures[1]))
}

/// The `clip` node with its `src` attribute.
pub fn clip_node(embed_url: &str) -> serde_json::Value {
    serde_json::json!({ "type": "clip", "attrs": { "src": embed_url } })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watch_short_and_embed_links_resolve_to_embed_urls() {
        assert_eq!(
            parse_youtube_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ").as_deref(),
            Some("https://www.youtube.com/embed/dQw4w9WgXcQ")
        );
        assert_eq!(
            parse_youtube_url(" https://youtu.be/dQw4w9WgXcQ?t=90s ").as_deref(),
            Some("https://www.youtube.com/embed/dQw4w9WgXcQ?start=90")
        );
        assert_eq!(
            parse_youtube_url("https://m.youtube.com/shorts/abc_-123?start=5").as_deref(),
            Some("https://www.youtube.com/embed/abc_-123?start=5")
        );
        assert_eq!(
            parse_youtube_url("https://www.youtube-nocookie.com/embed/xyz?clip=Ugk&clipt=EJ_&t=10")
                .as_deref(),
            Some("https://www.youtube.com/embed/xyz?clip=Ugk&clipt=EJ_&start=10")
        );
    }

    #[test]
    fn other_links_and_clip_links_are_not_embeds() {
        assert_eq!(parse_youtube_url("https://vimeo.com/123"), None);
        assert_eq!(parse_youtube_url("youtube.com/watch?v=abc"), None);
        assert_eq!(parse_youtube_url("https://www.youtube.com/"), None);
        assert_eq!(parse_youtube_url("https://www.youtube.com/watch"), None);
        assert_eq!(parse_youtube_url("https://youtube.com/clip/UgkxAbc"), None);
        assert_eq!(
            parse_youtube_clip_id("https://youtube.com/clip/UgkxAbc-_1?si=2").as_deref(),
            Some("UgkxAbc-_1")
        );
        assert_eq!(parse_youtube_clip_id("https://youtube.com/watch?v=a"), None);
    }

    #[test]
    fn embed_snippets_take_their_source() {
        assert_eq!(
            parse_youtube_embed_snippet(
                r#"<iframe width="560" src="https://www.youtube.com/embed/abc?start=3" title="x"></iframe>"#
            )
            .as_deref(),
            Some("https://www.youtube.com/embed/abc?start=3")
        );
        assert_eq!(
            parse_youtube_embed_snippet(r#"<Clip src='https://youtu.be/abc' />"#).as_deref(),
            Some("https://www.youtube.com/embed/abc")
        );
        assert_eq!(
            parse_youtube_embed_snippet(r#"<IFRAME src="https://youtu.be/abc" allow="x">"#)
                .as_deref(),
            Some("https://www.youtube.com/embed/abc")
        );
        assert_eq!(
            parse_youtube_embed_snippet(r#"<iframe src="https://vimeo.com/1"></iframe>"#),
            None
        );
        assert_eq!(parse_youtube_embed_snippet("<div>hi</div>"), None);
        assert_eq!(parse_youtube_embed_snippet(""), None);
    }

    #[test]
    fn clip_node_shape() {
        assert_eq!(
            clip_node("https://www.youtube.com/embed/abc").to_string(),
            r#"{"type":"clip","attrs":{"src":"https://www.youtube.com/embed/abc"}}"#
        );
    }
}
