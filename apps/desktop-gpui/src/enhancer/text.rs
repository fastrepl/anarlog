//! Text helpers around a generated summary: the `•` bullet normaliser from
//! `shared/transform_impl.ts`, `summary-tags.ts`, `title-success.ts`'s
//! `getPersistableGeneratedTitle`, `title-content.ts`'s markdown title, and
//! the `hasSummaryContent` / `shouldHydrateTemplateTitle` checks from the
//! enhancer service.

/// `normalizeBulletPoints`: a streaming transform that turns `• ` at a line
/// start into `- `, carrying the line state across chunks.
#[derive(Debug, Default, Clone)]
pub struct BulletNormalizer {
    mid_line: bool,
}

impl BulletNormalizer {
    pub fn push(&mut self, chunk: &str) -> String {
        let mut out = String::with_capacity(chunk.len());
        let chars: Vec<char> = chunk.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '\n' {
                out.push(c);
                self.mid_line = false;
                i += 1;
                continue;
            }
            if !self.mid_line {
                if c == ' ' || c == '\t' {
                    out.push(c);
                    i += 1;
                    continue;
                }
                if c == '•' && chars.get(i + 1) == Some(&' ') {
                    out.push('-');
                    self.mid_line = true;
                    i += 1;
                    continue;
                }
                self.mid_line = true;
            }
            out.push(c);
            i += 1;
        }
        out
    }
}

fn is_tag_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_tag_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

/// `TAG_NAME_RE`: `^[\p{L}_][\p{L}\p{N}_-]*$`.
fn is_tag_name(value: &str) -> bool {
    let mut chars = value.chars();
    chars.next().is_some_and(is_tag_start) && chars.all(is_tag_char)
}

/// `HASHTAG_RE`: `#name` not preceded by a letter, digit, `_`, `/` or `#`.
fn hashtags(source: &str) -> Vec<String> {
    let chars: Vec<char> = source.chars().collect();
    let mut tags = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '#' {
            let preceded_ok = i == 0
                || !(chars[i - 1].is_alphanumeric() || matches!(chars[i - 1], '_' | '/' | '#'));
            if preceded_ok && chars.get(i + 1).copied().is_some_and(is_tag_start) {
                let mut end = i + 1;
                while end < chars.len() && is_tag_char(chars[end]) {
                    end += 1;
                }
                tags.push(chars[i + 1..end].iter().collect());
                i = end;
                continue;
            }
        }
        i += 1;
    }
    tags
}

fn normalize_tag_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    for raw in names {
        let name = raw.trim_start_matches('#').trim().to_lowercase();
        if !is_tag_name(&name) || result.contains(&name) {
            continue;
        }
        result.push(name);
    }
    result
}

/// `extractEnhanceTagNames`: hashtags from the summary and its inputs.
pub fn extract_tag_names<'a>(sources: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let found: Vec<String> = sources.into_iter().flat_map(hashtags).collect();
    normalize_tag_names(found.iter().map(String::as_str))
}

fn is_tag_only_line(line: &str) -> bool {
    let mut tokens = line.split_whitespace().peekable();
    tokens.peek().is_some() && tokens.all(|token| token.strip_prefix('#').is_some_and(is_tag_name))
}

fn strip_trailing_tag_lines(markdown: &str) -> String {
    let lines: Vec<&str> = markdown
        .split(['\n'])
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    let mut end = lines.len();
    while end > 0 && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    while end > 0 && is_tag_only_line(lines[end - 1]) {
        end -= 1;
        while end > 0 && lines[end - 1].trim().is_empty() {
            end -= 1;
        }
    }
    lines[..end].join("\n")
}

/// `appendTagLineToMarkdown`.
pub fn append_tag_line(markdown: &str, tag_names: &[String]) -> String {
    let normalized = normalize_tag_names(tag_names.iter().map(String::as_str));
    if normalized.is_empty() {
        return markdown.to_string();
    }
    let body = strip_trailing_tag_lines(markdown).trim_end().to_string();
    let tag_line = normalized
        .iter()
        .map(|name| format!("#{name}"))
        .collect::<Vec<_>>()
        .join(" ");
    if body.is_empty() {
        tag_line
    } else {
        format!("{body}\n\n{tag_line}")
    }
}

const GENERATED_TITLE_MAX_LENGTH: usize = 160;

/// `getPersistableGeneratedTitle`: the last non-empty line, unwrapped from
/// `Title:` prefixes, list markers, and quote / emphasis wrappers.
pub fn persistable_generated_title(text: &str) -> String {
    let last_line = text
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .unwrap_or("");
    let mut title = last_line.split_whitespace().collect::<Vec<_>>().join(" ");
    while !title.is_empty() {
        let normalized = strip_title_prefixes(&title);
        let unwrapped = unwrap_title(normalized).trim().to_string();
        if unwrapped == title {
            break;
        }
        title = unwrapped;
    }
    if title.is_empty() || title == "<EMPTY>" || title.chars().count() > GENERATED_TITLE_MAX_LENGTH
    {
        return String::new();
    }
    title
}

