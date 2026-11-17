//! Shipping's changelog tab (`changelog/{index,data,source}.tsx`): "What's new in X?", the
//! release notes styled like `@anlg/changelog`, external links and a close button.

use std::{ops::Range, sync::Arc};

use futures::future::BoxFuture;
use gpui::{
    AnyElement, Context, FontStyle, FontWeight, HighlightStyle, Hsla, InteractiveText, Render,
    SharedString, StrikethroughStyle, StyledText, UnderlineStyle, WeakEntity, Window, div,
    prelude::*, px, rgb, rgba,
};

use super::shell::WorkspaceView;
use crate::{
    chat::markdown::{self, Block, Inline},
    ui::theme::{Theme, monospace_font, theme},
};

/// The newest `packages/changelog/content/desktop/<version>.md`, bundled by `build.rs`.
pub const BUNDLED_VERSION: &str = env!("ANLG_CHANGELOG_VERSION");
const BUNDLED: &str = include_str!(concat!(env!("OUT_DIR"), "/changelog.md"));

/// The release this build is: its `VITE_APP_VERSION`, else the newest bundled notes, as
/// shipping's changelog plugin picks `latestVersion`.
pub fn current_version() -> &'static str {
    option_env!("VITE_APP_VERSION")
        .filter(|version| !version.is_empty())
        .unwrap_or(BUNDLED_VERSION)
}

/// Release notes for a version that is not bundled: the raw file or GitHub release body,
/// `None` when there is none. Tests pass a fixture; the app fetches `url`.
pub type Fetch = Arc<dyn Fn(String) -> BoxFuture<'static, Option<String>> + Send + Sync>;

fn semver(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

/// `changelog/source.ts`.
pub fn url(version: &str) -> Option<String> {
    if let Some((release, build)) = version.split_once("-nightly.")
        && semver(release)
        && !build.is_empty()
        && build.bytes().all(|b| b.is_ascii_digit())
    {
        return Some(format!(
            "https://api.github.com/repos/fastrepl/anarlog/releases/tags/desktop_nightly_v{version}"
        ));
    }
    semver(version).then(|| {
        format!(
            "https://raw.githubusercontent.com/fastrepl/anarlog/main/packages/changelog/content/desktop/{version}.md"
        )
    })
}

/// Fetches `url(version)` on the runtime's service executor.
pub fn http(runtime: desktop_runtime::RuntimeHandle) -> Fetch {
    Arc::new(move |version: String| {
        let reply = runtime.service(move |_| async move {
            let Some(url) = url(&version) else {
                return Ok(None);
            };
            let response = reqwest::Client::new()
                .get(url)
                .header("User-Agent", "Anarlog")
                .send()
                .await
                .ok()
                .filter(|response| response.status().is_success());
            let Some(response) = response else {
                return Ok(None);
            };
            if version.contains("-nightly.") {
                let release: serde_json::Value = response.json().await.unwrap_or_default();
                return Ok(release["body"].as_str().map(str::to_owned));
            }
            Ok(response.text().await.ok())
        });
        Box::pin(async move {
            match reply {
                Ok(reply) => reply.receive().await.ok().flatten(),
                Err(_) => None,
            }
        })
    })
}

/// `processContent`: the notes without their frontmatter.
pub fn body(raw: &str) -> &str {
    let trimmed = raw.trim();
    let Some(rest) = trimmed.strip_prefix("---") else {
        return trimmed;
    };
    let rest = rest.trim_start_matches([' ', '\t']);
    let Some(rest) = rest.strip_prefix('\n') else {
        return trimmed;
    };
    match rest.find("\n---") {
        Some(end)
            if rest[end + 4..]
                .trim_start_matches([' ', '\t'])
                .starts_with('\n') =>
        {
            rest[end + 4..].trim()
        }
        _ => trimmed,
    }
}

enum Piece {
    Markdown(Vec<Block>),
    Banner {
        title: Option<String>,
        variant: String,
        blocks: Vec<Block>,
    },
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    let start = tag.find(&format!("{name}=\""))? + name.len() + 2;
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].to_owned())
}

/// Markdown with `@anlg/changelog`'s `<banner title variant>` blocks split out.
fn pieces(content: &str) -> Vec<Piece> {
    let mut pieces = Vec::new();
    let mut rest = content;
    while let Some(start) = rest.find("<banner") {
        let Some(open_end) = rest[start..].find('>').map(|end| start + end) else {
            break;
        };
        let Some(close) = rest[open_end..].find("</banner>").map(|end| open_end + end) else {
            break;
        };
        pieces.push(Piece::Markdown(markdown::parse(&rest[..start])));
        let tag = &rest[start..open_end];
        pieces.push(Piece::Banner {
            title: attribute(tag, "title"),
            variant: attribute(tag, "variant").unwrap_or_else(|| "default".into()),
            blocks: markdown::parse(rest[open_end + 1..close].trim()),
        });
        rest = &rest[close + "</banner>".len()..];
    }
    pieces.push(Piece::Markdown(markdown::parse(rest)));
    pieces
}

