//! `DOMParser.parseSlice` (`prosemirror-model/src/from_dom.ts`) over the
//! note schema's `parseDOM` rules, reading an html5ever tree the way the
//! browser's parser hands one to the web view.

use html5ever::namespace_url;
use kuchikiki::traits::TendrilSink as _;
use kuchikiki::{NodeRef, Selectors};
use serde_json::{Value, json};

use super::content::Match;
use super::node::{Attrs, Fragment, Mark, Node};
use super::resolved::ResolvedPos;
use super::schema::{MarkId, Schema, TypeId, schema};

/// `preserveWhitespace`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreserveWs {
    Collapse,
    Preserve,
    Full,
}

enum Target {
    Node(TypeId),
    Mark(MarkId),
    /// `clearMark`: drops the marks of this type from the set.
    ClearMark(MarkId),
}

enum Selector {
    Tag(Selectors),
    /// `style: "prop"` or `style: "prop=value"`.
    Style(&'static str, Option<&'static str>),
}

/// `getAttrs`' outcome: `None` is `false` (the rule does not apply).
type Outcome = Option<Option<Attrs>>;

struct Rule {
    selector: Selector,
    target: Target,
    get_attrs: Option<fn(&NodeRef) -> Outcome>,
    get_style_attrs: Option<fn(&str) -> Outcome>,
    preserve_whitespace: Option<PreserveWs>,
}

pub struct Parser {
    schema: &'static Schema,
    rules: Vec<Rule>,
    /// The style properties the rules read, in rule order.
    matched_styles: Vec<&'static str>,
    normalize_lists: bool,
}

const BLOCK_TAGS: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "canvas",
    "dd",
    "div",
    "dl",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hgroup",
    "hr",
    "li",
    "noscript",
    "ol",
    "output",
    "p",
    "pre",
    "section",
    "table",
    "tfoot",
    "ul",
];
const IGNORE_TAGS: &[&str] = &["head", "noscript", "object", "script", "style", "title"];
const LIST_TAGS: &[&str] = &["ol", "ul"];

const OPT_PRESERVE_WS: u8 = 1;
const OPT_PRESERVE_WS_FULL: u8 = 2;
const OPT_OPEN_LEFT: u8 = 4;

fn tag_name(node: &NodeRef) -> Option<String> {
    node.as_element()
        .map(|element| element.name.local.to_string().to_lowercase())
}

fn attr(node: &NodeRef, name: &str) -> Option<String> {
    node.as_element()
        .and_then(|element| element.attributes.borrow().get(name).map(str::to_string))
}

/// `element.style`: the inline declarations, the last one per property
/// winning, keyword values lower-cased, `!important` dropped; the
/// `text-decoration` shorthand also reads its `-line` longhand the way the
/// browser's `getPropertyValue` serialises it.
pub struct Style {
    declarations: Vec<(String, String)>,
}