/// `^(?:(?:final\s+)?title|final answer)\s*:\s*` then `^(?:\d+[.)]|[-*]|#+)\s+`.
fn strip_title_prefixes(title: &str) -> &str {
    let lower = title.to_lowercase();
    let mut rest = title;
    for prefix in ["final title", "final answer", "title"] {
        if let Some(after) = lower.strip_prefix(prefix) {
            // `final\s+title` allows any whitespace run; match the common form.
            let after_trim = after.trim_start();
            if let Some(after_colon) = after_trim.strip_prefix(':') {
                let skipped = title.len() - after_colon.len();
                rest = title[skipped..].trim_start();
                break;
            }
        }
    }
    let trimmed = rest;
    let marker_len = {
        let digits = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && trimmed[digits..].starts_with(['.', ')']) {
            digits + 1
        } else if trimmed.starts_with(['-', '*']) {
            1
        } else {
            trimmed.chars().take_while(|c| *c == '#').count()
        }
    };
    if marker_len > 0 && trimmed[marker_len..].starts_with(char::is_whitespace) {
        return trimmed[marker_len..].trim_start();
    }
    trimmed.trim()
}

/// `^(\*\*|__|["'`])(.*)\1$`.
fn unwrap_title(title: &str) -> &str {
    for wrapper in ["**", "__", "\"", "'", "`"] {
        if title.len() >= wrapper.len() * 2
            && title.starts_with(wrapper)
            && title.ends_with(wrapper)
        {
            return &title[wrapper.len()..title.len() - wrapper.len()];
        }
    }
    title
}

/// `ensureMarkdownFirstLineTitle`.
pub fn ensure_markdown_first_line_title(markdown: &str, title: Option<&str>) -> String {
    let Some(title) = title.map(str::trim).filter(|title| !title.is_empty()) else {
        return markdown.to_string();
    };
    let trimmed = markdown.trim_start();
    let first_line = trimmed.split('\n').next().unwrap_or("");
    if first_line == format!("# {title}") {
        return markdown.to_string();
    }
    format!("# {title}\n\n{trimmed}").trim().to_string()
}

const TEXT_CONTAINER_TYPES: [&str; 9] = [
    "doc",
    "heading",
    "paragraph",
    "text",
    "codeBlock",
    "blockquote",
    "bulletList",
    "orderedList",
    "listItem",
];

fn has_meaningful_content(node: &serde_json::Value) -> bool {
    if node
        .get("text")
        .and_then(|t| t.as_str())
        .is_some_and(|t| !t.trim().is_empty())
    {
        return true;
    }
    match node.get("type").and_then(|t| t.as_str()) {
        Some(kind) if TEXT_CONTAINER_TYPES.contains(&kind) => node
            .get("content")
            .and_then(|c| c.as_array())
            .is_some_and(|children| children.iter().any(has_meaningful_content)),
        // Images, mentions, and other atoms count as content.
        _ => true,
    }
}

fn collect_text(node: &serde_json::Value) -> String {
    let mut text = node
        .get("text")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    if let Some(children) = node.get("content").and_then(|c| c.as_array()) {
        for child in children {
            text.push_str(&collect_text(child));
        }
    }
    text
}

/// `hasSummaryContent`: a stored body has content beyond a synthesized
/// `# <session title>` heading.
pub fn has_summary_content(body: &str, session_title: Option<&str>) -> bool {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return false;
    }
    if !trimmed.starts_with('{') {
        return true;
    }
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return true;
    };
    if parsed.get("type").and_then(|t| t.as_str()) != Some("doc") {
        return true;
    }
    let blocks: Vec<serde_json::Value> = parsed
        .get("content")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    let session_title = session_title.map(str::trim).filter(|t| !t.is_empty());
    let synthesized_title = match (blocks.first(), session_title) {
        (Some(first), Some(title)) => {
            let attrs = first.get("attrs").and_then(|a| a.as_object());
            first.get("type").and_then(|t| t.as_str()) == Some("heading")
                && attrs.is_some_and(|attrs| {
                    attrs.len() == 1 && attrs.get("level").and_then(|l| l.as_u64()) == Some(1)
                })
                && collect_text(first).trim() == title
                && first
                    .get("content")
                    .and_then(|c| c.as_array())
                    .is_none_or(|children| {
                        children.iter().all(|child| {
                            child.get("type").and_then(|t| t.as_str()) == Some("text")
                                && child
                                    .get("text")
                                    .and_then(|t| t.as_str())
                                    .is_some_and(|t| !t.trim().is_empty())
                                && child
                                    .get("marks")
                                    .and_then(|m| m.as_array())
                                    .is_none_or(|marks| marks.is_empty())
                        })
                    })
        }
        _ => false,
    };
    let rest = if synthesized_title {
        &blocks[1..]
    } else {
        &blocks[..]
    };
    rest.iter().any(has_meaningful_content)
}

