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
            .when(self.sidebar_expanded, |surface| {
                surface
                    .border_l_1()
                    .border_color(theme.border)
                    .rounded_tl(px(12.0))
            })
            .overflow_hidden()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
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
                .rounded_full()
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
            .when(self.sidebar_expanded, |header| header.pl_2())
            .when(!self.sidebar_expanded, |header| header.pl(px(32.0)))
            .items_center()
            .gap(px(2.0))
            .when(preview.enhanced.len() + 1 > 1, |header| {
                header.child(self.render_view_switcher(preview, tab, cx))
            })
            .child(
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
            .child(div().flex_1())
            .child(self.render_meeting_cta(preview, window))
            .child(
                ghost_icon_button("overflow", theme)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                        this.set_hovered("overflow", *hovered, cx);
                    }))
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_overflow_menu(cx)),
                    )
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
    fn render_meeting_cta(&self, preview: &NotePreview, window: &Window) -> Div {
        let theme = self.theme;
        let event = preview.session_event();
        let event = event.as_ref();
        let show_demo_prompt =
            event.is_some_and(SessionEvent::is_welcome_demo) && !preview.has_transcript;
        let (label, glyph): (&str, AnyElement) = match event {
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
        let button = div()
            .id("meeting-cta")
            .flex()
            .h(px(32.0))
            .max_w(px(224.0))
            .flex_shrink_0()
            .items_center()
            .gap(px(6.0))
            .pl(px(6.0))
            .pr(px(10.0))
            .rounded_md()
            .border_1()
            .border_color(theme.border)
            .tw_text_sm()
            .text_color(theme.foreground)
            .cursor_pointer()
            .hover(move |style| style.bg(theme.accent))
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
            NoteTab::Enhanced(_) => None,
        };

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
            let renderer = self.document_editor_renderer(editor.clone(), window);
            let (blocks, pristine) = {
                let editor = editor.read(cx);
                (
                    crate::document::parse(editor.doc().root()),
                    editor.doc().is_pristine(),
                )
            };
            let mut children = renderer.blocks(&blocks, 0);
            if pristine {
                // `Start writing...` placeholder of the raw editor, drawn over
                // the empty first paragraph.
                let placeholder = div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .py(px(2.0))
                    .text_color(theme.muted_foreground)
                    .child("Start writing...")
                    .into_any_element();
                children.insert(0, placeholder);
            }
            let root = editor.update(cx, |editor, cx| editor.render_root(cx));
            return body.child(root.child(renderer.editable_root(&editor, children, cx)));
        }

        let blocks = match tab {
            NoteTab::Memo => preview.memo.as_slice(),
            NoteTab::Enhanced(id) => preview
                .enhanced
                .iter()
                .find(|doc| &doc.id == id)
                .map(|doc| doc.blocks.as_slice())
                .unwrap_or(&[]),
        };
        let renderer = self.document_renderer(window);
        let has_content = blocks.iter().any(super::document_view::has_visible_content);
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

/// Bundled bitmap/brand images. A bare file name parses as a relative URI, so
/// `img(&str)` would try to fetch it over HTTP.
fn embedded(name: &str) -> ImageSource {
    ImageSource::Resource(Resource::Embedded(name.to_string().into()))
}

impl NotePreview {
    pub fn session_event(&self) -> Option<SessionEvent> {
        SessionEvent::parse(&self.session.event_json)
    }
}
