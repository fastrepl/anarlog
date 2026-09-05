//! Main surface: `StandardContentWrapper` (`bg-card` with the left-chrome
//! border), `OuterHeader` (title breadcrumb, view switcher, meeting CTA,
//! overflow) and the note body (`NoteInput`), or the `EmptyView` actions.

use gpui::{
    AnyElement, ClickEvent, Context, Div, ImageSource, MouseButton, Resource, SharedString,
    Stateful, Window, div, img, prelude::*, px,
};

use super::{Note, NoteTab, Workspace};
use crate::db::NotePreview;
use crate::theme::alpha;
use crate::timeline::{RemoteMeeting, SessionEvent};
use crate::ui::{TailwindText as _, ghost_icon_button, icon};

impl Workspace {
    pub(super) fn render_main_surface(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        let content = match &self.note {
            _ if self.folders_open() => self.render_folders_main(cx),
            _ if self.settings_open() => {
                self.render_settings_content(window, cx).into_any_element()
            }
            Note::Empty => self.render_empty_view().into_any_element(),
            Note::Loading => div().flex_1().into_any_element(),
            Note::Failed(error) => div()
                .p_4()
                .tw_text_sm()
                .text_color(theme.destructive)
                .child(SharedString::from(error.clone()))
                .into_any_element(),
            Note::Ready { preview, tab } => {
                let (preview, tab) = (preview.clone(), tab.clone());
                div()
                    .flex()
                    .flex_col()
                    .size_full()
                    .min_h_0()
                    .child(
                        div()
                            .px_1()
                            .child(self.render_outer_header(&preview, &tab, window, cx)),
                    )
                    // Measured against the Tauri window: the text column starts at
                    // the same x as the breadcrumb title (12px from the surface).
                    .child(
                        div()
                            .min_h_0()
                            .flex_1()
                            .child(self.render_note_body(&preview, &tab, window, cx)),
                    )
                    .into_any_element()
            }
        };
        // `resolvedMainSurfaceChrome`: "left" while the sidebar is expanded
        // (left border, top-left corner rounded off macOS), "top-borderless"
        // when collapsed (no border, no rounding).
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .bg(theme.card)
            .when(self.sidebar_expanded && !self.is_standalone(), |surface| {
                surface
                    .border_l_1()
                    .border_color(theme.border)
                    .rounded_tl(px(12.0))
            })
            .overflow_hidden()
            // Presses that reach the surface neither drag the window nor keep
            // an input focused (a click on the page blurs it in the web view).
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &gpui::MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    if !this.focus_handle.is_focused(window) {
                        this.focus_handle.focus(window);
                    }
                }),
            )
            .child(content)
    }

    /// `EmptyView`: three centred actions with their shortcuts.
    fn render_empty_view(&self) -> Div {
        let theme = self.theme;
        let action = |label: &'static str,
                      keys: &'static [&'static str],
                      action: Option<Box<dyn gpui::Action>>| {
            div()
                .id(SharedString::from(format!("empty-action-{label}")))
                .when_some(action, |item, action| {
                    item.on_click(move |_: &ClickEvent, window, cx| {
                        window.dispatch_action(action.boxed_clone(), cx);
                    })
                })
                .flex()
                .items_center()
                .justify_between()
                .gap_8()
                // The desktop app remaps `.rounded-full` to `0.5rem`.
                .rounded(px(8.0))
                .px_4()
                .py_2()
                .tw_text_sm()
                .text_color(theme.foreground)
                .cursor_pointer()
                .hover(move |style| style.bg(theme.accent))
                .child(label)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .tw_text_xs()
                        .text_color(theme.muted_foreground)
                        .children(keys.iter().map(|key| {
                            div()
                                .px(px(6.0))
                                .py(px(1.0))
                                .rounded_md()
                                .border_1()
                                .border_color(theme.border)
                                .child(*key)
                        })),
                )
        };
        div()
            .flex()
            .h_full()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_6()
            .child(
                div()
                    .flex()
                    .min_w(px(280.0))
                    .flex_col()
                    .gap_1()
                    .child(action(
                        "New Note",
                        &["Ctrl", "N"],
                        Some(Box::new(crate::actions::NewNote)),
                    ))
                    .child(action(
                        "Start Recording",
                        &["Ctrl", "⇧", "N"],
                        Some(Box::new(crate::actions::StartRecording)),
                    ))
                    .child(div().my_1().h(px(1.0)).bg(theme.accent))
                    .child(action(
                        "Settings",
                        &["Ctrl", ","],
                        Some(Box::new(crate::actions::OpenSettings)),
                    )),
            )
    }

    /// `OuterHeader`: `h-12 pb-0.5 flex items-center gap-[2px] pl-2`.
    fn render_outer_header(
        &self,
        preview: &NotePreview,
        tab: &NoteTab,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        // `width: calc(${max(title.length, "Untitled".length)}ch + 2px)`
        let title_chars = self
            .title_input
            .read(cx)
            .text()
            .chars()
            .count()
            .max("Untitled".len());
        let ch = self.measure_text("0", px(14.0), window);
        let title_width = ch * title_chars as f32 + px(2.0);

        // `showSidebarTimelineHeaderGutter` (sidebar collapsed) widens the
        // left padding to `pl-[32px]`; otherwise `pl-2`.
        div()
            .relative()
            .flex()
            .w_full()
            .h(px(48.0))
            .pb(px(2.0))
            .when(self.sidebar_expanded || self.is_standalone(), |header| {
                header.pl_2()
            })
            .when(!self.sidebar_expanded && !self.is_standalone(), |header| {
                header.pl(px(32.0))
            })
            .items_center()
            .gap(px(2.0))
            .when(
                preview.enhanced.len() + 1 + usize::from(preview.has_transcript) > 1,
                |header| header.child(self.render_view_switcher(preview, tab, cx)),
            )
            // `showTitleInput = tab && !isLiveMeeting && !meetingOver`: once the
            // meeting is over the breadcrumb title leaves the header.
            .when(!preview.meeting_over(), |header| {
                header.child(
                    // `TitleInput variant="breadcrumb"`: `h-5 text-sm leading-5
                    // text-neutral-700`, placeholder in muted foreground.
                    div()
                        .w(title_width)
                        .max_w_full()
                        .min_w_0()
                        .flex_shrink()
                        .h(px(20.0))
                        .flex()
                        .items_center()
                        .overflow_hidden()
                        .tw_text_sm()
                        .text_color(theme.title)
                        .child(self.title_input.clone()),
                )
            })
            .child(div().flex_1())
            .child(self.render_meeting_cta(preview, window, cx))
            .child(
                ghost_icon_button("overflow", theme, self.hovered == Some("overflow"))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        this.set_hovered("overflow", *hovered, cx);
                    }))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.toggle_overflow_menu(window, cx)
                    }))
                    .child(icon(
                        "more-horizontal",
                        px(16.0),
                        self.chrome_icon_color("overflow"),
                    )),
            )
    }

    /// `HeaderMeetingControl` in its inactive state: `Join & record` when the
    /// session has a meeting link, otherwise `Record` with the pulsing dot. The
    /// onboarding demo note keeps its "Try the demo" popover open until it has
    /// a transcript.
    fn render_meeting_cta(
        &self,
        preview: &NotePreview,
        window: &Window,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let event = preview.session_event();
        let event = event.as_ref();
        let show_demo_prompt =
            event.is_some_and(SessionEvent::is_welcome_demo) && !preview.has_transcript;
        // Once the meeting is over the CTA turns into `SessionShareButton
        // variant="cta"` (`ShareNetwork size-3.5` + "Share").
        let (label, glyph): (&str, AnyElement) = match event {
            _ if preview.meeting_over() => (
                "Share",
                icon("share", px(14.0), theme.foreground).into_any_element(),
            ),
            Some(event) if event.is_welcome_demo() => (
                "Join & record",
                img(embedded("anarlog-icon.png"))
                    .size(px(14.0))
                    .flex_shrink_0()
                    .into_any_element(),
            ),
            Some(event)
                if event
                    .meeting_link
                    .as_deref()
                    .is_some_and(|link| !link.is_empty()) =>
            {
                (
                    "Join & record",
                    match event.remote_meeting() {
                        Some(RemoteMeeting::Zoom) => img(embedded("zoom-icon.svg"))
                            .size(px(14.0))
                            .into_any_element(),
                        Some(RemoteMeeting::GoogleMeet) => img(embedded("google-meet.svg"))
                            .size(px(14.0))
                            .into_any_element(),
                        Some(RemoteMeeting::Webex) => {
                            img(embedded("webex.png")).size(px(14.0)).into_any_element()
                        }
                        Some(RemoteMeeting::Teams) => {
                            img(embedded("teams.png")).size(px(14.0)).into_any_element()
                        }
                        Some(RemoteMeeting::CalCom) => {
                            icon("video-camera", px(14.0), theme.foreground).into_any_element()
                        }
                        None => icon("headset", px(14.0), theme.foreground).into_any_element(),
                    },
                )
            }
            _ => (
                "Record",
                div()
                    .relative()
                    .flex()
                    .size(px(12.0))
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .absolute()
                            .size(px(10.0))
                            .rounded_full()
                            .bg(alpha(theme.red, 0.4)),
                    )
                    .child(div().relative().size(px(8.0)).rounded_full().bg(theme.red))
                    .into_any_element(),
            ),
        };

        // `Button size="sm" variant="outline"`: `h-8 rounded-md border
        // border-border bg-transparent gap-1.5 pl-1.5 pr-2.5 text-sm`.
        let label_width = self.measure_text(label, px(14.0), window);
        let cta_width = 1.0 + 6.0 + 14.0 + 6.0 + f32::from(label_width) + 10.0 + 1.0;
        let hovered = self.hovered == Some("meeting-cta");
        let button = div()
            .id("meeting-cta")
            .relative()
            .flex()
            .h(px(32.0))
            .max_w(px(224.0))
            .flex_shrink_0()
            .items_center()
            .gap(px(6.0))
            .pl(px(6.0))
            .pr(px(10.0))
            // The Button's control squircle carries the border and hover fill.
            .child(crate::squircle::squircle(
                crate::squircle::CONTROL_RADIUS,
                hovered.then_some(theme.accent),
                Some((1.0, theme.border)),
            ))
            .tw_text_sm()
            .text_color(theme.foreground)
            .cursor_pointer()
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.set_hovered("meeting-cta", *hovered, cx);
            }))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(glyph)
            .child(div().truncate().child(label));

        div()
            .relative()
            .flex_shrink_0()
            .child(button)
            .when(show_demo_prompt, |anchor| {
                anchor.child(self.render_demo_prompt(cta_width))
            })
    }

    /// `PopoverContent` under the CTA: `w-72 rounded-md border px-3 py-2.5
    /// text-sm shadow-sm`, `sideOffset={10}`, with a 12px rotated tail. Radix
    /// shifts the box left so it stays inside the window; measured against
    /// the Tauri app that leaves its right edge 12px from the surface edge,
    /// which is 30px past the button's right edge here.
    fn render_demo_prompt(&self, cta_width: f32) -> Div {
        let theme = self.theme;
        const WIDTH: f32 = 288.0;
        const OVERHANG_RIGHT: f32 = 30.0;
        let tail_center_from_left = WIDTH - OVERHANG_RIGHT - cta_width / 2.0;
        div()
            .absolute()
            .top(px(32.0 + 10.0))
            .right(px(-OVERHANG_RIGHT))
            .w(px(WIDTH))
            .rounded_md()
            .border_1()
            .border_color(theme.border)
            .bg(theme.card)
            .shadow_sm()
            .px_3()
            .py(px(10.0))
            .tw_text_sm()
            .text_color(theme.foreground)
            .child(
                div()
                    .absolute()
                    .top(px(-7.0))
                    .left(px(tail_center_from_left - 6.0))
                    .child(icon("popover-tail-border", px(12.0), theme.border))
                    .child(
                        div()
                            .absolute()
                            .top(px(1.0))
                            .left_0()
                            .child(icon("popover-tail", px(12.0), theme.card)),
                    ),
            )
            .child(
                div()
                    .relative()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child("Try the demo"),
            )
            .child(
                div()
                    .relative()
                    .mt(px(2.0))
                    .line_height(px(19.25))
                    .text_color(theme.muted_foreground)
                    .child(
                        "This is a prerecorded demo, so your camera stays off. Click Join & record to see Anarlog in action.",
                    ),
            )
    }

    pub(super) fn measure_text(
        &self,
        text: &str,
        size: gpui::Pixels,
        window: &Window,
    ) -> gpui::Pixels {
        let mut style = window.text_style();
        style.font_size = size.into();
        if let Some(family) = &self.font_family {
            style.font_family = family.clone();
        }
        let run = style.to_run(text.len());
        window
            .text_system()
            .shape_line(SharedString::from(text.to_string()), size, &[run], None)
            .width
    }

    /// `SessionViewSwitcher`: pill strip shown only with more than one tab.
    /// Enhanced tabs first, then the memo; inactive tabs show only their icon.
    fn render_view_switcher(
        &self,
        preview: &NotePreview,
        current: &NoteTab,
        cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let mut tabs: Vec<(NoteTab, SharedString, &'static str)> = preview
            .enhanced
            .iter()
            .map(|doc| {
                let label = if doc.title.trim().is_empty() {
                    "Summary".to_string()
                } else {
                    doc.title.clone()
                };
                (
                    NoteTab::Enhanced(doc.id.clone()),
                    SharedString::from(label),
                    "sparkle",
                )
            })
            .collect();
        tabs.push((NoteTab::Memo, "Memos".into(), "text-align-left"));
        // `createEditorTabs`: the transcript tab follows the memo whenever
        // `getCanShowTranscript` holds (a stored transcript with words).
        if preview.has_transcript {
            tabs.push((NoteTab::Transcript, "Transcript".into(), "waveform"));
        }

        div()
            .flex()
            .h(px(28.0))
            .w_auto()
            .max_w_full()
            .flex_shrink_0()
            .items_center()
            .gap(px(2.0))
            .p(px(2.0))
            .rounded_full()
            .bg(alpha(theme.foreground, 0.1))
            .children(
                tabs.into_iter()
                    .enumerate()
                    .map(|(index, (tab, label, glyph))| {
                        let active = *current == tab;
                        div()
                            .id(("view-tab", index))
                            .flex()
                            .h(px(24.0))
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .gap_1()
                            .px_2()
                            .rounded_full()
                            .cursor_pointer()
                            .when(active, |t| {
                                t.bg(theme.white).text_color(theme.foreground).shadow_xs()
                            })
                            .when(!active, |t| {
                                t.text_color(alpha(theme.muted_foreground, 0.7)).hover(
                                    move |style| {
                                        style
                                            .bg(alpha(theme.background, 0.6))
                                            .text_color(theme.foreground)
                                    },
                                )
                            })
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                                this.set_tab(tab.clone(), cx);
                            }))
                            .child(icon(
                                glyph,
                                px(16.0),
                                if active {
                                    theme.foreground
                                } else {
                                    alpha(theme.muted_foreground, 0.7)
                                },
                            ))
                            .when(active, |t| {
                                t.child(
                                    div()
                                        .tw_text_xs()
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .child(label),
                                )
                                .when(glyph == "sparkle", |t| {
                                    t.child(icon("caret-down", px(12.0), theme.foreground))
                                })
                                // `HeaderViewTranscriptActive`: an inactive session with a
                                // stored transcript can enter edit mode, shown by the pencil.
                                .when(glyph == "waveform", |t| {
                                    t.child(icon("pencil-edit", px(14.0), theme.foreground))
                                })
                            })
                    }),
            )
    }

    /// `NoteInput` scroll column: `px-3 pt-2 pb-6 overflow-y-auto`.
    fn render_note_body(
        &self,
        preview: &NotePreview,
        tab: &NoteTab,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let theme = self.theme;
        let editor = match tab {
            NoteTab::Memo => self
                .editor
                .clone()
                .filter(|editor| editor.read(cx).session_id == preview.session.id),
            NoteTab::Enhanced(_) | NoteTab::Transcript => None,
        };

        if *tab == NoteTab::Transcript {
            // The transcript tab keeps `px-3 pt-2` but scrolls inside the
            // viewer (`overflow-hidden pb-0` on the column).
            return div()
                .id("note-body")
                .h_full()
                .px_3()
                .pt_2()
                .flex()
                .flex_col()
                .overflow_hidden()
                .child(self.render_transcript(preview, cx));
        }

        let body = div()
            .id("note-body")
            .h_full()
            .px_3()
            .pt_2()
            .pb_6()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .text_size(px(16.0))
            .line_height(px(24.0));

        if let Some(editor) = editor {
            // The memo is live: render the editor's document, not the snapshot.
            let renderer = self.document_editor_renderer(editor.clone(), window, cx);
            let (blocks, pristine) = {
                let editor = editor.read(cx);
                (
                    crate::document::parse(editor.doc().root()),
                    editor.doc().is_pristine(),
                )
            };
            let children = renderer.blocks(&blocks, 0);
            let root = editor.update(cx, |editor, cx| editor.render_root(cx));
            // `isMemoEmpty && audioExistsResolved && !canShowTranscript`: the
            // template suggestions float `top-8` over the empty editor.
            let suggestions = (pristine && !preview.has_transcript)
                .then(|| self.render_template_suggestions(preview, cx));
            return body.child(
                div()
                    .relative()
                    .child(root.child(renderer.editable_root(&editor, children, cx)))
                    .children(suggestions),
            );
        }

        let blocks = match tab {
            NoteTab::Memo | NoteTab::Transcript => preview.memo.as_slice(),
            NoteTab::Enhanced(id) => preview
                .enhanced
                .iter()
                .find(|doc| &doc.id == id)
                .map(|doc| doc.blocks.as_slice())
                .unwrap_or(&[]),
        };
        let renderer = self.document_renderer(window);
        let has_content = blocks.iter().any(super::document_view::has_visible_content);
        // `Enhanced`: with no stored content and no way to generate one
        // (`shouldShowEmptySummaryConfigError`: missing provider or model),
        // the tab shows `ConfigError` instead of an editor.
        if matches!(tab, NoteTab::Enhanced(_))
            && !has_content
            && (self.provider_settings.llm_provider.is_none()
                || self.provider_settings.llm_model.is_none())
        {
            return body.child(self.render_summary_config_error(cx));
        }
        body.when(!has_content, |body| {
            body.child(
                div()
                    .py(px(2.0))
                    .text_color(theme.muted_foreground)
                    .child("Start writing..."),
            )
        })
        .when(has_content, |body| {
            body.children(renderer.blocks(blocks, 0))
        })
    }
}