impl Style {
    pub fn of(node: &NodeRef) -> Style {
        let declarations = attr(node, "style")
            .map(|style| {
                style
                    .split(';')
                    .filter_map(|declaration| {
                        let (name, value) = declaration.split_once(':')?;
                        let name = name.trim().to_lowercase();
                        let value = value
                            .trim()
                            .trim_end_matches("!important")
                            .trim()
                            .to_lowercase();
                        (!name.is_empty() && !value.is_empty()).then_some((name, value))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Style { declarations }
    }

    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }

    pub fn get(&self, property: &str) -> Option<&str> {
        let direct = self
            .declarations
            .iter()
            .rev()
            .find(|(name, _)| name == property)
            .map(|(_, value)| value.as_str());
        if direct.is_some() || property != "text-decoration" {
            return direct;
        }
        self.declarations
            .iter()
            .rev()
            .find(|(name, _)| name == "text-decoration-line")
            .map(|(_, value)| value.as_str())
    }
}

fn selector(css: &str) -> Selector {
    Selector::Tag(Selectors::compile(css).expect("a valid parse rule selector"))
}

fn no_attrs(_: &NodeRef) -> Outcome {
    Some(None)
}

fn attrs_of(pairs: Vec<(&str, Value)>) -> Outcome {
    Some(Some(
        pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
    ))
}

fn string_or_null(node: &NodeRef, name: &str) -> Value {
    attr(node, name).map(Value::String).unwrap_or(Value::Null)
}

/// A JavaScript number as JSON: integral values without a fraction, `NaN`
/// as `null`.
fn js_number(value: f64) -> Value {
    if !value.is_finite() {
        Value::Null
    } else if value.fract() == 0.0 && value.abs() < 9007199254740992.0 {
        Value::from(value as i64)
    } else {
        json!(value)
    }
}

/// `Number(string)`: the whole string as a number, `""` as 0.
fn js_number_of(value: &str) -> f64 {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return 0.0;
    }
    trimmed.parse::<f64>().unwrap_or(f64::NAN)
}

/// `parseInt(string, 10)`: the leading integer, `NaN` without one.
fn js_parse_int(value: &str) -> f64 {
    let trimmed = value.trim_start();
    let (sign, digits) = match trimmed.strip_prefix('-') {
        Some(rest) => (-1.0, rest),
        None => (1.0, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };
    let digits: String = digits.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        f64::NAN
    } else {
        sign * digits.parse::<f64>().unwrap_or(f64::NAN)
    }
}

fn heading_rule(level: u8) -> Rule {
    let get_attrs: fn(&NodeRef) -> Outcome = match level {
        1 => |_| attrs_of(vec![("level", json!(1))]),
        2 => |_| attrs_of(vec![("level", json!(2))]),
        3 => |_| attrs_of(vec![("level", json!(3))]),
        4 => |_| attrs_of(vec![("level", json!(4))]),
        5 => |_| attrs_of(vec![("level", json!(5))]),
        _ => |_| attrs_of(vec![("level", json!(6))]),
    };
    Rule {
        selector: selector(&format!("h{level}")),
        target: Target::Node(schema().node("heading").unwrap()),
        get_attrs: Some(get_attrs),
        get_style_attrs: None,
        preserve_whitespace: None,
    }
}

fn table_cell_attrs(node: &NodeRef) -> Outcome {
    // `Number(el.getAttribute("colspan") || 1)`
    let number_or_one = |name: &str| match attr(node, name) {
        Some(value) if !value.is_empty() => js_number_of(&value),
        _ => 1.0,
    };
    let colspan = number_or_one("colspan");
    let rowspan = number_or_one("rowspan");
    let widths = attr(node, "data-colwidth")
        .filter(|value| {
            !value.is_empty()
                && value
                    .split(',')
                    .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
        })
        .map(|value| {
            value
                .split(',')
                .map(|part| json!(part.parse::<i64>().unwrap()))
                .collect::<Vec<_>>()
        })
        .filter(|widths| widths.len() as f64 == colspan);
    attrs_of(vec![
        ("colspan", js_number(colspan)),
        ("rowspan", js_number(rowspan)),
        ("colwidth", widths.map(Value::Array).unwrap_or(Value::Null)),
    ])
}

/// `normalizeTaskStatus` / `createTaskStatusAttrs`.
fn task_item_attrs(node: &NodeRef) -> Outcome {
    let status = attr(node, "data-status");
    let checked = attr(node, "data-checked").as_deref() == Some("true");
    let status = match status.as_deref() {
        Some(status @ ("todo" | "in_progress" | "done")) => status.to_string(),
        _ if checked => "done".to_string(),
        _ => "todo".to_string(),
    };
    attrs_of(vec![
        ("checked", json!(status == "done")),
        ("status", json!(status)),
        ("taskId", string_or_null(node, "data-task-id")),
        ("taskItemId", string_or_null(node, "data-task-item-id")),
    ])
}

/// `imageNodeSpec.parseDOM[0].getAttrs` with `parseImageMetadata`.
fn image_attrs(node: &NodeRef) -> Outcome {
    let title = attr(node, "title");
    let (metadata_width, metadata_title) = match title.as_deref() {
        Some(title) => {
            let rest = title.strip_prefix("char-editor-width=");
            let digits = rest
                .map(|rest| rest.chars().take_while(char::is_ascii_digit).count())
                .unwrap_or(0);
            match rest {
                Some(rest) if (1..=3).contains(&digits) => {
                    let tail = &rest[digits..];
                    let width = Some(crate::document::clamp_image_width(
                        rest[..digits].parse::<f64>().ok(),
                    ) as f64);
                    match tail {
                        "" => (width, None),
                        _ if tail.starts_with('|') => (width, Some(tail[1..].to_string())),
                        _ => (None, Some(title.to_string())),
                    }
                }
                _ => (None, Some(title.to_string())),
            }
        }
        None => (None, None),
    };
    // `parseInt(el.getAttribute("data-editor-width") ?? String(metadata.editorWidth), 10)`
    let width = match attr(node, "data-editor-width") {
        Some(value) => Some(js_parse_int(&value)),
        None => metadata_width,
    };
    let mut pairs = vec![
        ("src", string_or_null(node, "src")),
        ("alt", string_or_null(node, "alt")),
    ];
    if let Some(title) = metadata_title {
        pairs.push(("title", Value::String(title)));
    }
    pairs.push(("attachmentId", string_or_null(node, "data-attachment-id")));
    pairs.push((
        "sharedAttachmentId",
        string_or_null(node, "data-shared-attachment-id"),
    ));
    pairs.push((
        "editorWidth",
        json!(crate::document::clamp_image_width(width)),
    ));
    attrs_of(pairs)
}

fn file_attachment_attrs(node: &NodeRef) -> Outcome {
    attrs_of(vec![
        ("attachmentId", string_or_null(node, "data-attachment-id")),
        (
            "sharedAttachmentId",
            string_or_null(node, "data-shared-attachment-id"),
        ),
        ("name", string_or_null(node, "data-name")),
        ("mimeType", string_or_null(node, "data-mime-type")),
        ("src", string_or_null(node, "data-src")),
        (
            "size",
            attr(node, "data-size")
                .filter(|size| !size.is_empty())
                .map(|size| js_number(js_number_of(&size)))
                .unwrap_or(Value::Null),
        ),
    ])
}

fn app_link_attrs(node: &NodeRef) -> Outcome {
    let number = attr(node, "data-number")
        .filter(|value| !value.is_empty())
        .map(|value| js_number(js_number_of(&value)))
        .unwrap_or(Value::Null);
    attrs_of(vec![
        (
            "provider",
            attr(node, "data-provider")
                .map(Value::String)
                .unwrap_or(json!("github")),
        ),
        ("kind", string_or_null(node, "data-kind")),
        ("url", string_or_null(node, "data-url")),
        ("owner", string_or_null(node, "data-owner")),
        ("repo", string_or_null(node, "data-repo")),
        ("number", number),
        ("subId", string_or_null(node, "data-sub-id")),
        ("workspace", string_or_null(node, "data-workspace")),
        ("channelId", string_or_null(node, "data-channel-id")),
        ("messageTs", string_or_null(node, "data-message-ts")),
        ("threadTs", string_or_null(node, "data-thread-ts")),
        ("guildId", string_or_null(node, "data-guild-id")),
        ("messageId", string_or_null(node, "data-message-id")),
        ("inviteCode", string_or_null(node, "data-invite-code")),
        ("resourceId", string_or_null(node, "data-resource-id")),
        ("resourceTitle", string_or_null(node, "data-resource-title")),
    ])
}

fn mention_attrs(node: &NodeRef) -> Outcome {
    attrs_of(vec![
        ("id", string_or_null(node, "data-id")),
        ("type", string_or_null(node, "data-type")),
        ("label", string_or_null(node, "data-label")),
    ])
}

/// `getOptionalTaskStatus`
fn session_attrs(node: &NodeRef) -> Outcome {
    let status = attr(node, "data-status");
    let checked = match attr(node, "data-checked").as_deref() {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    };
    let status = match status.as_deref() {
        Some(status @ ("todo" | "in_progress" | "done")) => Some(status.to_string()),
        _ => checked.map(|checked| if checked { "done" } else { "todo" }.to_string()),
    };
    attrs_of(vec![
        ("sessionId", string_or_null(node, "data-session-id")),
        (
            "status",
            status.clone().map(Value::String).unwrap_or(Value::Null),
        ),
        (
            "checked",
            status
                .map(|status| json!(status == "done"))
                .unwrap_or(Value::Null),
        ),
    ])
}

fn clip_attrs_from(src: Option<String>) -> Outcome {
    let parsed = src.and_then(|src| crate::editor::clip::parse_youtube_url(&src))?;
    attrs_of(vec![("src", Value::String(parsed))])
}

fn clip_div_attrs(node: &NodeRef) -> Outcome {
    clip_attrs_from(attr(node, "data-src"))
}

fn clip_iframe_attrs(node: &NodeRef) -> Outcome {
    clip_attrs_from(attr(node, "src"))
}

fn link_attrs(node: &NodeRef) -> Outcome {
    let href = attr(node, "href");
    if href
        .as_deref()
        .is_some_and(|href| href.starts_with("asset://"))
    {
        return None;
    }
    attrs_of(vec![
        ("href", href.map(Value::String).unwrap_or(Value::Null)),
        ("target", string_or_null(node, "target")),
    ])
}

fn ordered_list_attrs(node: &NodeRef) -> Outcome {
    // `el.hasAttribute("start") ? +el.getAttribute("start") : 1`
    let start = attr(node, "start")
        .map(|start| js_number(js_number_of(&start)))
        .unwrap_or(json!(1));
    attrs_of(vec![("start", start)])
}

/// `<b>`: not bold when its own style says `font-weight: normal`.
fn bold_tag_attrs(node: &NodeRef) -> Outcome {
    (Style::of(node).get("font-weight") != Some("normal")).then_some(None)
}

fn italic_tag_attrs(node: &NodeRef) -> Outcome {
    (Style::of(node).get("font-style") != Some("normal")).then_some(None)
}

fn bold_style_attrs(value: &str) -> Outcome {
    let bold = value == "bold"
        || value == "bolder"
        || (value.len() >= 3
            && value.chars().all(|c| c.is_ascii_digit())
            && matches!(value.as_bytes()[0], b'5'..=b'9'));
    bold.then_some(None)
}

fn underline_style_attrs(value: &str) -> Outcome {
    value.contains("underline").then_some(None)
}

fn strike_style_attrs(value: &str) -> Outcome {
    value.contains("line-through").then_some(None)
}

fn tag(css: &str, node: &str, get_attrs: fn(&NodeRef) -> Outcome) -> Rule {
    Rule {
        selector: selector(css),
        target: Target::Node(schema().node(node).unwrap()),
        get_attrs: Some(get_attrs),
        get_style_attrs: None,
        preserve_whitespace: None,
    }
}

fn mark_tag(css: &str, mark: &str, get_attrs: Option<fn(&NodeRef) -> Outcome>) -> Rule {
    Rule {
        selector: selector(css),
        target: Target::Mark(schema().mark(mark).unwrap()),
        get_attrs,
        get_style_attrs: None,
        preserve_whitespace: None,
    }
}

fn mark_style(
    prop: &'static str,
    value: Option<&'static str>,
    mark: &str,
    get_attrs: Option<fn(&str) -> Outcome>,
) -> Rule {
    Rule {
        selector: Selector::Style(prop, value),
        target: Target::Mark(schema().mark(mark).unwrap()),
        get_attrs: None,
        get_style_attrs: get_attrs,
        preserve_whitespace: None,
    }
}

impl Parser {
    /// `DOMParser.fromSchema`: `schemaRules` in mark then node order.
    pub fn for_schema() -> Parser {
        let s = schema();
        let mut rules = vec![
            mark_tag("strong", "bold", None),
            mark_tag("b", "bold", Some(bold_tag_attrs)),
            Rule {
                selector: Selector::Style("font-weight", Some("400")),
                target: Target::ClearMark(s.mark("bold").unwrap()),
                get_attrs: None,
                get_style_attrs: None,
                preserve_whitespace: None,
            },
            mark_style("font-weight", None, "bold", Some(bold_style_attrs)),
            mark_tag("em", "italic", None),
            mark_tag("i", "italic", Some(italic_tag_attrs)),
            mark_style("font-style", Some("italic"), "italic", None),
            mark_tag("u", "underline", None),
            mark_style(
                "text-decoration",
                None,
                "underline",
                Some(underline_style_attrs),
            ),
            mark_tag("s", "strike", None),
            mark_tag("del", "strike", None),
            mark_style("text-decoration", None, "strike", Some(strike_style_attrs)),
            mark_tag("code", "code", None),
            mark_tag("a[href]", "link", Some(link_attrs)),
            mark_tag("mark", "highlight", None),
            tag("p", "paragraph", no_attrs),
        ];
        rules.extend((1..=6).map(heading_rule));
        rules.extend([
            tag("blockquote", "blockquote", no_attrs),
            Rule {
                preserve_whitespace: Some(PreserveWs::Full),
                ..tag("pre", "codeBlock", no_attrs)
            },
            tag("hr", "horizontalRule", no_attrs),
            tag("br", "hardBreak", no_attrs),
            tag("ul:not([data-type])", "bulletList", no_attrs),
            tag("ol", "orderedList", ordered_list_attrs),
            tag("li:not([data-type])", "listItem", no_attrs),
            tag("table", "table", no_attrs),
            tag("tr", "tableRow", no_attrs),
            tag("td", "tableCell", table_cell_attrs),
            tag("th", "tableHeader", table_cell_attrs),
            tag("ul[data-type=\"taskList\"]", "taskList", no_attrs),
            tag("li[data-type=\"taskItem\"]", "taskItem", task_item_attrs),
            tag("img[src]", "image", image_attrs),
            tag(
                "div[data-type=\"file-attachment\"]",
                "fileAttachment",
                file_attachment_attrs,
            ),
            tag("span[data-type=\"app-link\"]", "appLink", app_link_attrs),
            tag(
                "span.mention[data-mention=\"true\"]",
                "mention-@",
                mention_attrs,
            ),
            tag("div[data-type=\"session\"]", "session", session_attrs),
            tag("div[data-type=\"clip\"]", "clip", clip_div_attrs),
            tag("iframe[src]", "clip", clip_iframe_attrs),
        ]);
        let mut matched_styles = Vec::new();
        for rule in &rules {
            if let Selector::Style(prop, _) = rule.selector
                && !matched_styles.contains(&prop)
            {
                matched_styles.push(prop);
            }
        }
        // `normalizeLists`: none of the `ul` / `ol` node types can contain
        // itself directly.
        let normalize_lists = ["bulletList", "orderedList", "taskList"]
            .iter()
            .map(|name| s.node(name).unwrap())
            .all(|list| s.nodes[list].dfa.match_type(0, list).is_none());
        Parser {
            schema: s,
            rules,
            matched_styles,
            normalize_lists,
        }
    }

    fn match_tag(&self, dom: &NodeRef, after: Option<usize>) -> Option<(usize, Option<Attrs>)> {
        let element = dom.clone().into_element_ref()?;
        let start = after.map_or(0, |index| index + 1);
        for (index, rule) in self.rules.iter().enumerate().skip(start) {
            let Selector::Tag(selectors) = &rule.selector else {
                continue;
            };
            if !selectors.matches(&element) {
                continue;
            }
            let attrs = match rule.get_attrs {
                Some(get_attrs) => match get_attrs(dom) {
                    Some(attrs) => attrs,
                    None => continue,
                },
                None => None,
            };
            return Some((index, attrs));
        }
        None
    }

    fn match_style(
        &self,
        prop: &str,
        value: &str,
        after: Option<usize>,
    ) -> Option<(usize, Option<Attrs>)> {
        let start = after.map_or(0, |index| index + 1);
        for (index, rule) in self.rules.iter().enumerate().skip(start) {
            let Selector::Style(rule_prop, rule_value) = &rule.selector else {
                continue;
            };
            if *rule_prop != prop || rule_value.is_some_and(|expected| expected != value) {
                continue;
            }
            let attrs = match rule.get_style_attrs {
                Some(get_attrs) => match get_attrs(value) {
                    Some(attrs) => attrs,
                    None => continue,
                },
                None => None,
            };
            return Some((index, attrs));
        }
        None
    }

    /// `parseSlice(dom, options)`.
    pub fn parse_slice(&self, dom: &NodeRef, options: &ParseOptions) -> super::node::Slice {
        let mut context = ParseContext::new(self, options, true);
        context.add_all(dom, Vec::new());
        let fragment = context.finish();
        super::node::Slice::max_open(self.schema, fragment, true)
    }
}

pub struct ParseOptions<'a> {
    pub preserve_whitespace: Option<PreserveWs>,
    pub context: Option<&'a ResolvedPos<'a>>,
    /// `parseFromClipboard`'s `ruleFromNode`: a trailing `<br>` in a block
    /// parent is ignored.
    pub clipboard_br_rule: bool,
}

fn ws_options_for(
    schema: &Schema,
    ty: Option<TypeId>,
    preserve: Option<PreserveWs>,
    base: u8,
) -> u8 {
    match preserve {
        Some(PreserveWs::Collapse) => 0,
        Some(PreserveWs::Preserve) => OPT_PRESERVE_WS,
        Some(PreserveWs::Full) => OPT_PRESERVE_WS | OPT_PRESERVE_WS_FULL,
        None => {
            if ty.is_some_and(|ty| schema.nodes[ty].whitespace_pre()) {
                OPT_PRESERVE_WS | OPT_PRESERVE_WS_FULL
            } else {
                base & !OPT_OPEN_LEFT
            }
        }
    }
}

struct NodeContext {
    /// Identity, for `sync`: a context closed and replaced at the same depth
    /// is a different one.
    id: usize,
    ty: Option<TypeId>,
    attrs: Option<Attrs>,
    marks: Vec<Mark>,
    solid: bool,
    m: Option<Match>,
    options: u8,
    content: Vec<Node>,
}

impl NodeContext {
    #[allow(clippy::too_many_arguments)]
    fn new(
        id: usize,
        schema: &'static Schema,
        ty: Option<TypeId>,
        attrs: Option<Attrs>,
        marks: Vec<Mark>,
        solid: bool,
        m: Option<Match>,
        options: u8,
    ) -> NodeContext {
        let m = m.or_else(|| {
            if options & OPT_OPEN_LEFT != 0 {
                None
            } else {
                ty.map(|ty| Match {
                    dfa: &schema.nodes[ty].dfa,
                    state: 0,
                })
            }
        });
        NodeContext {
            id,
            ty,
            attrs,
            marks,
            solid,
            m,
            options,
            content: Vec::new(),
        }
    }