enum Content {
    Loading,
    Ready(Option<Arc<Vec<Piece>>>),
}

pub struct ChangelogView {
    version: SharedString,
    content: Content,
    workspace: WeakEntity<WorkspaceView>,
    sidebar_expanded: bool,
}

impl ChangelogView {
    pub fn new(
        version: SharedString,
        fetch: Fetch,
        workspace: WeakEntity<WorkspaceView>,
        cx: &mut Context<Self>,
    ) -> Self {
        let content = if version.as_ref() == BUNDLED_VERSION && !BUNDLED.trim().is_empty() {
            Content::Ready(Some(Arc::new(pieces(body(BUNDLED)))))
        } else {
            let request = fetch(version.to_string());
            cx.spawn(async move |this, cx| {
                let raw = request.await;
                let _ = this.update(cx, |this, cx| {
                    this.content = Content::Ready(
                        raw.filter(|raw| !body(raw).is_empty())
                            .map(|raw| Arc::new(pieces(body(&raw)))),
                    );
                    cx.notify();
                });
            })
            .detach();
            Content::Loading
        };
        Self {
            version,
            content,
            workspace,
            sidebar_expanded: true,
        }
    }

    pub fn set_sidebar_expanded(&mut self, expanded: bool) {
        self.sidebar_expanded = expanded;
    }

    #[cfg(test)]
    fn loaded(&self) -> Option<bool> {
        match &self.content {
            Content::Loading => None,
            Content::Ready(content) => Some(content.is_some()),
        }
    }
}

struct Palette {
    colors: Theme,
    dark: bool,
    mono: SharedString,
}

impl Palette {
    /// `text-[#374151] dark:text-[#e7e5e4]`.
    fn strong(&self) -> Hsla {
        if self.dark {
            rgb(0xe7e5e4).into()
        } else {
            rgb(0x374151).into()
        }
    }

    /// `text-blue-600 dark:text-blue-400`.
    fn link(&self) -> Hsla {
        if self.dark {
            rgb(0x51a2ff).into()
        } else {
            rgb(0x155dfc).into()
        }
    }
}

fn styled(inline: &Inline, id: String, base: Hsla, palette: &Palette) -> AnyElement {
    let highlights = inline
        .spans
        .iter()
        .map(|span| {
            let mut style = HighlightStyle::default();
            if span.bold {
                style.font_weight = Some(FontWeight::SEMIBOLD);
                style.color = Some(palette.strong());
            }
            if span.italic {
                style.font_style = Some(FontStyle::Italic);
            }
            if span.strike {
                style.strikethrough = Some(StrikethroughStyle {
                    thickness: px(1.),
                    color: None,
                });
            }
            if span.code {
                style.color = Some(if palette.dark {
                    rgb(0xe7e5e4).into()
                } else {
                    gpui::black()
                });
            }
            if span.link.is_some() {
                style.color = Some(palette.link());
                style.underline = Some(UnderlineStyle {
                    thickness: px(1.),
                    color: Some(if palette.dark {
                        rgba(0x2b7fff80).into()
                    } else {
                        rgba(0x51a2ff66).into()
                    }),
                    wavy: false,
                });
            }
            (span.range.clone(), style)
        })
        .collect::<Vec<_>>();
    let mono = inline
        .spans
        .iter()
        .filter(|span| span.code)
        .map(|span| (span.range.clone(), palette.mono.clone()))
        .collect::<Vec<_>>();
    let text = StyledText::new(SharedString::from(inline.text.clone()))
        .with_highlights(highlights)
        .with_font_family_overrides(mono);
    let links: Vec<(Range<usize>, String)> = inline
        .spans
        .iter()
        .filter_map(|span| Some((span.range.clone(), span.link.clone()?)))
        .collect();
    let element = if links.is_empty() {
        text.into_any_element()
    } else {
        let ranges = links.iter().map(|(range, _)| range.clone()).collect();
        InteractiveText::new(SharedString::from(id), text)
            .on_click(ranges, move |index, _, cx| cx.open_url(&links[index].1))
            .into_any_element()
    };
    div().text_color(base).child(element).into_any_element()
}

