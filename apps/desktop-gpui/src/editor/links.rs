//! `packages/editor/src/plugins/{url,autolink,link-boundary-guard}.ts`: the
//! link marks the editor maintains on its own after every change. Autolink
//! marks URL-looking text (with the `tlds` list deciding what a domain is);
//! the boundary guard keeps a link's `href` in step with its text, extends a
//! link over path text typed right after it, and drops links whose text
//! stopped being a URL.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Value, json};

/// The `tlds` package's list, one per line.
static TLDS: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| include_str!("tlds.txt").lines().collect());

static HTTP_SCHEME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^https?://").unwrap());
static DOMAIN_TEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^(?:www\.)?(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}(?::\d{1,5})?(?:[/?#].*)?$",
    )
    .unwrap()
});
static URL_CANDIDATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)(^|[^\p{L}\p{N}_@.-])((?:https?://|www\.)[^\s<>"'`]+|(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}(?::\d{1,5})?(?:[/?#][^\s<>"'`]*)?)"#,
    )
    .unwrap()
});
static STANDALONE_TRAILING_PUNCTUATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[.,!?;:)\]}]+$").unwrap());

pub fn normalize_url_href(text: &str) -> String {
    let trimmed = text.trim();
    if HTTP_SCHEME.is_match(trimmed) {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    }
}

pub fn looks_like_url_text(text: &str) -> bool {
    let trimmed = text.trim();
    HTTP_SCHEME.is_match(trimmed) || DOMAIN_TEXT.is_match(trimmed)
}

/// `isValidUrl`: parses like the WHATWG `URL` the web view uses, then requires
/// an http(s) scheme and a known top-level domain.
pub fn is_valid_url(text: &str) -> bool {
    if !looks_like_url_text(text) {
        return false;
    }
    let Ok(parsed) = url::Url::parse(&normalize_url_href(text)) else {
        return false;
    };
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return false;
    }
    let Some(host) = parsed.host_str() else {
        return false;
    };
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() < 2 {
        return false;
    }
    TLDS.contains(parts[parts.len() - 1].to_ascii_lowercase().as_str())
}

pub fn is_link_text_for_href(text: &str, href: Option<&str>) -> bool {
    href.is_some_and(|href| text == href || normalize_url_href(text) == href)
}

const TRAILING_PUNCTUATION: [char; 6] = ['.', ',', '!', '?', ';', ':'];

fn opening_for(closing: char) -> Option<char> {
    match closing {
        ')' => Some('('),
        ']' => Some('['),
        '}' => Some('{'),
        _ => None,
    }
}

/// Sentence punctuation and unbalanced closers after a URL are not part of it.
fn trim_trailing_punctuation(text: &str) -> &str {
    let mut end = text.len();
    while end > 0 {
        let Some(last) = text[..end].chars().next_back() else {
            break;
        };
        if TRAILING_PUNCTUATION.contains(&last) {
            end -= last.len_utf8();
            continue;
        }
        if let Some(opener) = opening_for(last) {
            let head = &text[..end];
            if head.matches(last).count() > head.matches(opener).count() {
                end -= last.len_utf8();
                continue;
            }
        }
        break;
    }
    &text[..end]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutolinkMatch {
    /// Byte range within the text.
    pub start: usize,
    pub end: usize,
    pub href: String,
}

/// `findAutolinkMatches`
pub fn find_autolink_matches(text: &str) -> Vec<AutolinkMatch> {
    let mut matches = Vec::new();
    for captures in URL_CANDIDATE.captures_iter(text) {
        let Some(candidate) = captures.get(2) else {
            continue;
        };
        let trimmed = trim_trailing_punctuation(candidate.as_str());
        if trimmed.is_empty() || !is_valid_url(trimmed) {
            continue;
        }
        matches.push(AutolinkMatch {
            start: candidate.start(),
            end: candidate.start() + trimmed.len(),
            href: normalize_url_href(trimmed),
        });
    }
    matches
}

/// The `link` mark as TipTap serialises it: `{ href, target }`.
pub fn link_mark(href: &str, target: Option<&Value>) -> Value {
    json!({
        "type": "link",
        "attrs": { "href": href, "target": target.cloned().unwrap_or(Value::Null) }
    })
}

pub fn link_mark_of(node: &Value) -> Option<&Value> {
    node.get("marks")?
        .as_array()?
        .iter()
        .find(|mark| mark.get("type").and_then(Value::as_str) == Some("link"))
}

pub fn link_href(node: &Value) -> Option<&str> {
    link_mark_of(node)?.get("attrs")?.get("href")?.as_str()
}

/// `hasChangedRange`: a zero-width change touches the nodes around it; a
/// replaced range touches the nodes it overlaps.
pub fn range_changed(changed: &std::ops::Range<usize>, from: usize, to: usize) -> bool {
    if changed.start == changed.end {
        from <= changed.start && changed.start <= to
    } else {
        changed.start < to && from < changed.end
    }
}

/// One edit to apply to a textblock's inline content, in block byte offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkEdit {
    Remove { from: usize, to: usize },
    Set { from: usize, to: usize, mark: Value },
}