    fn find_wrapping(&mut self, schema: &'static Schema, node: &Node) -> Option<Vec<TypeId>> {
        if self.m.is_none() {
            let Some(ty) = self.ty else {
                return Some(Vec::new());
            };
            let start = Match {
                dfa: &schema.nodes[ty].dfa,
                state: 0,
            };
            if let Some(fill) =
                start.fill_before(&Fragment::from(vec![node.clone()]), false, 0, schema)
            {
                self.m = Some(
                    start
                        .match_fragment(&fill, 0, fill.child_count())
                        .expect("the fill matches"),
                );
            } else if let Some(wrap) = start.find_wrapping(node.type_id, schema) {
                self.m = Some(start);
                return Some(wrap);
            } else {
                return None;
            }
        }
        self.m.as_ref().unwrap().find_wrapping(node.type_id, schema)
    }

    fn finish(mut self, schema: &'static Schema, open_end: bool) -> Result<Node, Fragment> {
        if self.options & OPT_PRESERVE_WS == 0
            && let Some(last) = self.content.last()
            && let Some(text) = &last.text
        {
            let trimmed = text.trim_end_matches(is_html_space);
            if trimmed.is_empty() {
                self.content.pop();
            } else if trimmed.len() != text.len() {
                let node = last.with_text(trimmed.to_string());
                *self.content.last_mut().unwrap() = node;
            }
        }
        let mut content = Fragment::from(self.content);
        if !open_end && let Some(m) = &self.m {
            let fill = m
                .fill_before(&Fragment::empty(), true, 0, schema)
                .unwrap_or_default();
            content = content.append(&fill);
        }
        match self.ty {
            Some(ty) => Ok(Node::new(
                schema,
                ty,
                self.attrs.as_ref(),
                content,
                self.marks,
            )),
            None => Err(content),
        }
    }

