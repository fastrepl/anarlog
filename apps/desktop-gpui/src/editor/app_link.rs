//! Shipping's app-link chips (`editor-bridge/app-link-view.tsx` over `@anlg/editor/app-link`):
//! an inline bordered button with the provider's mark, or a GitHub issue or PR's done checkbox,
//! then the link's header and muted subline. Text rows have no inline elements, so the row
//! reserves each chip's width with no-break spaces and the chip paints over them.

use std::{collections::HashMap, sync::Arc};

use desktop_runtime::RuntimeHandle;
use gpui::{
    App, BorderStyle, Bounds, Context, CursorStyle, DispatchPhase, FontWeight, Global, Hitbox,
    HitboxBehavior, Hsla, ImgResourceLoader, IntoElement, MouseMoveEvent, Pixels, Point, Resource,
    ShapedLine, Styled, TextAlign, TextRun, TransformationMatrix, Window, canvas, point, px, quad,
    rgb, size,
};
use serde_json::Value;

use super::{
    EditorPane,
    document::NodeRef,
    surface::{Row, Span, VisibleLayout},
};
use crate::ui::theme::{MEDIUM, Theme};

const NO_BREAK_SPACE: &str = "\u{a0}";
const HEIGHT: f32 = 24.;

/// Shipping's `getAppLinkDisplayParts`: the chip's header and subline.
pub fn display_parts(attrs: &Value) -> (String, String) {
    let text = |key: &str| attrs[key].as_str().filter(|value| !value.is_empty());
    let titled = |label: &str| match text("resourceTitle") {
        Some(title) => format!("{label}: {title}"),
        None => label.into(),
    };
    let numbered = |label: &str| match text("resourceId") {
        Some(id) => format!("{label} {id}"),
        None => label.into(),
    };
    let workspace = |fallback: &str| text("workspace").unwrap_or(fallback).to_owned();
    let kind = text("kind").unwrap_or_default();
    match text("provider").unwrap_or("github") {
        "github" => {
            let number = attrs["number"].as_u64().unwrap_or_default();
            let sub = text("subId").unwrap_or_default();
            let subline = match kind {
                "issue" => format!("Issue #{number}"),
                "pull_request" => format!("PR #{number}"),
                "issue_comment" => format!("Comment on #{number}"),
                "pull_request_review_comment" => format!("Review on #{number}"),
                "discussion" => format!("Discussion #{number}"),
                "discussion_comment" => format!("Comment on Discussion #{number}"),
                "commit" => format!("Commit {}", sub.chars().take(7).collect::<String>()),
                "release" => format!("Release {sub}"),
                "action_run" => format!("Run {sub}"),
                "workflow" => format!("Workflow {sub}"),
                _ => String::new(),
            };
            (
                format!(
                    "{}/{}",
                    text("owner").unwrap_or_default(),
                    text("repo").unwrap_or_default()
                ),
                subline,
            )
        }
        "slack" => {
            let channel = text("channelId").unwrap_or_default();
            let subline = match kind {
                "message" => format!("Message in #{channel}"),
                "thread" => format!("Thread in #{channel}"),
                _ => format!("#{channel}"),
            };
            (workspace(""), subline)
        }
        "discord" => (
            "Discord".into(),
            match kind {
                "invite" => format!("Invite {}", text("inviteCode").unwrap_or_default()),
                "channel" => "Channel".into(),
                _ => "Message".into(),
            },
        ),
        "linear" => (
            workspace("Linear"),
            match kind {
                "issue" => numbered("Issue"),
                "workspace" => "Workspace".into(),
                "document" => titled("Document"),
                "initiative" => titled("Initiative"),
                "project" => titled("Project"),
                "route" => titled("Route"),
                "team" => titled("Team"),
                _ => titled("View"),
            },
        ),
        "notion" => (
            workspace("Notion"),
            match kind {
                "workspace" => "Workspace".into(),
                "database" => titled("Database"),
                _ => titled("Page"),
            },
        ),
        "google" => {
            let (header, subline) = match kind {
                "document" => ("Google Docs", "Document"),
                "spreadsheet" => ("Google Sheets", "Spreadsheet"),
                "presentation" => ("Google Slides", "Presentation"),
                "form" => ("Google Forms", "Form"),
                "folder" => ("Google Drive", "Folder"),
                _ => ("Google Drive", "File"),
            };
            (header.into(), subline.into())
        }
        "figma" => (
            "Figma".into(),
            titled(match kind {
                "board" => "FigJam board",
                "prototype" => "Prototype",
                "slides" => "Slides",
                _ => "Design file",
            }),
        ),
        "atlassian" => match kind {
            "jira_issue" => (
                workspace("Jira"),
                text("resourceId").map_or("Jira issue".into(), |id| format!("Jira {id}")),
            ),
            "confluence_space" => (
                workspace("Confluence"),
                text("resourceId").map_or("Confluence space".into(), |id| {
                    format!("Confluence space {id}")
                }),
            ),
            _ => (
                workspace("Confluence"),
                text("resourceTitle").map_or("Confluence page".into(), |title| {
                    format!("Confluence: {title}")
                }),
            ),
        },
        provider => {
            let header = match provider {
                "airtable" => "Airtable",
                "asana" => "Asana",
                "calendly" => "Calendly",
                "dropbox" => "Dropbox",
                "loom" => "Loom",
                "miro" => "Miro",
                "trello" => "Trello",
                "zoom" => "Zoom",
                _ => return (text("url").unwrap_or(provider).into(), String::new()),
            };
            let subline = match kind {
                "base" => "Base".into(),
                "board" => titled("Board"),
                "card" => titled("Card"),
                "event" => titled("Event"),
                "file" => titled("File"),
                "folder" => "Folder".into(),
                "meeting" => numbered("Meeting"),
                "project" => numbered("Project"),
                "recording" => "Recording".into(),
                "share" => "Shared view".into(),
                "table" => "Table".into(),
                "task" => numbered("Task"),
                "video" => "Video".into(),
                _ => "View".into(),
            };
            (header.into(), subline)
        }
    }
}