/// The boundary guard (`linkBoundaryGuardPlugin`) over one textblock's text
/// nodes, given the block-relative range the last change touched. Returns
/// the edits in the order ProseMirror would apply them.
pub fn boundary_guard_edits(
    inline: &[(usize, &Value)],
    changed: &std::ops::Range<usize>,
) -> Vec<LinkEdit> {
    struct PrevLink {
        start: usize,
        end: usize,
        mark: Value,
        href: String,
    }
    let mut edits = Vec::new();
    let mut prev: Option<PrevLink> = None;
    for (pos, node) in inline {
        let pos = *pos;
        let Some(text) = node.get("text").and_then(Value::as_str).filter(|text| {
            node.get("type").and_then(Value::as_str) == Some("text") && !text.is_empty()
        }) else {
            prev = None;
            continue;
        };
        let end = pos + text.len();
        if let Some(mark) = link_mark_of(node) {
            let looks_like_url = looks_like_url_text(text);
            let Some(href) = link_href(node) else {
                prev = None;
                continue;
            };
            let text_changed = range_changed(changed, pos, end);
            if looks_like_url && !is_valid_url(text) && text_changed {
                edits.push(LinkEdit::Remove { from: pos, to: end });
                prev = None;
            } else if is_link_text_for_href(text, Some(href)) {
                prev = Some(PrevLink {
                    start: pos,
                    end,
                    mark: mark.clone(),
                    href: href.to_string(),
                });
            } else if looks_like_url && text_changed {
                let next_href = normalize_url_href(text);
                let mut next_mark = mark.clone();
                next_mark["attrs"]["href"] = Value::String(next_href.clone());
                edits.push(LinkEdit::Remove { from: pos, to: end });
                edits.push(LinkEdit::Set {
                    from: pos,
                    to: end,
                    mark: next_mark.clone(),
                });
                prev = Some(PrevLink {
                    start: pos,
                    end,
                    mark: next_mark,
                    href: next_href,
                });
            } else if looks_like_url {
                prev = Some(PrevLink {
                    start: pos,
                    end,
                    mark: mark.clone(),
                    href: href.to_string(),
                });
            } else {
                prev = None;
            }
        } else if let Some(link) = prev.take().filter(|link| link.end == pos) {
            if !text.starts_with(char::is_whitespace) {
                let extend_len = text.find(char::is_whitespace).unwrap_or(text.len());
                let extension = &text[..extend_len];
                let new_href = format!("{}{extension}", link.href);
                if !STANDALONE_TRAILING_PUNCTUATION.is_match(extension) && is_valid_url(&new_href) {
                    let mut mark = link.mark.clone();
                    mark["attrs"]["href"] = Value::String(new_href);
                    edits.push(LinkEdit::Remove {
                        from: link.start,
                        to: link.end,
                    });
                    edits.push(LinkEdit::Set {
                        from: link.start,
                        to: pos + extend_len,
                        mark,
                    });
                }
            }
        } else {
            prev = None;
        }
    }
    edits
}

/// `autolinkPlugin` over one textblock: link every URL match in text nodes
/// that are not already linked (code blocks are skipped by the caller).
pub fn autolink_edits(
    inline: &[(usize, &Value)],
    has_link: impl Fn(usize, usize) -> bool,
) -> Vec<LinkEdit> {
    let mut edits = Vec::new();
    for (pos, node) in inline {
        if node.get("type").and_then(Value::as_str) != Some("text") {
            continue;
        }
        let Some(text) = node.get("text").and_then(Value::as_str) else {
            continue;
        };
        for found in find_autolink_matches(text) {
            let (from, to) = (pos + found.start, pos + found.end);
            if has_link(from, to) {
                continue;
            }
            edits.push(LinkEdit::Set {
                from,
                to,
                mark: link_mark(&found.href, None),
            });
        }
    }
    edits
}

/// `getOpenableHref`: only http(s) links open from a click.
pub fn openable_href(href: &str) -> Option<String> {
    let parsed = url::Url::parse(href).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| parsed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_urls_like_the_web_editor() {
        assert!(is_valid_url("x.com"));
        assert!(is_valid_url("https://x.com/docs?ref=note"));
        assert!(is_valid_url("www.example.org:8080/a"));
        assert!(!is_valid_url("x.invalidtld"));
        assert!(!is_valid_url("localhost"));
        assert!(!is_valid_url("ftp://x.com"));
        assert_eq!(normalize_url_href(" x.com "), "https://x.com");
        assert_eq!(normalize_url_href("http://x.com"), "http://x.com");
    }

    #[test]
    fn finds_bare_domains_and_paths_without_sentence_punctuation() {
        assert_eq!(
            find_autolink_matches("x.com"),
            vec![AutolinkMatch {
                start: 0,
                end: 5,
                href: "https://x.com".into()
            }]
        );
        let text = "See linear.app/fastrepl-inc/initiative/product-45dff51a8672/overview.";
        let found = find_autolink_matches(text);
        assert_eq!(found.len(), 1);
        assert_eq!(
            &text[found[0].start..found[0].end],
            "linear.app/fastrepl-inc/initiative/product-45dff51a8672/overview"
        );
        assert_eq!(
            found[0].href,
            "https://linear.app/fastrepl-inc/initiative/product-45dff51a8672/overview"
        );
        // Balanced closers stay, unbalanced ones go.
        let text = "(see en.wikipedia.org/wiki/Foo_(bar))";
        let found = find_autolink_matches(text);
        assert_eq!(
            &text[found[0].start..found[0].end],
            "en.wikipedia.org/wiki/Foo_(bar)"
        );
    }

    #[test]
    fn does_not_link_email_domains() {
        assert!(find_autolink_matches("email support@x.com").is_empty());
    }

    #[test]
    fn openable_hrefs_are_http_only() {
        assert_eq!(
            openable_href("https://x.com"),
            Some("https://x.com/".into())
        );
        assert_eq!(openable_href("mailto:a@b.co"), None);
        assert_eq!(openable_href("not a url"), None);
    }
}