/// One block with the component classes of `@anlg/changelog` (`mb-1` blocks, `mt-4` headings).
fn block(item: &Block, id: &str, palette: &Palette) -> AnyElement {
    let foreground = palette.colors.foreground;
    match item {
        Block::Paragraph(inline) => div()
            .line_height(px(20.))
            .child(styled(inline, id.into(), foreground, palette))
            .into_any_element(),
        Block::Heading(level, inline) => {
            let (size, line) = match level {
                1 => (16., 24.),
                6 => (12., 16.),
                _ => (14., 20.),
            };
            div()
                .min_h(px(24.))
                .text_size(px(size))
                .line_height(px(line))
                .font_weight(FontWeight::SEMIBOLD)
                .child(styled(inline, id.into(), palette.strong(), palette))
                .into_any_element()
        }
        Block::List {
            ordered,
            start,
            items,
        } => div()
            .flex()
            .flex_col()
            .gap_1()
            // List items inherit the body's 1.5 line height.
            .line_height(px(24.))
            .children(items.iter().enumerate().map(|(index, item)| {
                let marker = if *ordered {
                    format!("{}.", *start + index as u64)
                } else {
                    "•".into()
                };
                div()
                    .flex()
                    .child(
                        div()
                            .w(px(24.))
                            .flex_shrink_0()
                            .flex()
                            .justify_end()
                            .pr(px(7.))
                            .child(marker),
                    )
                    .child(div().flex_1().min_w_0().flex().flex_col().gap_1().children(
                        item.blocks.iter().enumerate().map(|(child, nested)| {
                            let id = format!("{id}-{index}-{child}");
                            match nested {
                                // A tight list item's text inherits the body line height.
                                Block::Paragraph(inline) => styled(inline, id, foreground, palette),
                                nested => block(nested, &id, palette),
                            }
                        }),
                    ))
            }))
            .into_any_element(),
        Block::Quote(blocks) => div()
            .pl_2()
            .border_l(px(3.))
            .border_color(if palette.dark {
                Hsla::from(rgb(0xa8a29e))
            } else {
                gpui::black()
            })
            .flex()
            .flex_col()
            .gap_1()
            .children(
                blocks
                    .iter()
                    .enumerate()
                    .map(|(index, nested)| block(nested, &format!("{id}-{index}"), palette)),
            )
            .into_any_element(),
        Block::Code { code, .. } => div()
            .p_3()
            .border_1()
            .border_color(palette.colors.border)
            .font_family(palette.mono.clone())
            .text_size(px(13.6))
            .child(code.clone())
            .into_any_element(),
        Block::Rule => div().h(px(1.)).bg(palette.colors.border).into_any_element(),
        Block::Table { header, rows } => div()
            .flex()
            .flex_col()
            .text_sm()
            .children(
                std::iter::once(header)
                    .chain(rows)
                    .enumerate()
                    .map(|(row, cells)| {
                        div().flex().gap_4().children(cells.iter().enumerate().map(
                            |(cell, inline)| {
                                div().flex_1().child(styled(
                                    inline,
                                    format!("{id}-{row}-{cell}"),
                                    foreground,
                                    palette,
                                ))
                            },
                        ))
                    }),
            )
            .into_any_element(),
    }
}

/// Top-level blocks stacked with collapsed CSS margins: 4px after each block, 16px above a
/// heading that is not first.
fn blocks(blocks: &[Block], id: &str, palette: &Palette) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .children(blocks.iter().enumerate().map(|(index, item)| {
            let top = match (index, item) {
                (0, _) => 0.,
                (_, Block::Heading(..)) => 16.,
                _ => 4.,
            };
            div()
                .mt(px(top))
                .child(block(item, &format!("{id}-{index}"), palette))
        }))
}

fn banner(
    title: Option<&str>,
    variant: &str,
    content: &[Block],
    id: &str,
    palette: &Palette,
) -> gpui::Div {
    let dark = palette.dark;
    let (border, background, text): (Hsla, Hsla, Hsla) = match variant {
        "warning" if dark => (
            rgb(0x973c00).into(),
            rgba(0x46190166).into(),
            rgb(0xfef3c6).into(),
        ),
        "warning" => (
            rgb(0xffd230).into(),
            rgb(0xfffbeb).into(),
            rgb(0x7b3306).into(),
        ),
        "info" if dark => (
            rgb(0x193cb8).into(),
            rgba(0x16245666).into(),
            rgb(0xdbeafe).into(),
        ),
        "info" => (
            rgb(0x8ec5ff).into(),
            rgb(0xeff6ff).into(),
            rgb(0x1c398e).into(),
        ),
        _ => (
            palette.colors.border,
            palette.colors.muted,
            palette.colors.foreground,
        ),
    };
    div()
        .px_5()
        .pt_4()
        .pb_4()
        .border_1()
        .border_color(border)
        .bg(background)
        .text_color(text)
        .when_some(title, |view, title| {
            view.child(
                div()
                    .mb_1()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title.to_owned()),
            )
        })
        .child(div().text_sm().child(blocks(content, id, palette)))
}