    fn inline_context(&self, schema: &Schema, node: &NodeRef) -> bool {
        if let Some(ty) = self.ty {
            return schema.nodes[ty].inline_content(schema);
        }
        if let Some(first) = self.content.first() {
            return first.is_inline(schema);
        }
        node.parent()
            .and_then(|parent| tag_name(&parent))
            .is_some_and(|name| !BLOCK_TAGS.contains(&name.as_str()))
    }
}

/// `[ \t\r\n\u000c]`
fn is_html_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n' | '\u{c}')
}

/// JavaScript's `\s`.
fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

fn collapse_html_space(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut in_space = false;
    for c in value.chars() {
        if is_html_space(c) {
            if !in_space {
                out.push(' ');
                in_space = true;
            }
        } else {
            out.push(c);
            in_space = false;
        }
    }
    out
}

struct ParseContext<'p, 'o> {
    parser: &'p Parser,
    schema: &'static Schema,
    options: &'o ParseOptions<'o>,
    is_open: bool,
    open: usize,
    needs_block: bool,
    nodes: Vec<NodeContext>,
    local_preserve_ws: bool,
    next_id: usize,
}

impl<'p, 'o> ParseContext<'p, 'o> {
    fn new(
        parser: &'p Parser,
        options: &'o ParseOptions<'o>,
        is_open: bool,
    ) -> ParseContext<'p, 'o> {
        let schema = parser.schema;
        let top_options = ws_options_for(schema, None, options.preserve_whitespace, 0)
            | if is_open { OPT_OPEN_LEFT } else { 0 };
        let top = if is_open {
            NodeContext::new(0, schema, None, None, Vec::new(), true, None, top_options)
        } else {
            NodeContext::new(
                0,
                schema,
                Some(schema.top_type()),
                None,
                Vec::new(),
                true,
                None,
                top_options,
            )
        };
        ParseContext {
            parser,
            schema,
            options,
            is_open,
            open: 0,
            needs_block: false,
            nodes: vec![top],
            local_preserve_ws: false,
            next_id: 1,
        }
    }

