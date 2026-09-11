//! `DOMSerializer.fromSchema(schema)` over the note schema's `toDOM` specs
//! and prosemirror-view's `serializeForClipboard`: the HTML a copy puts on
//! the clipboard next to the text, with the `data-pm-slice` context a
//! ProseMirror paste reads back.

use serde_json::Value;

use super::node::{Fragment, Mark, Node, Slice};
use super::schema::Schema;

/// `serializeForClipboard(view, slice).dom.innerHTML`
pub fn serialize_for_clipboard(schema: &Schema, slice: &Slice) -> String {
    let mut content = slice.content.clone();
    let mut open_start = slice.open_start;
    let mut open_end = slice.open_end;
    let mut context: Vec<Value> = Vec::new();
    while open_start > 1
        && open_end > 1
        && content.child_count() == 1
        && content
            .first_child()
            .is_some_and(|node| node.child_count() == 1)
    {
        open_start -= 1;
        open_end -= 1;
        let node = content.first_child().unwrap().clone();
        context.push(Value::String(schema.nodes[node.type_id].name.into()));
        // `node.attrs != node.type.defaultAttrs ? node.attrs : null`
        let default_attrs = schema.nodes[node.type_id].default_attrs();
        context.push(if default_attrs.as_ref() == Some(&node.attrs) {
            Value::Null
        } else {
            Value::Object(node.attrs.clone())
        });
        content = node.content;
    }
    let mut html = String::new();
    serialize_fragment(schema, &content, &mut html);
    // `wrapMap`: table parts need their table around them for `innerHTML`.
    let mut wrappers = 0;
    let mut first_tag = first_element_tag(&html);
    while let Some(tag) = first_tag.as_deref()
        && let Some(wrap) = wrap_map(tag)
    {
        for name in wrap.iter().rev() {
            html = format!("<{name}>{html}</{name}>");
            wrappers += 1;
        }
        first_tag = first_element_tag(&html);
    }
    if first_element_tag(&html).is_some() {
        let value = format!(
            "{open_start} {open_end}{} {}",
            if wrappers > 0 {
                format!(" -{wrappers}")
            } else {
                String::new()
            },
            serde_json::to_string(&Value::Array(context)).unwrap_or_else(|_| "[]".into())
        );
        // The attribute goes on the first element, right after its tag name.
        if let Some(start) = html.find('<') {
            let name_end = html[start + 1..]
                .find([' ', '>', '/'])
                .map(|i| start + 1 + i)
                .unwrap_or(html.len());
            html.insert_str(
                name_end,
                &format!(" data-pm-slice=\"{}\"", escape_attribute(&value)),
            );
        }
    }
    html
}

fn wrap_map(tag: &str) -> Option<&'static [&'static str]> {
    Some(match tag {
        "thead" | "tbody" | "tfoot" | "caption" | "colgroup" => &["table"],
        "col" => &["table", "colgroup"],
        "tr" => &["table", "tbody"],
        "td" | "th" => &["table", "tbody", "tr"],
        _ => return None,
    })
}

fn first_element_tag(html: &str) -> Option<String> {
    let start = html.find('<')?;
    if start != 0 {
        // Text before the first element: `wrap.firstChild` is a text node.
        return None;
    }
    let tag: String = html[1..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    (!tag.is_empty()).then_some(tag)
}

/// The HTML serialisation of a text node's data.
fn escape_text(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\u{a0}' => out.push_str("&nbsp;"),
            c => out.push(c),
        }
    }
}

fn escape_attribute(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\u{a0}' => out.push_str("&nbsp;"),
            c => out.push(c),
        }
    }
    out
}

/// A `toDOM` output: the tag, its attributes (`null` values skipped like
/// `renderSpec`), and whether it has a content hole.
struct Spec {
    tag: &'static str,
    attrs: Vec<(&'static str, String)>,
    /// `["pre", ["code", 0]]`: an inner element holding the hole.
    inner: Option<&'static str>,
    hole: bool,
    /// A literal text child instead of the hole.
    text: Option<String>,
}

fn attr_string(node: &Node, name: &str) -> Option<String> {
    match node.attrs.get(name)? {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        other => Some(other.to_string()),
    }
}

/// JavaScript truthiness of an attribute value.
fn truthy(node: &Node, name: &str) -> bool {
    match node.attrs.get(name) {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Number(n)) => n.as_f64().is_some_and(|n| n != 0.0),
        Some(_) => true,
    }
}