/// `defaultSuggestedTemplateIds`
const DEFAULT_SUGGESTED_TEMPLATE_IDS: [&str; 3] = [
    "default-project-kickoff",
    "default-daily-standup",
    "default-one-on-one-meeting",
];

/// `contextualTemplateRules`
static CONTEXTUAL_TEMPLATE_RULES: std::sync::LazyLock<Vec<(&'static str, regex::Regex)>> =
    std::sync::LazyLock::new(|| {
        vec![
            (
                "default-sales-discovery-call",
                regex::Regex::new(r"(?i)\b(sales|discovery|demo|prospect|lead|client|customer|pipeline|qualification|qualifying|pitch)\b").unwrap(),
            ),
            (
                "default-project-kickoff",
                regex::Regex::new(r"(?i)\b(kickoff|kick-off|project launch)\b").unwrap(),
            ),
            (
                "default-daily-standup",
                regex::Regex::new(r"(?i)\b(standup|stand-up|daily sync|scrum)\b").unwrap(),
            ),
            (
                "default-one-on-one-meeting",
                regex::Regex::new(r"(?i)\b(1:1|one[- ]on[- ]one)\b").unwrap(),
            ),
        ]
    });

/// `getFavoriteTemplates`
pub(crate) fn favorite_templates(templates: &[crate::db::Template]) -> Vec<&crate::db::Template> {
    let mut favorites: Vec<&crate::db::Template> = templates
        .iter()
        .filter(|template| template.pinned && !template.section_titles.is_empty())
        .collect();
    favorites.sort_by(|a, b| {
        let order = |template: &crate::db::Template| template.pin_order.unwrap_or(i64::MAX);
        order(a).cmp(&order(b)).then_with(|| a.title.cmp(&b.title))
    });
    favorites
}

/// `getSuggestedTemplates`
pub(crate) fn suggested_templates<'a>(
    templates: &'a [crate::db::Template],
    favorites: &[&crate::db::Template],
    event_text: &str,
    participant_count: usize,
) -> Vec<&'a crate::db::Template> {
    let available: Vec<&crate::db::Template> = templates
        .iter()
        .filter(|template| {
            !favorites.iter().any(|favorite| favorite.id == template.id)
                && !template.section_titles.is_empty()
        })
        .collect();
    let mut preferred_ids: Vec<&str> = CONTEXTUAL_TEMPLATE_RULES
        .iter()
        .filter(|(_, pattern)| pattern.is_match(event_text))
        .map(|(id, _)| *id)
        .collect();
    if participant_count == 2 {
        preferred_ids.push("default-one-on-one-meeting");
    }
    preferred_ids.extend(DEFAULT_SUGGESTED_TEMPLATE_IDS);
    let mut seen = Vec::new();
    preferred_ids.retain(|id| {
        if seen.contains(id) {
            false
        } else {
            seen.push(*id);
            true
        }
    });
    let preferred: Vec<&crate::db::Template> = preferred_ids
        .iter()
        .filter_map(|id| {
            available
                .iter()
                .copied()
                .find(|template| template.id == *id)
        })
        .collect();
    let mut fallback: Vec<&crate::db::Template> = available
        .iter()
        .copied()
        .filter(|template| !preferred.iter().any(|p| p.id == template.id))
        .collect();
    fallback.sort_by(|a, b| a.title.cmp(&b.title));
    preferred
        .into_iter()
        .chain(fallback)
        .take(DEFAULT_SUGGESTED_TEMPLATE_IDS.len())
        .collect()
}