    fn add_dom(&mut self, dom: &NodeRef, marks: Vec<Mark>) {
        if dom.as_text().is_some() {
            self.add_text_node(dom, marks);
        } else if dom.as_element().is_some() {
            self.add_element(dom, marks, None);
        }
    }

    fn add_text_node(&mut self, dom: &NodeRef, marks: Vec<Mark>) {
        let mut value = dom
            .as_text()
            .map(|text| text.borrow().clone())
            .unwrap_or_default();
        let top_options = self.nodes[self.open].options;
        let preserve_full = top_options & OPT_PRESERVE_WS_FULL != 0;
        let preserve =
            preserve_full || self.local_preserve_ws || top_options & OPT_PRESERVE_WS != 0;
        let inline_context = self.nodes[self.open].inline_context(self.schema, dom);
        if preserve_full || inline_context || value.chars().any(|c| !is_html_space(c)) {
            if !preserve {
                value = collapse_html_space(&value);
                if value.starts_with(is_html_space) && self.open == self.nodes.len() - 1 {
                    let top = &self.nodes[self.open];
                    let node_before = top.content.last();
                    let dom_before = dom.previous_sibling();
                    let strip = match node_before {
                        None => true,
                        Some(before) => {
                            dom_before
                                .as_ref()
                                .and_then(tag_name)
                                .is_some_and(|name| name == "br")
                                || before
                                    .text
                                    .as_deref()
                                    .is_some_and(|text| text.ends_with(is_html_space))
                        }
                    };
                    if strip {
                        value.remove(0);
                    }
                }
            } else if preserve_full {
                value = value.replace("\r\n", "\n").replace('\r', "\n");
            } else {
                value = value.replace("\r\n", " ").replace(['\n', '\r'], " ");
            }
            if !value.is_empty() {
                let cautious = value.chars().all(is_js_space);
                let node = Node::text(self.schema, value, Vec::new());
                self.insert_node(node, marks, cautious);
            }
        }
    }