fn node_spec(schema: &Schema, node: &Node) -> Spec {
    let plain = |tag: &'static str| Spec {
        tag,
        attrs: Vec::new(),
        inner: None,
        hole: true,
        text: None,
    };
    let leaf = |tag: &'static str, attrs: Vec<(&'static str, String)>| Spec {
        tag,
        attrs,
        inner: None,
        hole: false,
        text: None,
    };
    match schema.nodes[node.type_id].name {
        "paragraph" => plain("p"),
        "heading" => match node.attrs.get("level").and_then(Value::as_u64) {
            Some(2) => plain("h2"),
            Some(3) => plain("h3"),
            Some(4) => plain("h4"),
            Some(5) => plain("h5"),
            Some(6) => plain("h6"),
            _ => plain("h1"),
        },
        "blockquote" => plain("blockquote"),
        "codeBlock" => Spec {
            tag: "pre",
            attrs: Vec::new(),
            inner: Some("code"),
            hole: true,
            text: None,
        },
        "horizontalRule" => leaf("hr", Vec::new()),
        "hardBreak" => leaf("br", Vec::new()),
        "bulletList" => plain("ul"),
        "orderedList" => {
            let start = node.attrs.get("start").and_then(Value::as_i64).unwrap_or(1);
            if start == 1 {
                plain("ol")
            } else {
                Spec {
                    tag: "ol",
                    attrs: vec![
                        ("start", start.to_string()),
                        ("style", format!("counter-reset: ol-counter {}", start - 1)),
                    ],
                    inner: None,
                    hole: true,
                    text: None,
                }
            }
        }
        "listItem" => plain("li"),
        "table" => Spec {
            tag: "table",
            attrs: Vec::new(),
            inner: Some("tbody"),
            hole: true,
            text: None,
        },
        "tableRow" => plain("tr"),
        "tableCell" | "tableHeader" => Spec {
            tag: if schema.nodes[node.type_id].name == "tableCell" {
                "td"
            } else {
                "th"
            },
            attrs: table_cell_attrs(node),
            inner: None,
            hole: true,
            text: None,
        },
        "taskList" => Spec {
            tag: "ul",
            attrs: vec![
                ("data-type", "taskList".into()),
                ("class", "task-list".into()),
            ],
            inner: None,
            hole: true,
            text: None,
        },
        "taskItem" => {
            let status = task_status(node);
            let mut attrs = vec![
                ("data-type", "taskItem".to_string()),
                ("data-status", status.to_string()),
                ("data-checked", (status == "done").to_string()),
            ];
            if let Some(id) = attr_string(node, "taskId") {
                attrs.push(("data-task-id", id));
            }
            if let Some(id) = attr_string(node, "taskItemId") {
                attrs.push(("data-task-item-id", id));
            }
            Spec {
                tag: "li",
                attrs,
                inner: None,
                hole: true,
                text: None,
            }
        }
        "image" => {
            let mut attrs = Vec::new();
            for (name, attr) in [
                ("src", "src"),
                ("alt", "alt"),
                ("title", "title"),
                ("data-attachment-id", "attachmentId"),
                ("data-shared-attachment-id", "sharedAttachmentId"),
                ("data-editor-width", "editorWidth"),
            ] {
                if truthy(node, attr)
                    && let Some(value) = attr_string(node, attr)
                {
                    attrs.push((name, value));
                }
            }
            leaf("img", attrs)
        }
        "fileAttachment" => {
            let mut attrs = vec![("data-type", "file-attachment".to_string())];
            for (name, attr) in [
                ("data-attachment-id", "attachmentId"),
                ("data-shared-attachment-id", "sharedAttachmentId"),
                ("data-name", "name"),
                ("data-mime-type", "mimeType"),
                ("data-src", "src"),
            ] {
                if truthy(node, attr)
                    && let Some(value) = attr_string(node, attr)
                {
                    attrs.push((name, value));
                }
            }
            if let Some(size) = attr_string(node, "size") {
                attrs.push(("data-size", size));
            }
            Spec {
                tag: "div",
                attrs,
                inner: None,
                hole: false,
                text: Some(
                    attr_string(node, "name")
                        .filter(|name| !name.is_empty())
                        .unwrap_or_else(|| "attachment".into()),
                ),
            }
        }
        "appLink" => {
            let mut attrs = vec![
                ("data-type", "app-link".to_string()),
                (
                    "data-provider",
                    attr_string(node, "provider").unwrap_or_else(|| "github".into()),
                ),
                ("data-kind", attr_string(node, "kind").unwrap_or_default()),
                ("data-url", attr_string(node, "url").unwrap_or_default()),
            ];
            for (name, attr) in [("data-owner", "owner"), ("data-repo", "repo")] {
                if truthy(node, attr)
                    && let Some(value) = attr_string(node, attr)
                {
                    attrs.push((name, value));
                }
            }
            if let Some(number) = attr_string(node, "number") {
                attrs.push(("data-number", number));
            }
            for (name, attr) in [
                ("data-sub-id", "subId"),
                ("data-workspace", "workspace"),
                ("data-channel-id", "channelId"),
                ("data-message-ts", "messageTs"),
                ("data-thread-ts", "threadTs"),
                ("data-guild-id", "guildId"),
                ("data-message-id", "messageId"),
                ("data-invite-code", "inviteCode"),
                ("data-resource-id", "resourceId"),
                ("data-resource-title", "resourceTitle"),
            ] {
                if truthy(node, attr)
                    && let Some(value) = attr_string(node, attr)
                {
                    attrs.push((name, value));
                }
            }
            // `getAppLinkLabel` has no producer in the shipping note editor;
            // the URL is its fallback.
            Spec {
                tag: "span",
                attrs,
                inner: None,
                hole: false,
                text: Some(attr_string(node, "url").unwrap_or_default()),
            }
        }
        "mention-@" => {
            let label = attr_string(node, "label").unwrap_or_default();
            let mut attrs = vec![
                ("class", "mention".to_string()),
                ("data-mention", "true".to_string()),
            ];
            for (name, attr) in [
                ("data-id", "id"),
                ("data-type", "type"),
                ("data-label", "label"),
            ] {
                if let Some(value) = attr_string(node, attr) {
                    attrs.push((name, value));
                }
            }
            Spec {
                tag: "span",
                attrs,
                inner: None,
                hole: false,
                text: Some(label),
            }
        }
        "session" => {
            let status = optional_task_status(node);
            let mut attrs = vec![("data-type", "session".to_string())];
            if let Some(id) = attr_string(node, "sessionId") {
                attrs.push(("data-session-id", id));
            }
            if let Some(status) = status {
                attrs.push(("data-status", status.to_string()));
                attrs.push(("data-checked", (status == "done").to_string()));
            }
            Spec {
                tag: "div",
                attrs,
                inner: None,
                hole: true,
                text: None,
            }
        }
        "clip" => {
            let mut attrs = vec![("data-type", "clip".to_string())];
            if let Some(src) = attr_string(node, "src") {
                attrs.push(("data-src", src));
            }
            leaf("div", attrs)
        }
        _ => plain("div"),
    }
}

