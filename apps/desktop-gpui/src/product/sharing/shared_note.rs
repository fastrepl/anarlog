//! Shipping's shared-note tabs (`shared-notes/index.tsx`): a note shared with the account,
//! read from the durable local cache, and the view-only preview behind a handoff link. The
//! body renders read-only with the note editor's typography.

use std::sync::Arc;

use desktop_runtime::{CancellationToken, RuntimeHandle, ServiceError};
use gpui::{
    Context, Div, FontStyle, FontWeight, Hsla, Render, SharedString, StrikethroughStyle,
    StyledText, TextRun, UnderlineStyle, Window, div, prelude::*, px,
};
use serde_json::{Value, json};

use super::{
    SharingCloud,
    popover::{button, icon},
};
use crate::{
    product::account,
    ui::theme::{Theme, monospace_font, theme},
};

const NOTE_SQL: &str = "SELECT title, body_json, capability, manage_access, attachments_json
    FROM shared_session_cache WHERE viewer_user_id = ? AND share_id = ?";

#[derive(Clone)]
struct Note {
    title: SharedString,
    subtitle: &'static str,
    icon: &'static str,
    blocks: Arc<[Block]>,
    attachments: Arc<[SharedString]>,
}

#[derive(Clone)]
enum State {
    Loading,
    SignedOut,
    Unavailable {
        icon: &'static str,
        title: &'static str,
        description: &'static str,
    },
    Ready(Note),
}

pub struct SharedNoteView {
    runtime: RuntimeHandle,
    id: String,
    preview: bool,
    state: State,
    signing_in: bool,
    viewer: Option<Arc<str>>,
    cancel: CancellationToken,
    _account: gpui::Subscription,
}