    fn add_element(&mut self, dom: &NodeRef, marks: Vec<Mark>, match_after: Option<usize>) {
        let outer_ws = self.local_preserve_ws;
        let name = tag_name(dom).unwrap_or_default();
        if name == "pre"
            || Style::of(dom)
                .get("white-space")
                .is_some_and(|value| value.contains("pre"))
        {
            self.local_preserve_ws = true;
        }
        if LIST_TAGS.contains(&name.as_str()) && self.parser.normalize_lists {
            normalize_list(dom);
        }
        // `ruleFromNode`: the clipboard's trailing-`<br>` rule comes first.
        let br_ignored = self.options.clipboard_br_rule
            && name == "br"
            && dom.next_sibling().is_none()
            && dom
                .parent()
                .and_then(|parent| tag_name(&parent))
                .is_some_and(|parent| !INLINE_PARENTS.contains(&parent.as_str()));
        let rule = if br_ignored {
            None
        } else {
            self.parser.match_tag(dom, match_after)
        };
        let ignore = if br_ignored {
            true
        } else if rule.is_none() {
            IGNORE_TAGS.contains(&name.as_str())
        } else {
            false
        };
        if ignore {
            self.ignore_fallback(dom, marks);
        } else if let Some((index, attrs)) = rule {
            if let Some(inner_marks) = self.read_styles(dom, marks) {
                self.add_element_by_rule(dom, index, attrs, inner_marks);
            }
        } else {
            let mut sync = false;
            let old_needs_block = self.needs_block;
            let mut top_id = self.nodes[self.open].id;
            if BLOCK_TAGS.contains(&name.as_str()) {
                let top = &self.nodes[self.open];
                if let Some(first) = top.content.first()
                    && first.is_inline(self.schema)
                    && self.open > 0
                {
                    self.open -= 1;
                    top_id = self.nodes[self.open].id;
                }
                sync = true;
                if self.nodes[self.open].ty.is_none() {
                    self.needs_block = true;
                }
            } else if dom.first_child().is_none() {
                self.leaf_fallback(dom, marks);
                self.local_preserve_ws = outer_ws;
                return;
            }
            if let Some(inner_marks) = self.read_styles(dom, marks) {
                self.add_all(dom, inner_marks);
            }
            if sync {
                self.sync(top_id);
            }
            self.needs_block = old_needs_block;
        }
        self.local_preserve_ws = outer_ws;
    }

    fn leaf_fallback(&mut self, dom: &NodeRef, marks: Vec<Mark>) {
        if tag_name(dom).as_deref() == Some("br")
            && self.nodes[self.open]
                .ty
                .is_some_and(|ty| self.schema.nodes[ty].inline_content(self.schema))
        {
            let text = NodeRef::new_text("\n");
            self.add_text_node(&text, marks);
        }
    }

    fn ignore_fallback(&mut self, dom: &NodeRef, marks: Vec<Mark>) {
        if tag_name(dom).as_deref() == Some("br")
            && !self.nodes[self.open]
                .ty
                .is_some_and(|ty| self.schema.nodes[ty].inline_content(self.schema))
        {
            let dash = Node::text(self.schema, "-", Vec::new());
            self.find_place(&dash, marks, true);
        }
    }

    /// `readStyles`: `None` when a style rule says to ignore the element.
    fn read_styles(&mut self, dom: &NodeRef, mut marks: Vec<Mark>) -> Option<Vec<Mark>> {
        let style = Style::of(dom);
        if style.is_empty() {
            return Some(marks);
        }
        for prop in &self.parser.matched_styles {
            let Some(value) = style.get(prop) else {
                continue;
            };
            // Every style rule here is consuming: the first match decides.
            if let Some((index, attrs)) = self.parser.match_style(prop, value, None) {
                match &self.parser.rules[index].target {
                    Target::ClearMark(mark) => marks.retain(|m| m.type_id != *mark),
                    Target::Mark(mark) => {
                        marks.push(Mark::new(self.schema, *mark, attrs.as_ref()));
                    }
                    Target::Node(_) => {}
                }
            }
        }
        Some(marks)
    }

    fn add_element_by_rule(
        &mut self,
        dom: &NodeRef,
        index: usize,
        attrs: Option<Attrs>,
        mut marks: Vec<Mark>,
    ) {
        let schema = self.schema;
        let mut sync = false;
        let mut node_type = None;
        let preserve = self.parser.rules[index].preserve_whitespace;
        match &self.parser.rules[index].target {
            Target::Node(ty) => {
                node_type = Some(*ty);
                if !schema.nodes[*ty].is_leaf() {
                    if let Some(inner) = self.enter(*ty, attrs.clone(), marks.clone(), preserve) {
                        sync = true;
                        marks = inner;
                    }
                } else {
                    let node =
                        Node::new(schema, *ty, attrs.as_ref(), Fragment::empty(), Vec::new());
                    let is_br = tag_name(dom).as_deref() == Some("br");
                    if !self.insert_node(node, marks.clone(), is_br) {
                        self.leaf_fallback(dom, marks.clone());
                    }
                }
            }
            Target::Mark(mark) => {
                marks.push(Mark::new(schema, *mark, attrs.as_ref()));
            }
            Target::ClearMark(_) => {}
        }
        let start_in = self.nodes[self.open].id;
        if !node_type.is_some_and(|ty| schema.nodes[ty].is_leaf()) {
            self.add_all(dom, marks);
        }
        if sync && self.sync(start_in) {
            self.open -= 1;
        }
    }

    fn add_all(&mut self, parent: &NodeRef, marks: Vec<Mark>) {
        let mut child = parent.first_child();
        while let Some(dom) = child {
            child = dom.next_sibling();
            self.add_dom(&dom, marks.clone());
        }
    }

    /// `findPlace`: the wrappers to open so `node` fits, possibly closing
    /// non-solid open nodes; the marks left to apply inside them.
    fn find_place(&mut self, node: &Node, marks: Vec<Mark>, cautious: bool) -> Option<Vec<Mark>> {
        let schema = self.schema;
        let mut route: Option<Vec<TypeId>> = None;
        let mut sync = None;
        let mut penalty = 0;
        let mut depth = self.open as isize;
        while depth >= 0 {
            let cx = &mut self.nodes[depth as usize];
            let found = cx.find_wrapping(schema, node);
            if let Some(found) = found
                && route
                    .as_ref()
                    .is_none_or(|route| route.len() > found.len() + penalty)
            {
                let empty = found.is_empty();
                route = Some(found);
                sync = Some(self.nodes[depth as usize].id);
                if empty {
                    break;
                }
            }
            if self.nodes[depth as usize].solid {
                if cautious {
                    break;
                }
                penalty += 2;
            }
            depth -= 1;
        }
        let route = route?;
        self.sync(sync.unwrap());
        let mut marks = marks;
        for ty in route {
            marks = self.enter_inner(ty, None, marks, false, None);
        }
        Some(marks)
    }