/// Shipping's `getAppLinkLabel`, the chip's text outside the editor.
pub fn label(attrs: &Value) -> String {
    let (header, subline) = display_parts(attrs);
    format!("{header} {subline}")
}

/// What leads the chip: shipping's `BrandIcon`, or the checkbox for GitHub issues and PRs.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mark {
    Checkbox,
    /// One-color marks: muted, in the chip's text color, or in the brand's color.
    Muted(&'static str),
    Text(&'static str),
    Brand(&'static str, u32),
    Color(&'static str),
}

fn mark(attrs: &Value) -> Mark {
    let kind = attrs["kind"].as_str().unwrap_or_default();
    match attrs["provider"].as_str().unwrap_or("github") {
        "github" if matches!(kind, "issue" | "pull_request") => Mark::Checkbox,
        "github" => Mark::Muted("editor/GithubLogo.svg"),
        "linear" => Mark::Muted("editor/LinearLogo.svg"),
        "notion" => Mark::Muted("editor/NotionLogo.svg"),
        // Lobehub's Google and Figma marks take the chip's text color.
        "google" => Mark::Text("editor/GoogleLogo.svg"),
        "figma" => Mark::Text("editor/FigmaLogo.svg"),
        "discord" => Mark::Brand("editor/DiscordLogo.svg", 0x5865f2),
        "loom" => Mark::Brand("editor/LoomLogo.svg", 0x625df5),
        "dropbox" => Mark::Brand("editor/DropboxLogo.svg", 0x0061ff),
        "slack" => Mark::Color("editor/SlackLogo.svg"),
        "atlassian" => Mark::Color("editor/AtlassianLogo.svg"),
        "asana" => Mark::Color("editor/AsanaLogo.svg"),
        "trello" => Mark::Color("editor/TrelloLogo.svg"),
        "airtable" => Mark::Color("editor/AirtableLogo.svg"),
        "miro" => Mark::Color("editor/MiroLogo.svg"),
        "zoom" => Mark::Color("editor/ZoomLogo.svg"),
        "calendly" => Mark::Color("editor/CalendlyLogo.svg"),
        _ => Mark::Muted("editor/GithubLogo.svg"),
    }
}

fn attrs(node: &NodeRef) -> &Value {
    node.fields.get("attrs").unwrap_or(&Value::Null)
}

/// The public API path for a GitHub issue or PR chip's state.
fn issue_path(node: &NodeRef) -> Option<String> {
    let attrs = attrs(node);
    if node.kind() != "appLink"
        || attrs["provider"] != "github"
        || !matches!(attrs["kind"].as_str(), Some("issue" | "pull_request"))
    {
        return None;
    }
    Some(format!(
        "/repos/{}/{}/issues/{}",
        attrs["owner"].as_str().filter(|owner| !owner.is_empty())?,
        attrs["repo"].as_str().filter(|repo| !repo.is_empty())?,
        attrs["number"].as_u64().filter(|number| *number > 0)?
    ))
}

/// GitHub issue and PR states by API path: `None` while loading or after a failure.
#[derive(Default)]
struct GithubStates(HashMap<String, Option<bool>>);

impl Global for GithubStates {}

/// Shipping's `useGitHubIssueState`: closed issues and closed or merged PRs show checked.
pub fn load_states<'a>(
    rows: impl IntoIterator<Item = &'a Arc<Row>>,
    runtime: &RuntimeHandle,
    cx: &mut Context<EditorPane>,
) {
    for path in rows
        .into_iter()
        .flat_map(|row| &row.spans)
        .filter_map(|span| issue_path(&span.node))
    {
        let states = &mut cx.default_global::<GithubStates>().0;
        if states.contains_key(&path) {
            continue;
        }
        states.insert(path.clone(), None);
        let github = crate::workspace::task::github(runtime, cx);
        cx.spawn(async move |pane, cx| {
            let done = github.get(path.clone()).await.ok().map(|issue| {
                issue["state"] == "closed" || !issue["pull_request"]["merged_at"].is_null()
            });
            let _ = pane.update(cx, |_, cx| {
                cx.default_global::<GithubStates>().0.insert(path, done);
                cx.notify();
            });
        })
        .detach();
    }
}

fn done(node: &NodeRef, cx: &App) -> bool {
    issue_path(node)
        .and_then(|path| cx.try_global::<GithubStates>()?.0.get(&path).copied())
        .flatten()
        .unwrap_or(false)
}

pub struct Chip {
    /// The atom's offset in its row.
    position: usize,
    units: usize,
    mark: Mark,
    done: bool,
    header: ShapedLine,
    subline: ShapedLine,
}

/// Replaces each app link's text in `row` with no-break spaces as wide as its chip, measured
/// with `run` at `font_size`; the chips paint over that space.
pub fn reserve(
    row: Arc<Row>,
    run: &TextRun,
    font_size: Pixels,
    colors: Theme,
    window: &Window,
    cx: &App,
) -> (Arc<Row>, Vec<Chip>) {
    if !row.spans.iter().any(|span| span.node.kind() == "appLink") {
        return (row, Vec::new());
    }
    let text_system = window.text_system().clone();
    let shape = |text: String, size: Pixels, weight: FontWeight, color: Hsla| {
        let mut font = run.font.clone();
        font.weight = weight;
        let len = text.len();
        text_system.shape_line(
            text.into(),
            size,
            &[TextRun {
                len,
                font,
                color,
                background_color: None,
                underline: None,
                strikethrough: None,
            }],
            None,
        )
    };
    let space = shape(
        NO_BREAK_SPACE.into(),
        font_size,
        run.font.weight,
        colors.foreground,
    )
    .width
    .max(px(1.));
    let mut text = String::new();
    let mut spans = Vec::with_capacity(row.spans.len());
    let mut chips = Vec::new();
    for span in &row.spans {
        let start = text.len();
        if span.node.kind() == "appLink" {
            let attrs = attrs(&span.node);
            let (header, subline) = display_parts(attrs);
            let chip = Chip {
                position: span.position,
                units: span.units,
                mark: mark(attrs),
                done: done(&span.node, cx),
                header: shape(header, px(14.), MEDIUM, colors.foreground),
                subline: shape(
                    subline,
                    px(12.),
                    FontWeight::NORMAL,
                    colors.muted_foreground,
                ),
            };
            let count = (f32::from(chip.width()) / f32::from(space)).round().max(1.) as usize;
            text.push_str(&NO_BREAK_SPACE.repeat(count));
            chips.push(chip);
        } else {
            text.push_str(&row.text[span.bytes.clone()]);
        }
        spans.push(Span {
            bytes: start..text.len(),
            ..span.clone()
        });
    }
    let row = Row {
        text: text.into(),
        spans,
        ..(*row).clone()
    };
    (Arc::new(row), chips)
}

impl Chip {
    /// `border`, `px-1.5`, the mark, then `gap-1.5` between the mark, header and subline.
    fn width(&self) -> Pixels {
        let mark = if self.mark == Mark::Checkbox {
            20.
        } else {
            14.
        };
        px(2. + 12. + mark + 6. + 6.) + self.header.width + self.subline.width
    }
}

/// The chip over the atom at `position..position + units` (row offsets) in `layout`.
fn bounds(layout: &VisibleLayout, position: usize, units: usize) -> Option<Bounds<Pixels>> {
    let start = layout.block_start + layout.row.start + position;
    let line = layout.bounds_for(start..start + units)?;
    let height = px(HEIGHT);
    Some(Bounds::new(
        point(
            line.left(),
            line.top() + (layout.layout.line_height() - height) / 2.,
        ),
        size(line.size.width, height),
    ))
}

/// The editor position of the app link chip under `point`, if any.
pub fn chip_at<'a>(
    layouts: impl IntoIterator<Item = &'a VisibleLayout>,
    point: Point<Pixels>,
) -> Option<usize> {
    layouts.into_iter().find_map(|layout| {
        layout
            .row
            .spans
            .iter()
            .filter(|span| span.node.kind() == "appLink")
            .find(|span| {
                bounds(layout, span.position, span.units)
                    .is_some_and(|bounds| bounds.contains(&point))
            })
            .map(|span| layout.block_start + layout.row.start + span.position)
    })
}

/// Paints `chips` over their reserved space in `layout`, with shipping's hover and pointer.
pub fn paint(layout: VisibleLayout, chips: Vec<Chip>, colors: Theme) -> impl IntoElement {
    canvas(
        move |_, window, _| {
            chips
                .into_iter()
                .filter_map(|chip| {
                    let bounds = bounds(&layout, chip.position, chip.units)?;
                    Some((window.insert_hitbox(bounds, HitboxBehavior::Normal), chip))
                })
                .collect::<Vec<_>>()
        },
        move |_, chips, window, cx| {
            for (hitbox, chip) in chips {
                paint_chip(&hitbox, &chip, colors, window, cx);
            }
        },
    )
    .absolute()
    .size_full()
    .top_0()
    .left_0()
}

fn paint_chip(hitbox: &Hitbox, chip: &Chip, colors: Theme, window: &mut Window, cx: &mut App) {
    let hovered = hitbox.is_hovered(window);
    window.set_cursor_style(CursorStyle::PointingHand, hitbox);
    window.on_mouse_event({
        let hitbox = hitbox.clone();
        move |_: &MouseMoveEvent, phase, window, _| {
            if phase == DispatchPhase::Bubble && hitbox.is_hovered(window) != hovered {
                window.refresh();
            }
        }
    });
    let bounds = hitbox.bounds;
    window.paint_quad(quad(
        bounds,
        px(0.),
        if hovered {
            colors.accent
        } else {
            colors.muted.opacity(0.35)
        },
        px(1.),
        colors.border.opacity(0.6),
        BorderStyle::Solid,
    ));
    let middle = bounds.center().y;
    let mut x = bounds.left() + px(7.);
    let square = |x: Pixels, side: f32| {
        Bounds::new(point(x, middle - px(side / 2.)), size(px(side), px(side)))
    };
    match chip.mark {
        Mark::Checkbox => {
            let (fill, border) = if chip.done {
                (colors.muted_foreground, colors.muted_foreground)
            } else {
                (colors.card, colors.border)
            };
            window.paint_quad(quad(
                square(x, 20.),
                px(0.),
                fill,
                px(2.),
                border,
                BorderStyle::Solid,
            ));
            if chip.done {
                let _ = window.paint_svg(
                    square(x + px(3.), 14.),
                    "editor/CheckIcon.svg".into(),
                    None,
                    TransformationMatrix::unit(),
                    colors.primary_foreground,
                    cx,
                );
            }
            x += px(20.);
        }
        Mark::Muted(path) | Mark::Text(path) | Mark::Brand(path, _) => {
            let color = match chip.mark {
                Mark::Brand(_, color) => rgb(color).into(),
                Mark::Text(_) => colors.foreground,
                _ => colors.muted_foreground,
            };
            let _ = window.paint_svg(
                square(x, 14.),
                path.into(),
                None,
                TransformationMatrix::unit(),
                color,
                cx,
            );
            x += px(14.);
        }
        Mark::Color(path) => {
            if let Some(Ok(image)) =
                window.use_asset::<ImgResourceLoader>(&Resource::Embedded(path.into()), cx)
            {
                let target = square(x, 14.);
                let _ = window.paint_image(target, target, Default::default(), image, 0, false);
            }
            x += px(14.);
        }
    }
    x += px(6.);
    // `items-baseline`: the subline sits on the header's baseline, the header's 14px line box
    // centred in the chip.
    let baseline = |line: &ShapedLine, height: Pixels| {
        (height - line.ascent - line.descent) / 2. + line.ascent
    };
    let top = middle - px(7.);
    let header_baseline = top + baseline(&chip.header, px(14.));
    let _ = chip
        .header
        .paint(point(x, top), px(14.), TextAlign::Left, None, window, cx);
    x += chip.header.width + px(6.);
    let _ = chip.subline.paint(
        point(x, header_baseline - baseline(&chip.subline, px(12.))),
        px(12.),
        TextAlign::Left,
        None,
        window,
        cx,
    );
}