/// `setTableCellAttrs`
fn table_cell_attrs(node: &Node) -> Vec<(&'static str, String)> {
    let mut attrs = Vec::new();
    for (name, attr) in [("colspan", "colspan"), ("rowspan", "rowspan")] {
        if let Some(value) = node.attrs.get(attr)
            && value.as_i64() != Some(1)
            && let Some(text) = attr_string(node, attr)
        {
            attrs.push((name, text));
        }
    }
    if let Some(Value::Array(widths)) = node.attrs.get("colwidth") {
        attrs.push((
            "data-colwidth",
            widths
                .iter()
                .map(|width| width.to_string())
                .collect::<Vec<_>>()
                .join(","),
        ));
    }
    attrs
}

/// `normalizeTaskStatus(node.attrs.status, node.attrs.checked)`
fn task_status(node: &Node) -> &'static str {
    match node.attrs.get("status") {
        Some(Value::Bool(true)) => return "done",
        Some(Value::Bool(false)) => return "todo",
        Some(Value::String(s)) if s == "todo" => return "todo",
        Some(Value::String(s)) if s == "in_progress" => return "in_progress",
        Some(Value::String(s)) if s == "done" => return "done",
        _ => {}
    }
    match node.attrs.get("checked") {
        Some(Value::Bool(true)) => "done",
        _ => "todo",
    }
}

/// `getOptionalTaskStatus`
fn optional_task_status(node: &Node) -> Option<&'static str> {
    match node.attrs.get("status") {
        Some(Value::Bool(true)) => return Some("done"),
        Some(Value::Bool(false)) => return Some("todo"),
        Some(Value::String(s)) if s == "todo" => return Some("todo"),
        Some(Value::String(s)) if s == "in_progress" => return Some("in_progress"),
        Some(Value::String(s)) if s == "done" => return Some("done"),
        _ => {}
    }
    match node.attrs.get("checked") {
        Some(Value::Bool(true)) => Some("done"),
        Some(Value::Bool(false)) => Some("todo"),
        _ => None,
    }
}