impl Render for ChangelogView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let palette = Palette {
            colors,
            dark: crate::ui::theme::is_dark(window),
            mono: monospace_font(cx),
        };
        let gutter = if cfg!(target_os = "macos") {
            f32::from(crate::zoom::window_controls_gutter(cx))
        } else {
            0.
        };
        let notice = |text: &'static str| {
            div()
                .text_color(colors.muted_foreground)
                .line_height(px(24.))
                .child(text)
                .into_any_element()
        };
        let body = match &self.content {
            Content::Loading => notice("Loading..."),
            Content::Ready(None) => notice("No changelog available for this version."),
            Content::Ready(Some(pieces)) => div()
                .flex()
                .flex_col()
                .children(pieces.iter().enumerate().map(|(index, piece)| {
                    let id = format!("changelog-{index}");
                    match piece {
                        Piece::Markdown(content) => blocks(content, &id, &palette),
                        Piece::Banner {
                            title,
                            variant,
                            blocks: content,
                        } => div().mb_2().child(banner(
                            title.as_deref(),
                            variant,
                            content,
                            &id,
                            &palette,
                        )),
                    }
                }))
                .into_any_element(),
        };
        let workspace = self.workspace.clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .text_base()
            .child(
                div().flex_shrink_0().pr_1().pl_3().child(
                    div()
                        .relative()
                        .h_12()
                        .w_full()
                        .flex()
                        .items_center()
                        .when(!self.sidebar_expanded, |view| view.pl(px(gutter + 80.)))
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .bottom_0()
                                .left(px(if self.sidebar_expanded {
                                    0.
                                } else {
                                    gutter + 28.
                                }))
                                .right(px(70.))
                                .flex()
                                .items_center()
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .text_xl()
                                        .line_height(px(28.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(colors.foreground)
                                        .child(format!("What's new in {}?", self.version)),
                                ),
                        )
                        .child(
                            div()
                                .id("changelog-close")
                                .ml_auto()
                                .size_7()
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .text_color(colors.muted_foreground)
                                .hover(|style| style.bg(colors.accent))
                                .on_click(move |_, _, cx| {
                                    let _ = workspace
                                        .update(cx, |workspace, cx| workspace.close_current(cx));
                                })
                                .child(
                                    gpui::svg()
                                        .path("product/XIcon.svg")
                                        .size(px(16.))
                                        .text_color(colors.muted_foreground),
                                ),
                        ),
                ),
            )
            .child(
                div().mt_2().flex_1().min_h_0().overflow_hidden().child(
                    div()
                        .id("changelog-body")
                        .size_full()
                        .overflow_y_scroll()
                        .px_3()
                        .pb_4()
                        .child(body),
                ),
            )
    }
}

impl WorkspaceView {
    /// The changelog tab's view, kept while the tab renders.
    pub(super) fn changelog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Entity<ChangelogView> {
        let fetch = http(self.runtime.clone());
        let workspace = cx.weak_entity();
        let view = window.use_keyed_state("changelog", cx, move |_, cx| {
            ChangelogView::new(current_version().into(), fetch, workspace, cx)
        });
        let expanded = self.sidebar.expanded;
        view.update(cx, |view, _| view.set_sidebar_expanded(expanded));
        view
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changelog_sources_match_shipping() {
        assert_eq!(
            url("1.4.35").as_deref(),
            Some(
                "https://raw.githubusercontent.com/fastrepl/anarlog/main/packages/changelog/content/desktop/1.4.35.md"
            )
        );
        assert!(
            url("1.4.36-nightly.2")
                .unwrap()
                .ends_with("desktop_nightly_v1.4.36-nightly.2")
        );
        assert_eq!(url("1.4"), None);
        assert_eq!(url("dev"), None);
        let raw = "---\ndate: \"2026-11-16\"\nsummary: \"x\"\n---\n\n- First\n";
        assert_eq!(body(raw), "- First");
        assert_eq!(body("---\ndate: x\n---"), "---\ndate: x\n---");
        assert_eq!(body("- No frontmatter\n"), "- No frontmatter");
    }

    #[gpui::test]
    fn unbundled_versions_load_through_the_fetch_adapter(cx: &mut gpui::TestAppContext) {
        let fetch: Fetch = Arc::new(|version: String| {
            Box::pin(async move {
                (version == "1.0.0").then(|| "---\ndate: \"2026-01-01\"\n---\n\n- Fixture".into())
            })
        });
        let found = cx.new(|cx| {
            ChangelogView::new("1.0.0".into(), fetch.clone(), WeakEntity::new_invalid(), cx)
        });
        let missing =
            cx.new(|cx| ChangelogView::new("0.0.1".into(), fetch, WeakEntity::new_invalid(), cx));
        cx.run_until_parked();
        assert_eq!(found.read_with(cx, |view, _| view.loaded()), Some(true));
        assert_eq!(missing.read_with(cx, |view, _| view.loaded()), Some(false));
    }
}