    fn insert_node(&mut self, node: Node, mut marks: Vec<Mark>, cautious: bool) -> bool {
        let schema = self.schema;
        if node.is_inline(schema)
            && self.needs_block
            && self.nodes[self.open].ty.is_none()
            && let Some(block) = self.textblock_from_context()
        {
            marks = self.enter_inner(block, None, marks, false, None);
        }
        let Some(inner_marks) = self.find_place(&node, marks, cautious) else {
            return false;
        };
        self.close_extra(false);
        let top = &mut self.nodes[self.open];
        if let Some(m) = top.m {
            top.m = m.match_type(node.type_id);
        }
        let mut node_marks: Vec<Mark> = Vec::new();
        for m in inner_marks.iter().chain(node.marks.iter()) {
            let allowed = match top.ty {
                Some(ty) => schema.nodes[ty].allows_mark_type(m.type_id),
                None => mark_may_apply(schema, m.type_id, node.type_id),
            };
            if allowed {
                node_marks = m.add_to_set(schema, &node_marks);
            }
        }
        top.content.push(node.mark(node_marks));
        true
    }

    fn enter(
        &mut self,
        ty: TypeId,
        attrs: Option<Attrs>,
        marks: Vec<Mark>,
        preserve: Option<PreserveWs>,
    ) -> Option<Vec<Mark>> {
        let node = Node::new(
            self.schema,
            ty,
            attrs.as_ref(),
            Fragment::empty(),
            Vec::new(),
        );
        let inner = self.find_place(&node, marks.clone(), false)?;
        let _ = inner;
        Some(self.enter_inner(ty, attrs, marks, true, preserve))
    }

    fn enter_inner(
        &mut self,
        ty: TypeId,
        attrs: Option<Attrs>,
        marks: Vec<Mark>,
        solid: bool,
        preserve: Option<PreserveWs>,
    ) -> Vec<Mark> {
        let schema = self.schema;
        self.close_extra(false);
        let top = &mut self.nodes[self.open];
        if let Some(m) = top.m {
            top.m = m.match_type(ty);
        }
        let mut options = ws_options_for(schema, Some(ty), preserve, top.options);
        if top.options & OPT_OPEN_LEFT != 0 && top.content.is_empty() {
            options |= OPT_OPEN_LEFT;
        }
        let top_ty = top.ty;
        let mut apply_marks: Vec<Mark> = Vec::new();
        let mut remaining = Vec::new();
        for m in marks {
            let allowed = match top_ty {
                Some(top_ty) => schema.nodes[top_ty].allows_mark_type(m.type_id),
                None => mark_may_apply(schema, m.type_id, ty),
            };
            if allowed {
                apply_marks = m.add_to_set(schema, &apply_marks);
            } else {
                remaining.push(m);
            }
        }
        let id = self.next_id;
        self.next_id += 1;
        self.nodes.push(NodeContext::new(
            id,
            schema,
            Some(ty),
            attrs,
            apply_marks,
            solid,
            None,
            options,
        ));
        self.open += 1;
        remaining
    }

    fn close_extra(&mut self, open_end: bool) {
        let schema = self.schema;
        let mut i = self.nodes.len() - 1;
        if i > self.open {
            while i > self.open {
                let node = self.nodes.pop().unwrap();
                let finished = node
                    .finish(schema, open_end)
                    .expect("a typed node context finishes as a node");
                self.nodes[i - 1].content.push(finished);
                i -= 1;
            }
        }
    }

    fn finish(mut self) -> Fragment {
        self.open = 0;
        self.close_extra(self.is_open);
        let top = self.nodes.pop().unwrap();
        match top.finish(self.schema, self.is_open) {
            Ok(node) => Fragment::from(vec![node]),
            Err(fragment) => fragment,
        }
    }

    /// `sync(to)`: back out to the context with this id.
    fn sync(&mut self, to: usize) -> bool {
        let mut i = self.open as isize;
        while i >= 0 {
            if self.nodes[i as usize].id == to {
                self.open = i as usize;
                return true;
            } else if self.local_preserve_ws {
                self.nodes[i as usize].options |= OPT_PRESERVE_WS;
            }
            i -= 1;
        }
        false
    }

    /// `textblockFromContext`: the context's default textblock, else the
    /// schema's first.
    fn textblock_from_context(&self) -> Option<TypeId> {
        let schema = self.schema;
        if let Some(context) = self.options.context {
            for d in (0..=context.depth()).rev() {
                let node = context.node(d);
                let m = node.content_match_at(schema, context.index_after(d));
                if let Some(deflt) = m.default_type(schema)
                    && schema.nodes[deflt].is_textblock(schema)
                    && schema.nodes[deflt].default_attrs().is_some()
                {
                    return Some(deflt);
                }
            }
        }
        schema
            .nodes
            .iter()
            .find(|node| node.is_textblock(schema) && node.default_attrs().is_some())
            .map(|node| node.id)
    }
}

const INLINE_PARENTS: &[&str] = &[
    "a", "abbr", "acronym", "b", "cite", "code", "del", "em", "i", "ins", "kbd", "label", "output",
    "q", "ruby", "s", "samp", "span", "strong", "sub", "sup", "time", "u", "tt", "var",
];

/// `normalizeList`: a list directly inside a list moves into the item
/// before it.
fn normalize_list(dom: &NodeRef) {
    let mut prev_item: Option<NodeRef> = None;
    let mut child = dom.first_child();
    while let Some(node) = child {
        let name = tag_name(&node);
        let next = node.next_sibling();
        match name.as_deref() {
            Some(name) if LIST_TAGS.contains(&name) && prev_item.is_some() => {
                node.detach();
                prev_item.as_ref().unwrap().append(node);
            }
            Some("li") => prev_item = Some(node),
            Some(_) => prev_item = None,
            None => {}
        }
        child = next;
    }
}