fn mark_spec(schema: &Schema, mark: &Mark) -> (&'static str, Vec<(&'static str, String)>) {
    match schema.marks[mark.type_id].name {
        "bold" => ("strong", Vec::new()),
        "italic" => ("em", Vec::new()),
        "underline" => ("u", Vec::new()),
        "strike" => ("s", Vec::new()),
        "code" => ("code", Vec::new()),
        "link" => {
            let mut attrs = Vec::new();
            if let Some(Value::String(href)) = mark.attrs.get("href") {
                attrs.push(("href", href.clone()));
            }
            if let Some(Value::String(target)) = mark.attrs.get("target") {
                attrs.push(("target", target.clone()));
            }
            attrs.push(("rel", "noopener noreferrer nofollow".into()));
            ("a", attrs)
        }
        _ => ("mark", Vec::new()),
    }
}

fn open_tag(out: &mut String, tag: &str, attrs: &[(&str, String)]) {
    out.push('<');
    out.push_str(tag);
    for (name, value) in attrs {
        out.push(' ');
        out.push_str(name);
        out.push_str("=\"");
        out.push_str(&escape_attribute(value));
        out.push('"');
    }
    out.push('>');
}

fn is_void(tag: &str) -> bool {
    matches!(tag, "br" | "hr" | "img")
}