impl SharedNoteView {
    /// `preview` is a handoff link's request id; otherwise `id` is a share in the account's
    /// cache.
    pub fn new(runtime: RuntimeHandle, id: String, preview: bool, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            runtime,
            id,
            preview,
            state: State::Loading,
            signing_in: false,
            viewer: None,
            cancel: CancellationToken::new(),
            _account: cx.observe_global::<account::Account>(|this, cx| {
                if !this.preview && account_id(cx) != this.viewer {
                    this.load(cx);
                }
            }),
        };
        this.load(cx);
        this
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        self.cancel.cancel();
        self.cancel = CancellationToken::new();
        let cancel = self.cancel.clone();
        let id = self.id.clone();
        if self.preview {
            self.state = State::Loading;
            let Some(cloud) = cx.try_global::<SharingCloud>().map(|cloud| cloud.0.clone()) else {
                self.state = preview_unavailable();
                return;
            };
            cx.spawn(async move |this, cx| {
                let result = cloud.claim_preview(id, cancel.clone()).await;
                if cancel.is_cancelled() {
                    return;
                }
                let _ = this.update(cx, |this, cx| {
                    this.state = match result {
                        Ok(snapshot) => State::Ready(Note {
                            blocks: blocks(&snapshot.body, &snapshot.title).into(),
                            title: snapshot.title.into(),
                            subtitle: "Shared link · View only",
                            icon: "sharing/LinkIcon.svg",
                            attachments: Arc::from([]),
                        }),
                        Err(_) => preview_unavailable(),
                    };
                    cx.notify();
                });
            })
            .detach();
            return;
        }
        self.viewer = account_id(cx);
        let Some(viewer) = self.viewer.clone() else {
            self.state = State::SignedOut;
            cx.notify();
            return;
        };
        self.state = State::Loading;
        let reply = self
            .runtime
            .read(cancel.clone(), move |services| async move {
                services
                    .executor
                    .execute(NOTE_SQL.into(), vec![json!(viewer), json!(id)])
                    .await
                    .map_err(|error| ServiceError::Failed(error.to_string().into()))
            });
        cx.spawn(async move |this, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            if cancel.is_cancelled() {
                return;
            }
            let _ = this.update(cx, |this, cx| {
                this.state = match result.map(|rows| rows.into_iter().next()) {
                    Ok(Some(row)) => cached_note(&row).unwrap_or_else(cache_unreadable),
                    Ok(None) => State::Unavailable {
                        icon: "sharing/UsersIcon.svg",
                        title: "Access no longer available",
                        description: "The note may have been unshared or moved out of a workspace you can access.",
                    },
                    Err(_) => cache_unreadable(),
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn sign_in(&mut self, cx: &mut Context<Self>) {
        let Some(cloud) = cx.try_global::<SharingCloud>().map(|cloud| cloud.0.clone()) else {
            return;
        };
        self.signing_in = true;
        cx.spawn(async move |this, cx| {
            let _ = cloud.sign_in().await;
            let _ = this.update(cx, |this, cx| {
                this.signing_in = false;
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

impl Drop for SharedNoteView {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

fn account_id(cx: &gpui::App) -> Option<Arc<str>> {
    account::current(cx).user.map(|user| user.id)
}

fn preview_unavailable() -> State {
    State::Unavailable {
        icon: "sharing/AlertCircleIcon.svg",
        title: "Shared note unavailable",
        description: "The link may have expired or its access may have changed.",
    }
}

fn cache_unreadable() -> State {
    State::Unavailable {
        icon: "sharing/AlertCircleIcon.svg",
        title: "Shared note unavailable",
        description: "Anarlog could not read the local shared-note cache.",
    }
}

fn cached_note(row: &Value) -> Option<State> {
    let title = row["title"].as_str()?.to_owned();
    let body: Value = serde_json::from_str(row["body_json"].as_str()?).ok()?;
    let manage = row["manage_access"].as_i64() == Some(1);
    let subtitle = match (manage, row["capability"].as_str()?) {
        (true, _) => "Shared note · Owner",
        (false, "editor") => "Shared with me · Can edit",
        (false, "commenter") => "Shared with me · Can comment",
        _ => "Shared with me · View only",
    };
    let attachments: Vec<Value> =
        serde_json::from_str(row["attachments_json"].as_str().unwrap_or("[]")).ok()?;
    let referenced = referenced_attachments(&body);
    Some(State::Ready(Note {
        blocks: blocks(&body, &title).into(),
        title: title.into(),
        subtitle,
        icon: "sharing/UsersIcon.svg",
        attachments: attachments
            .iter()
            .filter(|attachment| {
                attachment["id"]
                    .as_str()
                    .is_some_and(|id| !referenced.iter().any(|seen| seen == id))
            })
            .filter_map(|attachment| {
                attachment["filename"]
                    .as_str()
                    .map(|name| name.to_owned().into())
            })
            .collect(),
    }))
}

fn referenced_attachments(node: &Value) -> Vec<String> {
    let mut ids = Vec::new();
    let mut stack = vec![node];
    while let Some(node) = stack.pop() {
        if let Some(id) = node["attrs"]["sharedAttachmentId"].as_str() {
            ids.push(id.to_owned());
        }
        if let Some(children) = node["content"].as_array() {
            stack.extend(children);
        }
    }
    ids
}

impl Render for SharedNoteView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let surface = div()
            .id("shared-note")
            .size_full()
            .flex()
            .flex_col()
            .text_color(colors.foreground);
        match self.state.clone() {
            State::Loading => surface.child(skeleton(colors)),
            State::SignedOut => surface.child(unavailable(
                "sharing/Login03Icon.svg",
                "Sign in to view this shared note",
                "Shared notes are tied to the account they were shared with.",
                Some(
                    button("shared-note-sign-in", true, !self.signing_in, colors)
                        .mt_4()
                        .h(px(36.))
                        .px_4()
                        .text_sm()
                        .line_height(px(20.))
                        .when(!self.signing_in, |b| {
                            b.on_click(cx.listener(|this, _, _, cx| this.sign_in(cx)))
                        })
                        .child(if self.signing_in {
                            "Opening…"
                        } else {
                            "Sign in"
                        })
                        .into_any_element(),
                ),
                colors,
            )),
            State::Unavailable {
                icon,
                title,
                description,
            } => surface.child(unavailable(icon, title, description, None, colors)),
            State::Ready(note) => surface
                .child(
                    div().px_1().child(
                        div()
                            .h(px(48.))
                            .min_w_0()
                            .px_3()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(icon(note.icon, 16., colors.muted_foreground))
                            .child(
                                div()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .truncate()
                                            .text_sm()
                                            .line_height(px(20.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(if note.title.is_empty() {
                                                SharedString::from("Untitled")
                                            } else {
                                                note.title.clone()
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .line_height(px(16.))
                                            .text_color(colors.muted_foreground)
                                            .child(note.subtitle),
                                    ),
                            ),
                    ),
                )
                .child(
                    div()
                        .id("shared-note-body")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .px(px(20.))
                        .pt_2()
                        .pb_6()
                        .children(
                            note.blocks
                                .iter()
                                .map(|block| render_block(block, colors, window, cx)),
                        )
                        .when(!note.attachments.is_empty(), |view| {
                            view.child(attachment_list(&note.attachments, colors))
                        }),
                ),
        }
    }
}

fn skeleton(colors: Theme) -> Div {
    let bar = |width: f32, height: f32, opacity: f32| {
        div()
            .h(px(height))
            .w(gpui::relative(width))
            .rounded(px(6.))
            .bg(colors.muted.opacity(opacity))
    };
    div()
        .size_full()
        .px_4()
        .py_5()
        .flex()
        .flex_col()
        .gap_3()
        .child(bar(0.6, 20., 1.))
        .child(bar(0.8, 16., 0.8))
        .child(bar(2. / 3., 16., 0.7))
}

fn unavailable(
    glyph: &'static str,
    title: &'static str,
    description: &'static str,
    action: Option<gpui::AnyElement>,
    colors: Theme,
) -> Div {
    div()
        .size_full()
        .px_6()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .max_w(px(384.))
                .flex()
                .flex_col()
                .items_center()
                .text_center()
                .child(icon(glyph, 24., colors.muted_foreground).mb_3())
                .child(
                    div()
                        .text_sm()
                        .line_height(px(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(title),
                )
                .child(
                    div()
                        .mt_1()
                        .text_sm()
                        .line_height(px(20.))
                        .text_color(colors.muted_foreground)
                        .child(description),
                )
                .children(action),
        )
}

/// `SharedAttachmentList`: attachments the body does not place inline. The native app cannot
/// download shared attachments yet, so they show as unavailable.
fn attachment_list(attachments: &[SharedString], colors: Theme) -> impl IntoElement {
    div()
        .mt_8()
        .pt_5()
        .border_t_1()
        .border_color(colors.border.opacity(0.6))
        .child(
            div()
                .mb_2()
                .text_sm()
                .line_height(px(20.))
                .font_weight(FontWeight::MEDIUM)
                .child("Attachments"),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .children(attachments.iter().map(|name| {
                    div()
                        .px_3()
                        .py_3()
                        .flex()
                        .items_center()
                        .gap_3()
                        .rounded(px(12.))
                        .border_1()
                        .border_color(colors.border.opacity(0.6))
                        .text_color(colors.muted_foreground)
                        .child(icon(
                            "sharing/PaperclipIcon.svg",
                            16.,
                            colors.muted_foreground,
                        ))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_sm()
                                .line_height(px(20.))
                                .child(name.clone()),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_xs()
                                .line_height(px(16.))
                                .child("Unavailable"),
                        )
                })),
        )
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Marks {
    bold: bool,
    italic: bool,
    code: bool,
    link: bool,
    strike: bool,
    underline: bool,
    highlight: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum Marker {
    None,
    /// Filled squircle, hollow squircle, small square, cycling with bullet depth.
    Bullet(usize),
    Ordered(String),
    Task(bool),
}

#[derive(Clone, Debug, PartialEq)]
struct Block {
    /// `paragraph`, `heading`, `codeBlock`, `horizontalRule`.
    kind: &'static str,
    level: u64,
    indent: f32,
    marker: Marker,
    quotes: Vec<f32>,
    /// Inside a done task item: shipping fades the item and strikes its text.
    done: bool,
    spans: Vec<(String, Marks)>,
}

#[derive(Clone, Default)]
struct Style {
    depth: usize,
    lists: usize,
    bullets: usize,
    ordered: usize,
    quotes: Vec<f32>,
    marker: Option<Marker>,
    done: bool,
}

impl Style {
    fn indent(&self) -> f32 {
        (self.depth * 20 + self.lists * 4) as f32
    }
}

/// `ensureFirstLineTitle`, then the document as rows in the note editor's layout.
fn blocks(body: &Value, title: &str) -> Vec<Block> {
    let mut content = body["content"].as_array().cloned().unwrap_or_default();
    let title = title.trim();
    if !title.is_empty() {
        let heading =
            json!({"type":"heading","attrs":{"level":1},"content":[{"type":"text","text":title}]});
        let first = content.first();
        let is_h1 =
            first.is_some_and(|block| block["type"] == "heading" && block["attrs"]["level"] == 1);
        let first_text = first.map(inline_text).unwrap_or_default();
        if (is_h1 || first.is_some_and(|block| block["type"] == "paragraph"))
            && first_text.trim() == title
        {
            if !is_h1 {
                content[0] = heading;
            }
        } else if is_h1 && first_text.trim().is_empty() {
            content[0] = heading;
        } else {
            content.insert(0, heading);
        }
    }
    let mut out = Vec::new();
    for node in &content {
        flatten(node, &Style::default(), &mut out);
    }
    out
}

fn inline_text(node: &Value) -> String {
    let mut text = String::new();
    for child in node["content"].as_array().into_iter().flatten() {
        match child["type"].as_str() {
            Some("text") => text.push_str(child["text"].as_str().unwrap_or_default()),
            _ => text.push_str(&inline_text(child)),
        }
    }
    text
}

fn spans(node: &Value) -> Vec<(String, Marks)> {
    let mut spans = Vec::new();
    for child in node["content"].as_array().into_iter().flatten() {
        let mut marks = Marks::default();
        for mark in child["marks"].as_array().into_iter().flatten() {
            match mark["type"].as_str().unwrap_or_default() {
                "bold" => marks.bold = true,
                "italic" => marks.italic = true,
                "code" => marks.code = true,
                "link" => marks.link = true,
                "strike" => marks.strike = true,
                "underline" => marks.underline = true,
                "highlight" => marks.highlight = true,
                _ => {}
            }
        }
        let text = match child["type"].as_str().unwrap_or_default() {
            "text" => child["text"].as_str().unwrap_or_default().to_owned(),
            "hardBreak" => "\n".to_owned(),
            "mention" | "mention-@" => {
                format!("@{}", child["attrs"]["label"].as_str().unwrap_or_default())
            }
            _ => inline_text(child),
        };
        if !text.is_empty() {
            spans.push((text, marks));
        }
    }
    spans
}

fn flatten(node: &Value, style: &Style, out: &mut Vec<Block>) {
    let kind = node["type"].as_str().unwrap_or_default();
    let block = |kind: &'static str, level: u64, spans: Vec<(String, Marks)>| Block {
        kind,
        level,
        indent: style.indent(),
        marker: style.marker.clone().unwrap_or(Marker::None),
        quotes: style.quotes.clone(),
        done: style.done,
        spans,
    };
    match kind {
        "heading" => out.push(block(
            "heading",
            node["attrs"]["level"].as_u64().unwrap_or(1),
            spans(node),
        )),
        "codeBlock" => out.push(block(
            "codeBlock",
            0,
            vec![(inline_text(node), Marks::default())],
        )),
        "horizontalRule" => out.push(block("horizontalRule", 0, Vec::new())),
        "image" | "fileAttachment" => {
            let name = node["attrs"]["name"]
                .as_str()
                .or(node["attrs"]["alt"].as_str())
                .unwrap_or("Attachment");
            out.push(block(
                "paragraph",
                0,
                vec![(
                    name.to_owned(),
                    Marks {
                        italic: true,
                        ..Marks::default()
                    },
                )],
            ));
        }
        "bulletList" | "orderedList" | "taskList" => {
            let mut child = style.clone();
            child.depth += 1;
            child.lists += 1;
            child.marker = None;
            if kind == "bulletList" {
                child.bullets += 1;
            }
            if kind == "orderedList" {
                child.ordered += 1;
            }
            let start = node["attrs"]["start"].as_u64().unwrap_or(1);
            for (index, item) in node["content"].as_array().into_iter().flatten().enumerate() {
                let mut item_style = child.clone();
                let marker = match kind {
                    "bulletList" => Marker::Bullet((child.bullets - 1) % 3),
                    "orderedList" => {
                        Marker::Ordered(ordered_marker(start + index as u64, child.ordered))
                    }
                    _ => Marker::Task(item["attrs"]["checked"].as_bool() == Some(true)),
                };
                item_style.done = style.done || item["attrs"]["checked"].as_bool() == Some(true);
                for (position, part) in item["content"].as_array().into_iter().flatten().enumerate()
                {
                    item_style.marker = (position == 0).then(|| marker.clone());
                    flatten(part, &item_style, out);
                }
            }
        }
        "blockquote" => {
            let mut child = style.clone();
            child.quotes.push(style.indent());
            child.depth += 1;
            for (position, part) in node["content"].as_array().into_iter().flatten().enumerate() {
                child.marker = if position == 0 {
                    style.marker.clone()
                } else {
                    None
                };
                flatten(part, &child, out);
            }
        }
        "paragraph" => out.push(block("paragraph", 0, spans(node))),
        _ => match node["content"].as_array() {
            Some(children) if children.iter().any(|child| child["type"] != "text") => {
                for part in children {
                    flatten(part, style, out);
                }
            }
            _ => out.push(block("paragraph", 0, spans(node))),
        },
    }
}

/// Shipping cycles ordered markers by depth: decimal, lower-alpha, lower-roman.
fn ordered_marker(number: u64, depth: usize) -> String {
    match depth.saturating_sub(1) % 3 {
        1 => {
            let mut letters = Vec::new();
            let mut rest = number.max(1);
            while rest > 0 {
                rest -= 1;
                letters.push(b'a' + (rest % 26) as u8);
                rest /= 26;
            }
            letters.reverse();
            format!("{}.", String::from_utf8(letters).unwrap_or_default())
        }
        2 if (1..4000).contains(&number) => {
            let mut rest = number;
            let mut roman = String::new();
            for (value, digits) in [
                (1000, "m"),
                (900, "cm"),
                (500, "d"),
                (400, "cd"),
                (100, "c"),
                (90, "xc"),
                (50, "l"),
                (40, "xl"),
                (10, "x"),
                (9, "ix"),
                (5, "v"),
                (4, "iv"),
                (1, "i"),
            ] {
                while rest >= value {
                    roman.push_str(digits);
                    rest -= value;
                }
            }
            format!("{roman}.")
        }
        _ => format!("{number}."),
    }
}

fn render_block(block: &Block, colors: Theme, window: &Window, cx: &gpui::App) -> gpui::AnyElement {
    let (font_size, line_height) = match (block.kind, block.level) {
        ("heading", 1) => (20., 28.),
        ("heading", 2) => (18., 26.),
        ("heading", _) => (16., 24.),
        ("codeBlock", _) => (14., 20.),
        _ => (16., 24.),
    };
    let mut base = window.text_style().to_run(0);
    base.color = if block.quotes.is_empty() {
        colors.foreground
    } else {
        colors.muted_foreground
    };
    if block.done {
        base.color.a *= 0.5;
    }
    if block.kind == "heading" {
        base.font.weight = if block.level == 1 {
            FontWeight::BOLD
        } else {
            FontWeight::SEMIBOLD
        };
    }
    if block.kind == "codeBlock" {
        base.font.family = monospace_font(cx);
    }
    let mut text = String::new();
    let mut runs = Vec::new();
    for (span, marks) in &block.spans {
        text.push_str(span);
        runs.push(run(&base, span.len(), *marks, block.done, colors, cx));
    }
    if text.is_empty() {
        text.push(' ');
        runs.push(TextRun {
            len: 1,
            ..base.clone()
        });
    }
    let marker_color = colors.foreground.opacity(0.65);
    let marker_left = block.indent - 24.;
    let row = div()
        .relative()
        .w_full()
        .min_h(px(line_height))
        .text_size(px(font_size))
        .line_height(px(line_height));
    if block.kind == "horizontalRule" {
        return row
            .min_h(px(5.))
            .py(px(2.))
            .child(div().ml(px(block.indent)).h(px(1.)).bg(colors.border))
            .into_any_element();
    }
    row.map(|row| {
        if block.kind == "codeBlock" {
            row.my_2()
                .ml(px(block.indent))
                .py(px(14.))
                .px(px(10.5))
                .bg(colors.muted)
                .rounded(px(6.))
        } else {
            row.py(px(font_size * 0.125)).pl(px(block.indent))
        }
    })
    .children(block.quotes.iter().map(|offset| {
        div()
            .absolute()
            .left(px(*offset))
            .top_0()
            .bottom_0()
            .w(px(3.))
            .bg(colors.border)
    }))
    .map(|row| match &block.marker {
        Marker::None => row,
        Marker::Bullet(variant) => {
            let size = font_size * if *variant == 2 { 0.42 } else { 0.5 };
            row.child(
                div()
                    .absolute()
                    .left(px(marker_left + font_size * 0.5 - size / 2.))
                    .top(px(font_size * 0.875 - size / 2.))
                    .size(px(size))
                    .rounded(px(if *variant == 2 {
                        font_size * 0.06
                    } else {
                        size * 0.3
                    }))
                    .when_else(
                        *variant == 1,
                        |view| view.border(px(1.5)).border_color(marker_color),
                        |view| view.bg(marker_color),
                    ),
            )
        }
        Marker::Ordered(label) => row.child(
            div()
                .absolute()
                .left(px(marker_left))
                .top(px(font_size * 0.125))
                .w(px(font_size))
                .text_center()
                .text_color(marker_color)
                .child(label.clone()),
        ),
        Marker::Task(done) => row.child(
            div()
                .absolute()
                .left(px(marker_left))
                .top(px(font_size * 0.375))
                .size(px(font_size))
                .rounded(px(font_size * 0.22))
                .border(px(1.5))
                .when_else(
                    *done,
                    |view| {
                        view.bg(colors.foreground)
                            .border_color(colors.foreground)
                            .child(
                                gpui::svg()
                                    .path("editor/TaskCheck.svg")
                                    .absolute()
                                    .top(px(-1.5))
                                    .left(px(-1.5))
                                    .size(px(font_size))
                                    .text_color(colors.background),
                            )
                    },
                    |view| view.border_color(marker_color),
                ),
        ),
    })
    .child(StyledText::new(text).with_runs(runs))
    .into_any_element()
}

fn run(
    base: &TextRun,
    len: usize,
    marks: Marks,
    done: bool,
    colors: Theme,
    cx: &gpui::App,
) -> TextRun {
    let mut run = TextRun {
        len,
        ..base.clone()
    };
    if marks.bold {
        run.font.weight = FontWeight::SEMIBOLD;
    }
    if marks.italic {
        run.font.style = FontStyle::Italic;
    }
    if marks.code {
        run.font.family = monospace_font(cx);
        run.background_color = Some(colors.muted);
    }
    if marks.link || marks.underline {
        run.underline = Some(UnderlineStyle {
            thickness: px(1.),
            color: None,
            wavy: false,
        });
    }
    if marks.link {
        run.color = gpui::rgb(0x2563eb).into();
    }
    if marks.strike || done {
        run.strikethrough = Some(StrikethroughStyle {
            thickness: px(1.),
            color: None,
        });
    }
    if marks.highlight {
        run.background_color = Some(Hsla::from(gpui::rgb(0xfef08a)));
    }
    run
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn text(block: &Block) -> String {
        block.spans.iter().map(|(text, _)| text.as_str()).collect()
    }

    #[test]
    fn the_title_leads_the_document_once() {
        let body = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Body"}]}]});
        let rows = blocks(&body, "Roadmap");
        assert_eq!(
            (rows[0].kind, rows[0].level, text(&rows[0])),
            ("heading", 1, "Roadmap".into())
        );
        assert_eq!(text(&rows[1]), "Body");
        // A first paragraph that already is the title becomes the heading instead.
        let titled = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Roadmap"}]}]});
        assert_eq!(blocks(&titled, " Roadmap ").len(), 1);
        let empty = json!({"type":"doc","content":[{"type":"heading","attrs":{"level":1}}]});
        assert_eq!(text(&blocks(&empty, "Roadmap")[0]), "Roadmap");
        assert_eq!(blocks(&titled, "").len(), 1);
    }

    #[test]
    fn lists_quotes_and_marks_follow_the_note_editor() {
        let body = json!({"type":"doc","content":[
            {"type":"bulletList","content":[{"type":"listItem","content":[
                {"type":"paragraph","content":[{"type":"text","text":"One "},{"type":"text","text":"bold","marks":[{"type":"bold"}]}]},
                {"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Nested"}]}]}]}
            ]}]},
            {"type":"orderedList","attrs":{"start":3},"content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"Third"}]}]}]},
            {"type":"taskList","content":[{"type":"taskItem","attrs":{"checked":true},"content":[{"type":"paragraph","content":[{"type":"text","text":"Done"}]}]}]},
            {"type":"blockquote","content":[{"type":"paragraph","content":[{"type":"text","text":"Quote"},{"type":"hardBreak"},{"type":"text","text":"line"}]}]}
        ]});
        let rows = blocks(&body, "");
        assert_eq!(rows[0].marker, Marker::Bullet(0));
        assert_eq!(rows[0].indent, 24.);
        assert!(rows[0].spans[1].1.bold);
        assert_eq!(
            (rows[1].marker.clone(), rows[1].indent),
            (Marker::Bullet(1), 48.)
        );
        assert_eq!(rows[2].marker, Marker::Ordered("3.".into()));
        assert!(rows[3].done && rows[3].marker == Marker::Task(true));
        assert_eq!(
            (rows[4].quotes.clone(), text(&rows[4])),
            (vec![0.], "Quote\nline".into())
        );
        assert_eq!(ordered_marker(28, 2), "ab.");
        assert_eq!(ordered_marker(4, 3), "iv.");
    }

    #[test]
    fn cached_rows_describe_the_viewers_access() {
        let row = |capability: &str, manage: i64| {
            json!({"title":"Roadmap","body_json":"{\"type\":\"doc\",\"content\":[]}","capability":capability,
                "manage_access":manage,"attachments_json":"[{\"id\":\"a\",\"filename\":\"deck.pdf\"}]"})
        };
        let subtitle = |row: &Value| match cached_note(row) {
            Some(State::Ready(note)) => (note.subtitle, note.attachments.len()),
            _ => panic!("unreadable"),
        };
        assert_eq!(
            subtitle(&row("viewer", 0)),
            ("Shared with me · View only", 1)
        );
        assert_eq!(
            subtitle(&row("commenter", 0)).0,
            "Shared with me · Can comment"
        );
        assert_eq!(subtitle(&row("editor", 0)).0, "Shared with me · Can edit");
        assert_eq!(subtitle(&row("viewer", 1)).0, "Shared note · Owner");
        assert!(
            cached_note(&json!({"title":"x","body_json":"not json","capability":"viewer"}))
                .is_none()
        );
    }
}