/// `markMayApply`: some node type that allows the mark can contain `ty`.
fn mark_may_apply(schema: &'static Schema, mark: MarkId, ty: TypeId) -> bool {
    schema.nodes.iter().any(|parent| {
        if !parent.allows_mark_type(mark) {
            return false;
        }
        let mut seen: Vec<usize> = Vec::new();
        fn scan(
            dfa: &super::content::Dfa,
            state: usize,
            ty: TypeId,
            seen: &mut Vec<usize>,
        ) -> bool {
            seen.push(state);
            for (edge_ty, next) in dfa.edges(state) {
                if *edge_ty == ty {
                    return true;
                }
                if !seen.contains(next) && scan(dfa, *next, ty, seen) {
                    return true;
                }
            }
            false
        }
        scan(&parent.dfa, 0, ty, &mut seen)
    })
}

/// `readHTML` + the `<div>`'s `innerHTML`: the fragment's element, the
/// leading `<meta>` tags dropped and table parts wrapped so the parser
/// keeps them.
pub fn read_html(html: &str) -> NodeRef {
    let mut html = html;
    while let Some(rest) = html.trim_start().strip_prefix("<meta ") {
        match rest.find('>') {
            Some(end) => html = &rest[end + 1..],
            None => break,
        }
    }
    let wrap_map: &[(&str, &[&str])] = &[
        ("thead", &["table"]),
        ("tbody", &["table"]),
        ("tfoot", &["table"]),
        ("caption", &["table"]),
        ("colgroup", &["table"]),
        ("col", &["table", "colgroup"]),
        ("tr", &["table", "tbody"]),
        ("td", &["table", "tbody", "tr"]),
        ("th", &["table", "tbody", "tr"]),
    ];
    // `/<([a-z][^>\s]+)/i`
    let first_tag = html.match_indices('<').find_map(|(start, _)| {
        let rest = &html[start + 1..];
        rest.chars().next().filter(char::is_ascii_alphabetic)?;
        let tag: String = rest
            .chars()
            .take_while(|c| !c.is_whitespace() && *c != '>')
            .collect();
        (tag.len() >= 2).then(|| tag.to_lowercase())
    });
    let wrap = first_tag.and_then(|tag| {
        wrap_map
            .iter()
            .find(|(name, _)| *name == tag)
            .map(|(_, wrap)| *wrap)
    });
    let wrapped = match wrap {
        Some(wrap) => format!(
            "{}{}{}",
            wrap.iter().map(|n| format!("<{n}>")).collect::<String>(),
            html,
            wrap.iter()
                .rev()
                .map(|n| format!("</{n}>"))
                .collect::<String>()
        ),
        None => html.to_string(),
    };
    let context =
        html5ever::QualName::new(None, html5ever::ns!(html), html5ever::local_name!("div"));
    let document = kuchikiki::parse_fragment(context, Vec::new())
        .one(wrapped)
        .document_node;
    let mut element = document
        .first_child()
        .expect("the fragment parse yields an html element");
    if let Some(wrap) = wrap {
        for name in wrap {
            if let Ok(found) = element.select_first(name) {
                element = found.as_node().clone();
            }
        }
    }
    element
}

/// `restoreReplacedSpaces` for WebKit: Safari's `span.Apple-converted-space`
/// holding one no-break space becomes a plain space.
pub fn restore_replaced_spaces(dom: &NodeRef) {
    let Ok(spans) = dom.select("span.Apple-converted-space") else {
        return;
    };
    let spans: Vec<NodeRef> = spans.map(|span| span.as_node().clone()).collect();
    for span in spans {
        let single = span
            .first_child()
            .filter(|child| child.next_sibling().is_none());
        if let Some(child) = single
            && child
                .as_text()
                .is_some_and(|text| *text.borrow() == "\u{a0}")
            && span.parent().is_some()
        {
            span.insert_after(NodeRef::new_text(" "));
            span.detach();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(html: &str) -> Value {
        let parser = Parser::for_schema();
        let dom = read_html(html);
        let slice = parser.parse_slice(
            &dom,
            &ParseOptions {
                preserve_whitespace: None,
                context: None,
                clipboard_br_rule: true,
            },
        );
        json!({"content": slice.content.to_json(schema()), "openStart": slice.open_start, "openEnd": slice.open_end})
    }

    #[test]
    fn parses_blocks_marks_and_lists() {
        let parsed = parse("<p>a <b>b</b></p><ul><li>c</li></ul>");
        assert_eq!(
            parsed["content"].to_string(),
            r#"[{"type":"paragraph","content":[{"type":"text","text":"a "},{"type":"text","marks":[{"type":"bold"}],"text":"b"}]},{"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"c"}]}]}]}]"#
        );
        assert_eq!(parsed["openStart"], 1);
        assert_eq!(parsed["openEnd"], 3);
    }

    #[test]
    fn styles_read_like_the_browser() {
        let style = Style::of(&read_html(r#"<span style="font-weight:700;white-space:pre;white-space:pre-wrap;text-decoration-line: underline !important">x</span>"#).first_child().unwrap());
        assert_eq!(style.get("font-weight"), Some("700"));
        assert_eq!(style.get("white-space"), Some("pre-wrap"));
        assert_eq!(style.get("text-decoration"), Some("underline"));
        let parsed = parse(
            r#"<span style="font-weight:700">bold</span> <b style="font-weight:normal">plain</b>"#,
        );
        assert_eq!(
            parsed["content"].to_string(),
            r#"[{"type":"text","marks":[{"type":"bold"}],"text":"bold"},{"type":"text","text":" plain"}]"#
        );
    }

    #[test]
    fn meta_tags_and_table_parts_are_handled() {
        let dom = read_html("<meta charset='utf-8'><meta charset=\"utf-8\"><p>x</p>");
        assert_eq!(dom.children().count(), 1);
        let dom = read_html("<td>cell</td>");
        assert_eq!(tag_name(&dom).as_deref(), Some("tr"));
    }
}