/// `serializeFragment`: marks nest as elements, shared across adjacent nodes
/// while the leading marks stay equal.
fn serialize_fragment(schema: &Schema, fragment: &Fragment, out: &mut String) {
    let mut active: Vec<(Mark, &'static str)> = Vec::new();
    for node in &fragment.children {
        if !active.is_empty() || !node.marks.is_empty() {
            let mut keep = 0;
            let mut rendered = 0;
            while keep < active.len() && rendered < node.marks.len() {
                if node.marks[rendered] != active[keep].0 {
                    break;
                }
                keep += 1;
                rendered += 1;
            }
            while keep < active.len() {
                let (_, tag) = active.pop().unwrap();
                out.push_str("</");
                out.push_str(tag);
                out.push('>');
            }
            while rendered < node.marks.len() {
                let mark = node.marks[rendered].clone();
                rendered += 1;
                let (tag, attrs) = mark_spec(schema, &mark);
                open_tag(out, tag, &attrs);
                active.push((mark, tag));
            }
        }
        serialize_node_inner(schema, node, out);
    }
    while let Some((_, tag)) = active.pop() {
        out.push_str("</");
        out.push_str(tag);
        out.push('>');
    }
}

fn serialize_node_inner(schema: &Schema, node: &Node, out: &mut String) {
    if let Some(text) = &node.text {
        escape_text(text, out);
        return;
    }
    let spec = node_spec(schema, node);
    open_tag(out, spec.tag, &spec.attrs);
    if is_void(spec.tag) {
        return;
    }
    if let Some(inner) = spec.inner {
        open_tag(out, inner, &[]);
    }
    if let Some(text) = &spec.text {
        escape_text(text, out);
    } else if spec.hole {
        serialize_fragment(schema, &node.content, out);
    }
    if let Some(inner) = spec.inner {
        out.push_str("</");
        out.push_str(inner);
        out.push('>');
    }
    out.push_str("</");
    out.push_str(spec.tag);
    out.push('>');
}

#[cfg(test)]
mod tests {
    use super::super::schema::schema;
    use super::*;

    fn doc(json: &str) -> Node {
        Node::from_json(schema(), &serde_json::from_str(json).unwrap()).unwrap()
    }

    #[test]
    fn serialises_slices_like_the_web_view() {
        let s = schema();
        let d = doc(r#"{"type":"doc","content":[
              {"type":"paragraph","content":[{"type":"text","text":"The "},{"type":"text","marks":[{"type":"bold"}],"text":"build"},{"type":"text","marks":[{"type":"bold"},{"type":"italic"}],"text":" passed"},{"type":"text","text":", see "},{"type":"text","marks":[{"type":"link","attrs":{"href":"https://x.y/z","target":null}}],"text":"it"},{"type":"text","text":" & <more>\u00a0"}]},
              {"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"a"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"b"}]}]}]}
            ]}"#);
        // The whole first paragraph's inline content (a text selection).
        let html = serialize_for_clipboard(s, &d.slice_at(1, 35, true));
        assert_eq!(
            html,
            r#"<p data-pm-slice="1 1 []">The <strong>build<em> passed</em></strong>, see <a href="https://x.y/z" rel="noopener noreferrer nofollow">it</a> &amp; &lt;more&gt;&nbsp;</p>"#
        );
        // From inside the first item's text to inside the second's: both
        // items open three deep, the list kept since it has two children.
        let list_content = d.child(0).node_size() + 1;
        let html =
            serialize_for_clipboard(s, &d.slice_at(list_content + 2, list_content + 8, true));
        assert_eq!(
            html,
            r#"<ul data-pm-slice="3 3 []"><li><p>a</p></li><li><p>b</p></li></ul>"#
        );
        // Within one item's text: the single-child chain is stripped into
        // the context the paste puts back.
        let html =
            serialize_for_clipboard(s, &d.slice_at(list_content + 2, list_content + 3, true));
        assert_eq!(
            html,
            r#"<p data-pm-slice="1 1 [&quot;bulletList&quot;,null,&quot;listItem&quot;,null]">a</p>"#
        );
        // Whole blocks copied: closed slice.
        let html = serialize_for_clipboard(s, &d.slice_at(0, d.content.size, true));
        assert!(html.starts_with(r#"<p data-pm-slice="0 0 []">The "#));
        assert!(html.ends_with("<ul><li><p>a</p></li><li><p>b</p></li></ul>"));
    }

    #[test]
    fn node_specs_follow_to_dom() {
        let s = schema();
        let d = doc(r#"{"type":"doc","content":[
              {"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"H"}]},
              {"type":"orderedList","attrs":{"start":3},"content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"x"}]}]}]},
              {"type":"codeBlock","content":[{"type":"text","text":"let a = 1;\n"}]},
              {"type":"taskList","content":[{"type":"taskItem","attrs":{"status":"done","checked":true,"taskId":"t","taskItemId":"i"},"content":[{"type":"paragraph","content":[{"type":"text","text":"done"}]}]}]},
              {"type":"image","attrs":{"src":"https://e.com/a.png","alt":null,"title":"cap","attachmentId":"image.png","sharedAttachmentId":null,"editorWidth":80}},
              {"type":"paragraph","content":[{"type":"text","text":"a"},{"type":"hardBreak"},{"type":"mention-@","attrs":{"id":"1","type":"session","label":"Seek"}}]},
              {"type":"horizontalRule"},
              {"type":"table","content":[{"type":"tableRow","content":[{"type":"tableCell","attrs":{"colspan":2,"rowspan":1,"colwidth":[100,50]},"content":[{"type":"paragraph","content":[{"type":"text","text":"c"}]}]}]}]}
            ]}"#);
        let html = serialize_for_clipboard(s, &d.slice_at(0, d.content.size, true));
        assert_eq!(
            html,
            concat!(
                r#"<h2 data-pm-slice="0 0 []">H</h2>"#,
                r#"<ol start="3" style="counter-reset: ol-counter 2"><li><p>x</p></li></ol>"#,
                "<pre><code>let a = 1;\n</code></pre>",
                r#"<ul data-type="taskList" class="task-list"><li data-type="taskItem" data-status="done" data-checked="true" data-task-id="t" data-task-item-id="i"><p>done</p></li></ul>"#,
                r#"<img src="https://e.com/a.png" title="cap" data-attachment-id="image.png" data-editor-width="80">"#,
                r#"<p>a<br><span class="mention" data-mention="true" data-id="1" data-type="session" data-label="Seek">Seek</span></p>"#,
                "<hr>",
                r#"<table><tbody><tr><td colspan="2" data-colwidth="100,50"><p>c</p></td></tr></tbody></table>"#,
            )
        );
        // A row's cells: the table strips into the context and the `<tr>` is
        // wrapped for `innerHTML`, the wrappers counted.
        let table_start: usize = d.content.children[..7].iter().map(Node::node_size).sum();
        let cell_html =
            serialize_for_clipboard(s, &d.slice_at(table_start + 2, table_start + 2 + 5, true));
        assert_eq!(
            cell_html,
            r#"<table data-pm-slice="1 1 -2 [&quot;table&quot;,null]"><tbody><tr><td colspan="2" data-colwidth="100,50"><p>c</p></td></tr></tbody></table>"#
        );
    }
}