impl Workspace {
    /// `TemplateEmptyState`: `absolute inset-x-0 top-8 flex-col` with the
    /// favorite and suggested sections (`h-8 text-xs` labels, `h-8 -ml-2 px-2
    /// gap-2 rounded-md` rows in muted foreground) and the New template row.
    fn render_template_suggestions(&self, preview: &NotePreview, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        let event = preview.session_event();
        let event_title = event
            .as_ref()
            .and_then(|event| event.title.clone())
            .unwrap_or_else(|| preview.session.title.clone());
        let event_description = event
            .as_ref()
            .and_then(|event| event.description.clone())
            .unwrap_or_default();
        let participant_count = event
            .as_ref()
            .map(|event| event.participants.len())
            .unwrap_or(0);
        let favorites = favorite_templates(&self.templates);
        let suggested = suggested_templates(
            &self.templates,
            &favorites,
            &format!("{event_title} {event_description}"),
            participant_count,
        );

        let row = |id: gpui::ElementId, glyph: AnyElement, label: SharedString| {
            div()
                .id(id)
                .flex()
                .h(px(32.0))
                .w_auto()
                .max_w_full()
                .items_center()
                .gap_2()
                .ml(px(-8.0))
                .px_2()
                .rounded_md()
                .text_color(theme.muted_foreground)
                .cursor_pointer()
                .hover(move |style| style.bg(theme.accent).text_color(theme.foreground))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(glyph)
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .tw_text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(label),
                )
        };
        let template_glyph = |template: &crate::db::Template| -> AnyElement {
            match &template.icon {
                crate::db::TemplateIcon::Emoji(emoji) => div()
                    .flex()
                    .size(px(16.0))
                    .items_center()
                    .justify_center()
                    .tw_text_sm()
                    .child(SharedString::from(emoji.clone()))
                    .into_any_element(),
                crate::db::TemplateIcon::Icon { name, color } => {
                    let color = parse_hex_color(color).unwrap_or(theme.muted_foreground);
                    icon(template_icon_asset(name), px(16.0), color).into_any_element()
                }
            }
        };
        let section =
            |label: &'static str, templates: &[&crate::db::Template]| -> Vec<AnyElement> {
                if templates.is_empty() {
                    return Vec::new();
                }
                let mut children = vec![
                    div()
                        .flex()
                        .h(px(32.0))
                        .items_center()
                        .tw_text_xs()
                        .text_color(theme.muted_foreground)
                        .child(label)
                        .into_any_element(),
                ];
                for template in templates {
                    let template = (*template).clone();
                    let label = if template.title.trim().is_empty() {
                        "Untitled".to_string()
                    } else {
                        template.title.clone()
                    };
                    children.push(
                        row(
                            SharedString::from(format!("template-{}", template.id)).into(),
                            template_glyph(&template),
                            SharedString::from(label),
                        )
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.apply_template(&template, window, cx);
                        }))
                        .into_any_element(),
                    );
                }
                children
            };

        div()
            .absolute()
            .top(px(32.0))
            .left_0()
            .right_0()
            .flex()
            .flex_col()
            .children(section("Start with a favorite template", &favorites))
            .children(section("Suggested templates", &suggested))
            .child(
                row(
                    "template-new".into(),
                    icon("plus", px(16.0), theme.muted_foreground).into_any_element(),
                    "New template".into(),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    let task = this.store.create_template();
                    cx.spawn(async move |this, cx| match task.await {
                        Ok(Ok(_template_id)) => {
                            // The Templates tab that would open on the new
                            // template is not ported yet.
                            this.update(cx, |this, cx| this.reload_settings(cx)).ok();
                        }
                        Ok(Err(error)) => tracing::error!(%error, "[useCreateTemplate]"),
                        Err(error) => tracing::error!(%error, "[useCreateTemplate]"),
                    })
                    .detach();
                })),
            )
    }

    /// `ConfigError`: centred copy with the `Get Pro` (default) and `Add API
    /// key` (outline) buttons routing to the Account / Intelligence settings.
    fn render_summary_config_error(&self, cx: &mut Context<Self>) -> Div {
        let theme = self.theme;
        let hovered = self.hovered;
        let button = |id: &'static str, label: &'static str, primary: bool| {
            let is_hovered = hovered == Some(id);
            // `Button` (default / outline): the control squircle carries the
            // fill and border.
            let (fill, border) = if primary {
                (
                    Some(if is_hovered {
                        alpha(theme.primary, 0.9)
                    } else {
                        theme.primary
                    }),
                    None,
                )
            } else {
                (
                    Some(if is_hovered {
                        theme.accent
                    } else {
                        theme.background
                    }),
                    Some((1.0, theme.border)),
                )
            };
            div()
                .id(id)
                .relative()
                .flex()
                .h(px(36.0))
                .items_center()
                .justify_center()
                .gap_2()
                .px_4()
                .py_2()
                .child(crate::squircle::squircle(
                    crate::squircle::CONTROL_RADIUS,
                    fill,
                    border,
                ))
                .tw_text_sm()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(if primary {
                    theme.primary_foreground
                } else {
                    theme.foreground
                })
                .cursor_pointer()
                .on_hover(cx.listener(move |this, hovering: &bool, _, cx| {
                    this.set_hovered(id, *hovering, cx);
                }))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(label)
        };
        div()
            .flex()
            .h_full()
            .min_h(px(400.0))
            .flex_col()
            .items_center()
            .justify_center()
            .px_6()
            .child(
                div()
                    .mb_6()
                    .flex()
                    .max_w(px(448.0))
                    .flex_col()
                    .gap_2()
                    .text_center()
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .justify_center()
                            .tw_text_base()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.foreground)
                            .child("Set up AI summaries"),
                    )
                    .child(
                        div()
                            .w_full()
                            .text_center()
                            .text_size(px(14.0))
                            .line_height(px(22.0))
                            .text_color(theme.muted_foreground)
                            .child("Start a Pro trial or add your own LLM API key to generate a summary from this transcript."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(button("summary-get-pro", "Get Pro", true).on_click(cx.listener(
                        |this, _: &ClickEvent, window, cx| {
                            this.open_settings(super::settings::SettingsTab::Account, window, cx);
                        },
                    )))
                    .child(button("summary-add-key", "Add API key", false).on_click(cx.listener(
                        |this, _: &ClickEvent, window, cx| {
                            this.open_settings(super::settings::SettingsTab::Intelligence, window, cx);
                        },
                    ))),
            )
    }

    /// `TranscriptViewer`: a `gap-8` column of transcripts scrolling inside
    /// the tab, `pb-[4rem]`, with `~ ~ ~` separators between transcripts.
    /// Each transcript lists its segments 16px apart (`SEGMENT_GAP`).
    fn render_transcript(&self, preview: &NotePreview, cx: &Context<Self>) -> Stateful<Div> {
        let theme = self.theme;
        let count = preview.transcripts.len();
        div()
            .id("transcript-viewer")
            .relative()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .gap_8()
            .pb(px(64.0))
            .overflow_y_scroll()
            .children(
                preview
                    .transcripts
                    .iter()
                    .enumerate()
                    .map(|(index, transcript)| {
                        let is_last = index + 1 == count;
                        div()
                            .flex()
                            .flex_col()
                            .gap_8()
                            .child(
                                div()
                                    .relative()
                                    .w_full()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(16.0))
                                    .children(transcript.segments.iter().map(|segment| {
                                        self.render_transcript_segment(segment, cx)
                                    })),
                            )
                            .when(!is_last, |column| {
                                // `TranscriptSeparator`
                                let rule = || {
                                    div()
                                        .flex_1()
                                        .border_t_1()
                                        .border_color(alpha(theme.border, 0.4))
                                };
                                column.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_3()
                                        .tw_text_xs()
                                        .font_weight(gpui::FontWeight::LIGHT)
                                        .text_color(theme.muted_foreground)
                                        .child(rule())
                                        .child("~ ~ ~ ~ ~ ~ ~ ~ ~")
                                        .child(rule()),
                                )
                            })
                    }),
            )
    }

    /// `SegmentRenderer`: `section rounded-lg px-2` with the `py-1 text-xs
    /// font-light` speaker header (the assign trigger `-my-0.5 py-0.5 pr-2`
    /// in the segment colour) above `mt-1.5 text-sm leading-relaxed` words.
    fn render_transcript_segment(
        &self,
        segment: &crate::transcript::Segment,
        _cx: &Context<Self>,
    ) -> Div {
        let theme = self.theme;
        let color = crate::transcript::segment_color(&segment.key, theme.dark);
        div()
            .rounded_lg()
            .px_2()
            .child(
                div().relative().py_1().flex().items_center().gap_2().child(
                    div()
                        .id(SharedString::from(format!("speaker-{}", segment.id)))
                        .my(px(-2.0))
                        .py(px(2.0))
                        .pr_2()
                        .rounded(px(8.0))
                        .tw_text_xs()
                        .font_weight(gpui::FontWeight::LIGHT)
                        .text_color(color)
                        .cursor_pointer()
                        .hover(|style| style.text_decoration_1().text_decoration_solid())
                        .child(SharedString::from(segment.speaker_label.clone())),
                ),
            )
            .child(
                div()
                    .mt(px(6.0))
                    .text_size(px(14.0))
                    // `leading-relaxed` = 1.625 * 14 = 22.75, laid out as 22px by WebKit.
                    .line_height(px(22.0))
                    .text_color(theme.foreground)
                    .child(SharedString::from(segment.text.clone())),
            )
    }
}