fn is_uuid_like(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && value.chars().enumerate().all(|(index, c)| match index {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

fn is_iso_like(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 16
        && value
            .chars()
            .enumerate()
            .take(16)
            .all(|(index, c)| match index {
                4 | 7 => c == '-',
                10 => c == 'T',
                13 => c == ':',
                _ => c.is_ascii_digit(),
            })
}

/// `shouldHydrateTemplateTitle`: placeholder titles get the template's.
pub fn should_hydrate_template_title(current_title: Option<&str>, template_id: &str) -> bool {
    let Some(title) = current_title.map(str::trim).filter(|t| !t.is_empty()) else {
        return true;
    };
    title == "Summary" || title == template_id || is_uuid_like(title) || is_iso_like(title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bullets_normalize_across_chunks() {
        let mut normalizer = BulletNormalizer::default();
        let mut out = normalizer.push("• First\n  ");
        out.push_str(&normalizer.push("• Second\nText • not a bullet"));
        assert_eq!(out, "- First\n  - Second\nText • not a bullet");
    }

    #[test]
    fn tags_are_extracted_and_appended_once() {
        let tags = extract_tag_names([
            "# Summary\n\nTalked about #Launch and #q3-plan, url/#anchor, ##double",
            "memo with #launch again",
        ]);
        assert_eq!(tags, vec!["launch", "q3-plan"]);
        assert_eq!(
            append_tag_line("# Summary\n\n- a\n\n#old #tags\n", &tags),
            "# Summary\n\n- a\n\n#launch #q3-plan"
        );
        assert_eq!(append_tag_line("body", &[]), "body");
        assert_eq!(append_tag_line("", &["x".to_string()]), "#x");
    }

    #[test]
    fn generated_titles_unwrap_decorations() {
        assert_eq!(
            persistable_generated_title("Thinking...\n\nTitle: \"**Weekly Sync**\""),
            "Weekly Sync"
        );
        assert_eq!(
            persistable_generated_title("1. Roadmap review"),
            "Roadmap review"
        );
        assert_eq!(persistable_generated_title("# Heading"), "Heading");
        assert_eq!(persistable_generated_title("<EMPTY>"), "");
        assert_eq!(persistable_generated_title(&"x".repeat(161)), "");
        assert_eq!(persistable_generated_title(""), "");
    }

    #[test]
    fn markdown_title_is_prepended_once() {
        assert_eq!(
            ensure_markdown_first_line_title("# Sync\n\nbody", Some("Sync")),
            "# Sync\n\nbody"
        );
        assert_eq!(
            ensure_markdown_first_line_title("# Other\n\nbody", Some("Sync")),
            "# Sync\n\n# Other\n\nbody"
        );
        assert_eq!(ensure_markdown_first_line_title("body", Some("  ")), "body");
    }

    #[test]
    fn summary_content_ignores_a_synthesized_title() {
        let only_title = r#"{"type":"doc","content":[{"type":"heading","attrs":{"level":1},"content":[{"type":"text","text":"Sync"}]}]}"#;
        assert!(!has_summary_content(only_title, Some("Sync")));
        assert!(has_summary_content(only_title, Some("Other")));
        let with_body = r#"{"type":"doc","content":[{"type":"heading","attrs":{"level":1},"content":[{"type":"text","text":"Sync"}]},{"type":"paragraph","content":[{"type":"text","text":"hello"}]}]}"#;
        assert!(has_summary_content(with_body, Some("Sync")));
        assert!(!has_summary_content("   ", None));
        assert!(has_summary_content("# markdown", None));
        assert!(!has_summary_content(
            r#"{"type":"doc","content":[{"type":"paragraph"}]}"#,
            None
        ));
    }

    #[test]
    fn template_titles_replace_placeholders_only() {
        assert!(should_hydrate_template_title(None, "tpl"));
        assert!(should_hydrate_template_title(Some("Summary"), "tpl"));
        assert!(should_hydrate_template_title(Some("tpl"), "tpl"));
        assert!(should_hydrate_template_title(
            Some("40bc9d36-7634-4c48-988f-6a3e301467e7"),
            "tpl"
        ));
        assert!(should_hydrate_template_title(
            Some("2026-09-07T03:00"),
            "tpl"
        ));
        assert!(!should_hydrate_template_title(Some("My notes"), "tpl"));
    }
}