/// `TEMPLATE_ICON_COMPONENTS`: the bundled subset of the template icon names.
pub(super) fn template_icon_asset(name: &str) -> &'static str {
    match name {
        "notebook-tabs" | "notebook" => "notebook",
        "book-open" => "book-open",
        "calendar" => "calendar-dots",
        "file-text" => "file-text",
        "folder" => "folder",
        "users" => "users",
        "code" => "code",
        "bell" => "bell",
        _ => "notebook",
    }
}

/// `#rrggbb` -> `Rgba`.
pub(super) fn parse_hex_color(value: &str) -> Option<gpui::Rgba> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let channel = |range: std::ops::Range<usize>| u8::from_str_radix(&hex[range], 16).ok();
    Some(gpui::Rgba {
        r: channel(0..2)? as f32 / 255.0,
        g: channel(2..4)? as f32 / 255.0,
        b: channel(4..6)? as f32 / 255.0,
        a: 1.0,
    })
}

/// Bundled bitmap/brand images. A bare file name parses as a relative URI, so
/// `img(&str)` would try to fetch it over HTTP.
fn embedded(name: &str) -> ImageSource {
    ImageSource::Resource(Resource::Embedded(name.to_string().into()))
}

impl NotePreview {
    pub fn session_event(&self) -> Option<SessionEvent> {
        SessionEvent::parse(&self.session.event_json)
    }

    /// `meetingOver = !isRecording && (ended || hasTranscript || audioExists)`
    /// (no capture runs in this shell yet, and audio files are not tracked).
    pub fn meeting_over(&self) -> bool {
        self.has_transcript
            || self.session_event().is_some_and(|event| {
                event
                    .ended_at
                    .as_deref()
                    .and_then(|ended| crate::timeline::parse_date(ended, &chrono::Local))
                    .is_some_and(|ended| ended <= chrono::Utc::now())
            })
    }
}
