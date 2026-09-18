//! The floating recording bar: `plugins/windows/src/window/floating_bar.rs`
//! (the transparent always-on-top window at the work area's top right) and
//! `meeting-float/overlay/bar.tsx` (the compact pill with the stop control,
//! and the expanded live-transcript panel). `FloatingMeetingWindowHost`
//! shows it whenever `floating_bar_enabled` holds and a live session is
//! active, and hides it when the session ends.

use std::collections::HashMap;

use anlg_listener_core::LiveTranscriptSegment;
use anlg_transcript::{ChannelProfile, SegmentKey};
use gpui::{
    AnyElement, App, Context, Render, ScrollHandle, SharedString, WeakEntity, Window, div,
    prelude::*, px, rgba,
};

use super::Workspace;
use crate::ui::{TailwindText, icon};

/// `layout` constants.
pub const INSET: f32 = 4.0;
pub const SCREEN_MARGIN: f32 = 8.0;
pub const COMPACT_HEIGHT: f32 = 38.0;
pub const COMPACT_STOP_WIDTH: f32 = 62.0;
pub const COMPACT_SOLO_STOP_WIDTH: f32 = 68.0;
pub const COMPACT_ICON_SIZE: f32 = 30.0;
pub const COMPACT_GAP: f32 = 3.0;
pub const COMPACT_HORIZONTAL_PADDING: f32 = 4.0;
pub const HOVER_HANDLE_HEIGHT: f32 = 12.0;
pub const HOVER_HANDLE_TOP_PADDING: f32 = 7.0;
pub const HOVER_HANDLE_GAP: f32 = 2.0;
pub const HOVER_HANDLE_RESERVED_HEIGHT: f32 =
    HOVER_HANDLE_TOP_PADDING + HOVER_HANDLE_HEIGHT + HOVER_HANDLE_GAP;
pub const CONTROL_RADIUS: f32 = 10.0;
pub const COMPACT_RADIUS: f32 = 14.0;
pub const EXPANDED_WIDTH: f32 = 360.0;
pub const EXPANDED_HEIGHT: f32 = 430.0;
pub const EXPANDED_RADIUS: f32 = 21.0;
/// `LIVE_TRANSCRIPT_PREVIEW_SEGMENT_LIMIT`
pub const PREVIEW_SEGMENT_LIMIT: usize = 200;
/// `FLOATING_TRANSCRIPT_OVERLAP_THRESHOLD_MS`
const OVERLAP_THRESHOLD_MS: i64 = 300;

/// `compactControlsWidth`
pub fn compact_controls_width(shows_expand: bool) -> f32 {
    if shows_expand {
        COMPACT_STOP_WIDTH + COMPACT_GAP + COMPACT_ICON_SIZE
    } else {
        COMPACT_SOLO_STOP_WIDTH
    }
}

/// `compactWidth`
pub fn compact_width(shows_expand: bool) -> f32 {
    compact_controls_width(shows_expand) + COMPACT_HORIZONTAL_PADDING * 2.0
}

/// `container_size`: the window around the compact pill or the expanded panel.
pub fn container_size(is_expanded: bool, shows_expand: bool) -> (f32, f32) {
    if is_expanded {
        (
            EXPANDED_WIDTH + INSET * 2.0,
            EXPANDED_HEIGHT + HOVER_HANDLE_RESERVED_HEIGHT + INSET * 2.0,
        )
    } else {
        (
            compact_width(shows_expand) + INSET * 2.0,
            COMPACT_HEIGHT + HOVER_HANDLE_RESERVED_HEIGHT + INSET * 2.0,
        )
    }
}

/// `resize_keep_top_right`: the panel grows and shrinks from its top-right
/// corner.
pub fn resize_keep_top_right(x: f32, y: f32, current_width: f32, next_width: f32) -> (f32, f32) {
    (x + current_width - next_width, y)
}

/// `clamp_to_work_area`
pub fn clamp_to_work_area(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    work: (f32, f32, f32, f32),
) -> (f32, f32) {
    let (work_x, work_y, work_width, work_height) = work;
    let max_x = (work_x + work_width - width).max(work_x);
    let max_y = (work_y + work_height - height).max(work_y);
    (x.clamp(work_x, max_x), y.clamp(work_y, max_y))
}

/// `top_right_origin`: `SCREEN_MARGIN` inside the work area's top-right corner.
pub fn top_right_origin(
    work_x: f32,
    work_y: f32,
    work_width: f32,
    window_width: f32,
) -> (f32, f32) {
    (
        work_x + work_width - window_width - SCREEN_MARGIN,
        work_y + SCREEN_MARGIN,
    )
}

/// `FloatingTranscriptBubble`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranscriptBubble {
    pub id: String,
    pub speaker_label: String,
    pub text: String,
    pub is_self: bool,
    pub is_final: bool,
    pub start_ms: i64,
    pub end_ms: i64,
    pub overlaps_previous: bool,
    pub overlaps_next: bool,
}

/// `createMeetingFloatLabelContext`: the live session's title, owner and
/// participants plus every human's name, for `SegmentKeyUtils.renderLabel`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LabelContext {
    pub title: Option<String>,
    pub owner_user_id: String,
    pub participant_human_ids: Vec<String>,
    pub human_names: HashMap<String, String>,
}

impl LabelContext {
    fn human_name(&self, human_id: &str) -> Option<&str> {
        self.human_names
            .get(human_id)
            .map(String::as_str)
            .filter(|name| !name.is_empty())
    }

    /// `getUniqueRemoteParticipantHumanId`
    fn unique_remote_participant(&self) -> Option<&str> {
        let mut remote: Vec<&str> = Vec::new();
        for id in &self.participant_human_ids {
            if !id.is_empty() && *id != self.owner_user_id && !remote.contains(&id.as_str()) {
                remote.push(id);
            }
        }
        (remote.len() == 1).then(|| remote[0])
    }
}

/// `getFloatingTitle`
pub fn floating_title(title: Option<&str>) -> String {
    match title.map(str::trim) {
        Some(title) if !title.is_empty() => title.to_string(),
        _ => "Live transcript".to_string(),
    }
}

/// `applyLiveSegmentDelta`: drop the changed ids, add the upserts, keep the
/// last `LIVE_TRANSCRIPT_PREVIEW_SEGMENT_LIMIT` in time order.
pub fn apply_segment_delta(
    segments: &mut Vec<LiveTranscriptSegment>,
    upserts: Vec<LiveTranscriptSegment>,
    removed_ids: &[String],
) {
    segments.retain(|segment| {
        !removed_ids.contains(&segment.id) && !upserts.iter().any(|s| s.id == segment.id)
    });
    segments.extend(upserts);
    sort_segments(segments);
    let overflow = segments.len().saturating_sub(PREVIEW_SEGMENT_LIMIT);
    segments.drain(..overflow);
}

fn sort_segments(segments: &mut [LiveTranscriptSegment]) {
    segments.sort_by(|a, b| {
        a.start_ms
            .cmp(&b.start_ms)
            .then(a.end_ms.cmp(&b.end_ms))
            .then_with(|| a.id.cmp(&b.id))
    });
}

/// `getFloatingTranscriptBubbles`
pub fn transcript_bubbles(
    segments: &[LiveTranscriptSegment],
    ctx: Option<&LabelContext>,
) -> Vec<TranscriptBubble> {
    let mut sorted: Vec<LiveTranscriptSegment> = segments.to_vec();
    sort_segments(&mut sorted);
    let skip = sorted.len().saturating_sub(PREVIEW_SEGMENT_LIMIT);
    let mut bubbles: Vec<TranscriptBubble> = sorted[skip..]
        .iter()
        .filter_map(|segment| {
            let text = segment_text(segment);
            if text.is_empty() {
                return None;
            }
            Some(TranscriptBubble {
                id: segment.id.clone(),
                speaker_label: speaker_label(&segment.key, ctx),
                text,
                is_self: is_self_speaker(&segment.key),
                is_final: segment.words.iter().all(|word| word.is_final),
                start_ms: segment.start_ms,
                end_ms: segment.end_ms,
                overlaps_previous: false,
                overlaps_next: false,
            })
        })
        .collect();
    mark_overlaps(&mut bubbles);
    bubbles
}

/// `getFloatingSegmentText`: the words joined, punctuation re-attached,
/// falling back to the segment text; whitespace collapsed.
fn segment_text(segment: &LiveTranscriptSegment) -> String {
    let words: Vec<&str> = segment
        .words
        .iter()
        .map(|word| word.text.trim())
        .filter(|text| !text.is_empty())
        .collect();
    let mut joined = String::new();
    for word in words {
        let punctuation = word.len() == 1 && matches!(word, "," | "." | "?" | "!" | ";" | ":");
        if !joined.is_empty() && !punctuation {
            joined.push(' ');
        }
        joined.push_str(word);
    }
    let source = if joined.is_empty() {
        segment.text.as_str()
    } else {
        joined.as_str()
    };
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_self_speaker(key: &SegmentKey) -> bool {
    key.channel == ChannelProfile::DirectMic
}

/// `getFloatingSpeakerLabel` over `SegmentKeyUtils.renderLabel(key, ctx)`.
fn speaker_label(key: &SegmentKey, ctx: Option<&LabelContext>) -> String {
    if is_self_speaker(key) {
        return "You".to_string();
    }
    if let Some(ctx) = ctx {
        if let Some(name) = key
            .speaker_human_id
            .as_deref()
            .and_then(|id| ctx.human_name(id))
        {
            return name.to_string();
        }
        if key.channel == ChannelProfile::RemoteParty
            && key.speaker_human_id.is_none()
            && let Some(remote) = ctx.unique_remote_participant()
        {
            return ctx.human_name(remote).unwrap_or(remote).to_string();
        }
        return match key.speaker_index {
            Some(index) => format!("Speaker {}", index + 1),
            None => format!(
                "Speaker {}",
                match key.channel {
                    ChannelProfile::DirectMic => "A",
                    ChannelProfile::RemoteParty => "B",
                    ChannelProfile::MixedCapture => "C",
                }
            ),
        };
    }
    match key.speaker_index {
        Some(index) => format!("Speaker {}", index + 1),
        None if key.channel == ChannelProfile::RemoteParty => "Speaker".to_string(),
        None => "Audio".to_string(),
    }
}

struct RankedBoundary {
    is_self: bool,
    speaker_label: String,
    value: i64,
}

/// `markFloatingTranscriptOverlaps`: a bubble overlaps its neighbour when a
/// different speaker's segment runs past its start (or starts before its end)
/// by more than the threshold, tracked over the two latest speakers.
fn mark_overlaps(bubbles: &mut [TranscriptBubble]) {
    let mut latest_ends: Vec<RankedBoundary> = Vec::new();
    for bubble in bubbles.iter_mut() {
        let latest_other_end = other_speaker_boundary(&latest_ends, bubble);
        bubble.overlaps_previous = bubble.end_ms - bubble.start_ms >= OVERLAP_THRESHOLD_MS
            && latest_other_end.is_some_and(|end| end >= bubble.start_ms + OVERLAP_THRESHOLD_MS);
        update_ranked(&mut latest_ends, bubble, bubble.end_ms, true);
    }
    let mut earliest_starts: Vec<RankedBoundary> = Vec::new();
    for bubble in bubbles.iter_mut().rev() {
        let earliest_other_start = other_speaker_boundary(&earliest_starts, bubble);
        bubble.overlaps_next =
            earliest_other_start.is_some_and(|start| start <= bubble.end_ms - OVERLAP_THRESHOLD_MS);
        if bubble.end_ms - bubble.start_ms >= OVERLAP_THRESHOLD_MS {
            update_ranked(&mut earliest_starts, bubble, bubble.start_ms, false);
        }
    }
}

fn other_speaker_boundary(boundaries: &[RankedBoundary], bubble: &TranscriptBubble) -> Option<i64> {
    boundaries
        .iter()
        .find(|b| b.is_self != bubble.is_self || b.speaker_label != bubble.speaker_label)
        .map(|b| b.value)
}

fn update_ranked(
    boundaries: &mut Vec<RankedBoundary>,
    bubble: &TranscriptBubble,
    value: i64,
    largest: bool,
) {
    match boundaries
        .iter_mut()
        .find(|b| b.is_self == bubble.is_self && b.speaker_label == bubble.speaker_label)
    {
        Some(existing) => {
            if (largest && value > existing.value) || (!largest && value < existing.value) {
                existing.value = value;
            }
        }
        None => boundaries.push(RankedBoundary {
            is_self: bubble.is_self,
            speaker_label: bubble.speaker_label.clone(),
            value,
        }),
    }
    if largest {
        boundaries.sort_by(|a, b| b.value.cmp(&a.value));
    } else {
        boundaries.sort_by(|a, b| a.value.cmp(&b.value));
    }
    boundaries.truncate(2);
}

/// `FloatingBarState`
#[derive(Clone, Debug, PartialEq)]
pub struct FloatingBarState {
    pub amplitude: f32,
    pub title: String,
    pub error: bool,
    pub dark: bool,
    pub opacity: f32,
    pub live_caption_toggle_visible: bool,
    /// `liveCaptionMinimized`: the setting the expand / collapse button flips.
    pub live_caption_minimized: bool,
    pub transcript_bubbles: Vec<TranscriptBubble>,
}

impl FloatingBarState {
    /// `is_expanded`
    pub fn is_expanded(&self) -> bool {
        self.live_caption_toggle_visible && !self.live_caption_minimized
    }

    /// The window around the state's pill or panel.
    pub fn container_size(&self) -> (f32, f32) {
        container_size(self.is_expanded(), self.live_caption_toggle_visible)
    }
}

/// `barColors(state)`
struct BarColors {
    surface: gpui::Rgba,
    envelope_surface: gpui::Rgba,
    content: gpui::Rgba,
    handle: gpui::Rgba,
    outer_stroke: gpui::Rgba,
    control_fill: gpui::Rgba,
    accent: gpui::Rgba,
}

fn with_alpha(rgb: u32, alpha: f32) -> gpui::Rgba {
    rgba((rgb << 8) | ((alpha.clamp(0.0, 1.0) * 255.0).round() as u32))
}

fn bar_colors(state: &FloatingBarState) -> BarColors {
    let opacity = state.opacity.clamp(0.35, 0.95);
    let surface_rgb = if state.dark { 0x6e7066 } else { 0xdbd9d1 };
    let ink = if state.dark { 0xffffff } else { 0x1f1c1a };
    BarColors {
        surface: with_alpha(surface_rgb, opacity * 0.82),
        envelope_surface: with_alpha(surface_rgb, (opacity * 1.08).min(0.95)),
        content: with_alpha(ink, 1.0),
        handle: with_alpha(ink, if state.dark { 0.48 } else { 0.36 }),
        outer_stroke: with_alpha(ink, if state.dark { 0.14 } else { 0.12 }),
        control_fill: with_alpha(ink, if state.dark { 0.08 } else { 0.07 }),
        accent: if state.error {
            gpui::rgb(0xff403d)
        } else {
            gpui::rgb(0xff334d)
        },
    }
}

/// The floating window's root view.
pub struct FloatingBar {
    pub state: FloatingBarState,
    workspace: WeakEntity<Workspace>,
    hovered: bool,
    stop_hovered: bool,
    /// `TranscriptList`'s scroll container and its `pinned` flag: the list
    /// follows new bubbles until the user scrolls more than 20px up.
    transcript_scroll: ScrollHandle,
    pinned: bool,
    /// The list's scroll range at the last frame: an offset change under the
    /// same range is the user scrolling (`onScroll`); a range change is new
    /// content, which the pin follows.
    last_scroll_range: gpui::Pixels,
    font_family: Option<SharedString>,
}

impl FloatingBar {
    pub fn new(state: FloatingBarState, workspace: WeakEntity<Workspace>) -> Self {
        Self {
            state,
            workspace,
            hovered: false,
            stop_hovered: false,
            transcript_scroll: ScrollHandle::new(),
            pinned: true,
            last_scroll_range: px(0.0),
            font_family: None,
        }
    }

    /// The unwrapped width of `text` at `size` in the window's UI font.
    fn measure_text(&self, text: &str, size: gpui::Pixels, window: &Window) -> gpui::Pixels {
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

    /// `onToggleExpanded`: flips `live_caption_minimized` through the main
    /// window's settings write, which re-syncs the bar. Deferred past this
    /// window's own update, since the re-sync closes and reopens it.
    fn toggle_expanded(&mut self, cx: &mut Context<Self>) {
        let expanded = self.state.is_expanded();
        let workspace = self.workspace.clone();
        cx.spawn(async move |_, cx| {
            workspace
                .update(cx, |workspace, cx| {
                    workspace.set_live_caption_minimized(expanded, cx);
                })
                .ok();
        })
        .detach();
    }

    /// `HoverHandle`: a `5px × 7px` dot grid, 16px narrower than the pill.
    fn render_hover_handle(&self, colors: &BarColors, width: f32) -> AnyElement {
        let dots_width = (width - 16.0).max(0.0);
        let columns = (dots_width / 5.0).floor() as usize;
        let rows = (HOVER_HANDLE_HEIGHT / 7.0).floor() as usize;
        let handle = colors.handle;
        div()
            .flex()
            .h(px(HOVER_HANDLE_HEIGHT))
            .w(px(width))
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .h_full()
                    .w(px(dots_width))
                    .children((0..rows).map(move |_| {
                        div()
                            .flex()
                            .h(px(7.0))
                            .items_center()
                            .children((0..columns).map(move |_| {
                                div()
                                    .flex()
                                    .w(px(5.0))
                                    .items_center()
                                    .justify_center()
                                    .child(div().size(px(1.6)).rounded_full().bg(handle))
                            }))
                    })),
            )
            .into_any_element()
    }

    /// `FloatingControls`: the stop control and, with live captions, the
    /// expand / collapse button.
    fn render_controls(&self, colors: &BarColors, cx: &mut Context<Self>) -> AnyElement {
        let content = colors.content;
        let expanded = self.state.is_expanded();
        div()
            .flex()
            .items_center()
            .gap(px(COMPACT_GAP))
            .child(self.render_stop_control(colors, cx))
            .when(self.state.live_caption_toggle_visible, |controls| {
                controls.child(
                    div()
                        .id("floating-bar-expand")
                        .flex()
                        .size(px(COMPACT_ICON_SIZE))
                        .items_center()
                        .justify_center()
                        .rounded(px(CONTROL_RADIUS))
                        .cursor_pointer()
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            this.toggle_expanded(cx);
                        }))
                        .child(icon(
                            if expanded {
                                "arrows-in-simple"
                            } else {
                                "arrows-out-simple"
                            },
                            px(14.0),
                            content,
                        )),
                )
            })
            .into_any_element()
    }

    /// `ExpandedPanel`: the title row, the transcript list and the controls
    /// at the top right, under a hover handle that fades in.
    fn render_expanded_panel(
        &mut self,
        colors: &BarColors,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let hovered = self.hovered;
        let controls_width = compact_controls_width(self.state.live_caption_toggle_visible);
        let height = EXPANDED_HEIGHT
            + if hovered {
                HOVER_HANDLE_RESERVED_HEIGHT
            } else {
                0.0
            };
        div()
            .relative()
            .overflow_hidden()
            .w(px(EXPANDED_WIDTH))
            .h(px(height))
            .rounded(px(EXPANDED_RADIUS))
            .bg(colors.surface)
            .child(inset_stroke(EXPANDED_RADIUS, colors.outer_stroke))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(HOVER_HANDLE_RESERVED_HEIGHT))
                    .pt(px(HOVER_HANDLE_TOP_PADDING))
                    .opacity(if hovered { 1.0 } else { 0.0 })
                    .child(self.render_hover_handle(colors, EXPANDED_WIDTH)),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(px(EXPANDED_HEIGHT))
                    .child(
                        // `h-[38px] pl-4`, right padding clearing the controls.
                        div()
                            .flex()
                            .h(px(COMPACT_HEIGHT))
                            .items_center()
                            .pl(px(16.0))
                            .pr(px(controls_width + 12.0))
                            .child(self.render_title(
                                EXPANDED_WIDTH - 16.0 - (controls_width + 12.0),
                                colors,
                                window,
                                cx,
                            )),
                    )
                    .child(self.render_transcript_list(window, cx))
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .right(px(COMPACT_HORIZONTAL_PADDING))
                            .flex()
                            .w(px(controls_width))
                            .h(px(COMPACT_HEIGHT))
                            .items_center()
                            .justify_center()
                            .child(self.render_controls(colors, cx)),
                    ),
            )
            .into_any_element()
    }

    /// `p.min-w-0.truncate.text-[13px].font-semibold`: the title on the
    /// root's `line-height: 1.5`, ellipsized to the row's free width.
    fn render_title(
        &self,
        width: f32,
        colors: &BarColors,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut style = window.text_style();
        style.font_size = px(13.0).into();
        style.font_weight = gpui::FontWeight::SEMIBOLD;
        style.color = colors.content.into();
        if let Some(family) = &self.font_family {
            style.font_family = family.clone();
        }
        let title = SharedString::from(self.state.title.clone());
        let mut runs = vec![style.to_run(title.len())];
        let title = cx
            .text_system()
            .line_wrapper(style.font(), px(13.0))
            .truncate_line(title, px(width), "…", &mut runs);
        div()
            .w(px(width))
            .child(
                crate::prose_text::ProseText::new(title, runs, px(13.0), px(19.5))
                    .max_width(px(width)),
            )
            .into_any_element()
    }

    /// `TranscriptList`: `h-[calc(100%-38px)] px-3 pb-3`, bubbles bottom-aligned
    /// in a scroll container that follows the newest one while pinned, with the
    /// `Go to bottom` chip once the user scrolls away.
    fn render_transcript_list(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let dark = self.state.dark;
        let bubbles = &self.state.transcript_bubbles;
        // `onScroll`: pinned while within 20px of the bottom.
        let offset = self.transcript_scroll.offset();
        let range = self.transcript_scroll.max_offset().height;
        if range == self.last_scroll_range && range > px(0.0) {
            self.pinned = f32::from(range + offset.y) < 20.0;
        }
        self.last_scroll_range = range;
        if self.pinned {
            self.transcript_scroll.scroll_to_bottom();
        }
        let show_go_to_bottom = !self.pinned && !bubbles.is_empty();
        // `overflow-y-auto` reserves WebKit's 6px scrollbar gutter once the
        // list scrolls.
        let gutter = if range > px(0.0) { 6.0 } else { 0.0 };
        let list_width = EXPANDED_WIDTH - 24.0 - gutter;

        let mut list = div()
            .flex()
            .w(px(list_width))
            .min_h_full()
            .flex_col()
            .justify_end()
            .gap_2();
        // `max-w-[calc(100%-40px)]` on a shrink-to-fit block column: its
        // width is decided here from the unwrapped label and text so wrapped
        // bubbles are laid out at their real height.
        let max_column_width = list_width - 40.0;
        for (index, bubble) in bubbles.iter().enumerate() {
            let previous = index.checked_sub(1).and_then(|i| bubbles.get(i));
            let shows_speaker_label = previous.is_none_or(|previous| {
                previous.speaker_label != bubble.speaker_label || previous.is_self != bubble.is_self
            });
            let text_width = f32::from(self.measure_text(&bubble.text, px(13.0), window));
            let label_width = if shows_speaker_label {
                f32::from(self.measure_text(&bubble.speaker_label, px(10.0), window)) + 8.0
            } else {
                0.0
            };
            let column_width = (text_width + 20.0)
                .max(label_width)
                .ceil()
                .min(max_column_width);
            let mut style = window.text_style();
            style.font_size = px(13.0).into();
            style.color = gpui::rgb(0xffffff).into();
            if let Some(family) = &self.font_family {
                style.font_family = family.clone();
            }
            let run = style.to_run(bubble.text.len());
            list = list.child(render_bubble(
                bubble,
                shows_speaker_label,
                dark,
                column_width,
                run,
            ));
        }
        // The `bottomRef` sentinel: an empty flex child that keeps one `gap-2`
        // below the last bubble.
        list = list.child(div().h(px(0.0)));

        div()
            .relative()
            .h(px(EXPANDED_HEIGHT - COMPACT_HEIGHT))
            .px_3()
            .pb_3()
            .child(
                // `h-full` inside the padded box: the list stops above `pb-3`.
                div()
                    .id("floating-bar-transcript")
                    .h(px(EXPANDED_HEIGHT - COMPACT_HEIGHT - 12.0))
                    .overflow_y_scroll()
                    .track_scroll(&self.transcript_scroll)
                    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(list),
            )
            .when(show_go_to_bottom, |panel| {
                panel.child(
                    div()
                        .absolute()
                        .bottom_3()
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .id("floating-bar-go-to-bottom")
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .rounded(px(10.0))
                                .px_3()
                                .py(px(6.0))
                                .text_size(px(11.0))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .bg(if dark {
                                    gpui::rgb(0x2e2e2b)
                                } else {
                                    gpui::rgb(0xf2f2ed)
                                })
                                .text_color(if dark {
                                    gpui::rgb(0xffffff)
                                } else {
                                    gpui::rgb(0x1f1c1a)
                                })
                                .cursor_pointer()
                                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| {
                                    cx.stop_propagation()
                                })
                                .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                                    this.pinned = true;
                                    this.transcript_scroll.scroll_to_bottom();
                                    cx.notify();
                                }))
                                .child(icon(
                                    "caret-down",
                                    px(10.0),
                                    if dark {
                                        gpui::rgb(0xffffff)
                                    } else {
                                        gpui::rgb(0x1f1c1a)
                                    },
                                ))
                                .child("Go to bottom"),
                        ),
                )
            })
            .into_any_element()
    }

    /// `StopControl`: the dancing sticks, or `■ Stop` while hovered.
    fn render_stop_control(&self, colors: &BarColors, cx: &mut Context<Self>) -> AnyElement {
        let width = if self.state.live_caption_toggle_visible {
            COMPACT_STOP_WIDTH
        } else {
            COMPACT_SOLO_STOP_WIDTH
        };
        let accent = colors.accent;
        div()
            .id("floating-bar-stop")
            .flex()
            .w(px(width))
            .h(px(COMPACT_ICON_SIZE))
            .items_center()
            .justify_center()
            .rounded(px(CONTROL_RADIUS))
            .bg(if self.stop_hovered {
                with_alpha(0xff334d, 0.18)
            } else {
                colors.control_fill
            })
            .cursor_pointer()
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.stop_hovered = *hovered;
                cx.notify();
            }))
            .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                // `onStop` → the main window's `stopListening`.
                this.workspace
                    .update(cx, |workspace, cx| workspace.stop_listening(cx))
                    .ok();
            }))
            .child(if self.stop_hovered {
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .tw_text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(accent)
                    // `<Square size={9} />`: the outlined square.
                    .child(icon("square", px(9.0), accent))
                    .child("Stop")
                    .into_any_element()
            } else {
                super::recording::dancing_sticks(self.state.amplitude, accent, 20.0, 26.0, 3.0, 2.0)
            })
            .into_any_element()
    }
}

impl Render for FloatingBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = bar_colors(&self.state);
        if self.font_family.is_none() {
            self.font_family =
                crate::theme::ui_font_family(cx.text_system()).map(SharedString::from);
        }
        // `FloatingBarOverlay`: the pill or panel sits at the bottom right of
        // the `INSET` container; the whole window is a drag region.
        let root = div()
            .id("floating-bar")
            .when_some(self.font_family.clone(), |root, family| {
                root.font_family(family)
            })
            .size_full()
            .flex()
            .items_end()
            .justify_end()
            .p(px(INSET))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.hovered = *hovered;
                cx.notify();
            }))
            .on_mouse_down(gpui::MouseButton::Left, |_, window, _| {
                window.start_window_move();
            });
        if self.state.is_expanded() {
            return root.child(self.render_expanded_panel(&colors, window, cx));
        }
        let width = compact_width(self.state.live_caption_toggle_visible);
        let height = COMPACT_HEIGHT
            + if self.hovered {
                HOVER_HANDLE_RESERVED_HEIGHT
            } else {
                0.0
            };
        let hovered = self.hovered;
        root.child(
            div()
                .relative()
                .overflow_hidden()
                .w(px(width))
                .h(px(height))
                .rounded(px(COMPACT_RADIUS))
                .bg(if hovered {
                    colors.envelope_surface
                } else {
                    colors.surface
                })
                .child(inset_stroke(COMPACT_RADIUS, colors.outer_stroke))
                .when(hovered, |pill| {
                    pill.child(
                        div()
                            .absolute()
                            .top(px(HOVER_HANDLE_TOP_PADDING))
                            .left_0()
                            .child(self.render_hover_handle(&colors, width)),
                    )
                })
                .child(
                    div()
                        .absolute()
                        .right_0()
                        .bottom_0()
                        .flex()
                        .w(px(width))
                        .h(px(COMPACT_HEIGHT))
                        .items_center()
                        .justify_center()
                        .child(self.render_controls(&colors, cx)),
                ),
        )
    }
}

/// `box-shadow: inset 0 0 0 0.5px`: the outer stroke drawn over the surface
/// without taking layout space, unlike a GPUI border.
fn inset_stroke(radius: f32, color: gpui::Rgba) -> AnyElement {
    div()
        .absolute()
        .inset_0()
        .rounded(px(radius))
        .border(px(0.5))
        .border_color(color)
        .into_any_element()
}

/// `TranscriptBubble`: the speaker label (blank while only overlapping) over
/// the `rounded-[11px] px-2.5 py-2 text-[13px] leading-5` bubble, tinted by
/// speaker and outlined when it overlaps a neighbour.
fn render_bubble(
    bubble: &TranscriptBubble,
    shows_speaker_label: bool,
    dark: bool,
    column_width: f32,
    run: gpui::TextRun,
) -> AnyElement {
    let overlapping = bubble.overlaps_previous || bubble.overlaps_next;
    let background = match (bubble.is_self, dark) {
        (true, true) => gpui::hsla(0.0, 0.0, 0.0, 0.34),
        (true, false) => gpui::hsla(0.0, 0.0, 0.0, 0.24),
        (false, true) => gpui::hsla(0.0, 0.0, 0.0, 0.28),
        (false, false) => gpui::hsla(0.0, 0.0, 0.0, 0.2),
    };
    let outline = gpui::hsla(0.0, 0.0, 1.0, if dark { 0.26 } else { 0.34 });
    div()
        .flex()
        .w_full()
        .when(bubble.is_self, |row| row.justify_end())
        .when(!bubble.is_self, |row| row.justify_start())
        .child(
            // A block column: `items-end` has no effect without `flex`, so the
            // label sits at the column's left edge for both speakers.
            div()
                .flex()
                .flex_col()
                .items_start()
                .w(px(column_width))
                .when(shows_speaker_label || overlapping, |column| {
                    column.child(
                        div()
                            .mb_1()
                            .px_1()
                            .text_size(px(10.0))
                            .line_height(px(15.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(gpui::rgb(0xffffff))
                            .child(SharedString::from(if shows_speaker_label {
                                bubble.speaker_label.clone()
                            } else {
                                String::new()
                            })),
                    )
                })
                .child(
                    // `p.text-[13px] leading-5` under the app's `text-wrap: pretty`.
                    div()
                        .w_full()
                        .rounded(px(11.0))
                        .px(px(10.0))
                        .py_2()
                        .bg(background)
                        .when(overlapping, |bubble| {
                            bubble.border_1().border_color(outline)
                        })
                        .child(
                            crate::prose_text::ProseText::new(
                                bubble.text.clone(),
                                vec![run],
                                px(13.0),
                                px(20.0),
                            )
                            .pretty(),
                        ),
                ),
        )
        .into_any_element()
}

/// `windows.floatingBarShow` + `floatingBarUpdate`: open the window on the
/// primary display's top right (or update the shown one).
pub fn show(
    workspace: WeakEntity<Workspace>,
    state: FloatingBarState,
    cx: &mut App,
) -> Option<gpui::WindowHandle<FloatingBar>> {
    let (width, height) = state.container_size();
    let display = cx.primary_display()?;
    let bounds = display.bounds();
    let (x, y) = top_right_origin(
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        width,
    );
    open(workspace, state, (x, y, width, height), cx)
}

/// `apply_layout` for a size change: GPUI has no public window move, so the
/// window reopens at `resize_keep_top_right`, clamped to the display.
pub fn reopen_resized(
    handle: gpui::WindowHandle<FloatingBar>,
    workspace: WeakEntity<Workspace>,
    state: FloatingBarState,
    cx: &mut App,
) -> Option<gpui::WindowHandle<FloatingBar>> {
    let (next_width, next_height) = state.container_size();
    let current = handle
        .update(cx, |_, window, cx| {
            let bounds = window.bounds();
            let display = window.display(cx).map(|display| display.bounds());
            window.remove_window();
            (bounds, display)
        })
        .ok();
    let Some((bounds, display)) = current else {
        return show(workspace, state, cx);
    };
    let (mut x, mut y) = resize_keep_top_right(
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        next_width,
    );
    if let Some(display) = display {
        (x, y) = clamp_to_work_area(
            x,
            y,
            next_width,
            next_height,
            (
                f32::from(display.origin.x),
                f32::from(display.origin.y),
                f32::from(display.size.width),
                f32::from(display.size.height),
            ),
        );
    }
    open(workspace, state, (x, y, next_width, next_height), cx)
}

fn open(
    workspace: WeakEntity<Workspace>,
    state: FloatingBarState,
    (x, y, width, height): (f32, f32, f32, f32),
    cx: &mut App,
) -> Option<gpui::WindowHandle<FloatingBar>> {
    let window_bounds =
        gpui::Bounds::new(gpui::point(px(x), px(y)), gpui::size(px(width), px(height)));
    let result = cx.open_window(
        gpui::WindowOptions {
            window_bounds: Some(gpui::WindowBounds::Windowed(window_bounds)),
            titlebar: None,
            focus: false,
            show: true,
            kind: gpui::WindowKind::PopUp,
            window_decorations: Some(gpui::WindowDecorations::Client),
            is_movable: true,
            is_resizable: false,
            is_minimizable: false,
            window_background: gpui::WindowBackgroundAppearance::Transparent,
            app_id: Some(crate::APP_ID.to_string()),
            ..Default::default()
        },
        move |_, cx| cx.new(|_| FloatingBar::new(state, workspace)),
    );
    match result {
        Ok(handle) => Some(handle),
        Err(error) => {
            tracing::error!(%error, "failed to open the floating bar window");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_sizes_follow_the_layout_constants() {
        assert_eq!(compact_controls_width(false), 68.0);
        assert_eq!(compact_controls_width(true), 62.0 + 3.0 + 30.0);
        assert_eq!(compact_width(false), 76.0);
        assert_eq!(container_size(false, false), (84.0, 38.0 + 21.0 + 8.0));
        assert_eq!(container_size(false, true), (111.0, 67.0));
        assert_eq!(container_size(true, true), (368.0, 459.0));
    }

    #[test]
    fn the_window_sits_inside_the_top_right_margin() {
        assert_eq!(top_right_origin(0.0, 29.0, 1920.0, 84.0), (1828.0, 37.0));
    }

    #[test]
    fn colours_follow_bar_colors() {
        let state = FloatingBarState {
            amplitude: 0.0,
            title: String::new(),
            error: false,
            dark: false,
            opacity: 0.78,
            live_caption_toggle_visible: false,
            live_caption_minimized: true,
            transcript_bubbles: Vec::new(),
        };
        let colors = bar_colors(&state);
        // rgba(219, 217, 209, 0.78 * 0.82)
        assert!((colors.surface.a - 0.6396).abs() < 0.01);
        assert!((colors.envelope_surface.a - 0.8424).abs() < 0.01);
        assert_eq!(colors.accent, gpui::rgb(0xff334d));
        let error = bar_colors(&FloatingBarState {
            error: true,
            ..state.clone()
        });
        assert_eq!(error.accent, gpui::rgb(0xff403d));
        // The opacity is clamped to `[0.35, 0.95]`.
        let dim = bar_colors(&FloatingBarState {
            opacity: 0.1,
            ..state
        });
        assert!((dim.surface.a - 0.35 * 0.82).abs() < 0.01);
    }

    #[test]
    fn expansion_follows_the_toggle_and_the_minimized_setting() {
        let mut state = FloatingBarState {
            amplitude: 0.5,
            title: "Standup".into(),
            error: false,
            dark: true,
            opacity: 0.78,
            live_caption_toggle_visible: true,
            live_caption_minimized: true,
            transcript_bubbles: Vec::new(),
        };
        assert!(!state.is_expanded());
        assert_eq!(state.container_size(), (111.0, 67.0));
        state.live_caption_minimized = false;
        assert!(state.is_expanded());
        assert_eq!(state.container_size(), (368.0, 459.0));
        state.live_caption_toggle_visible = false;
        assert!(!state.is_expanded());
        assert!(bar_colors(&state).content.r > 0.99);
    }

    #[test]
    fn resizing_keeps_the_top_right_corner_inside_the_work_area() {
        assert_eq!(
            resize_keep_top_right(1828.0, 37.0, 84.0, 368.0),
            (1544.0, 37.0)
        );
        assert_eq!(
            clamp_to_work_area(-20.0, 1000.0, 368.0, 459.0, (0.0, 29.0, 1920.0, 1051.0)),
            (0.0, 621.0)
        );
    }

    #[test]
    fn titles_fall_back_to_live_transcript() {
        assert_eq!(floating_title(Some("  Standup ")), "Standup");
        assert_eq!(floating_title(Some("   ")), "Live transcript");
        assert_eq!(floating_title(None), "Live transcript");
    }

    fn segment(
        id: &str,
        channel: ChannelProfile,
        start: i64,
        end: i64,
        words: &[&str],
    ) -> LiveTranscriptSegment {
        LiveTranscriptSegment {
            id: id.to_string(),
            key: SegmentKey {
                channel,
                speaker_index: None,
                speaker_human_id: None,
            },
            start_ms: start,
            end_ms: end,
            text: words.join(" "),
            words: words
                .iter()
                .map(|word| anlg_transcript::SegmentWord {
                    text: word.to_string(),
                    start_ms: start,
                    end_ms: end,
                    channel,
                    is_final: true,
                    id: None,
                })
                .collect(),
        }
    }

    #[test]
    fn segment_deltas_replace_sort_and_cap_the_preview() {
        let mut segments = vec![segment(
            "b",
            ChannelProfile::RemoteParty,
            1000,
            2000,
            &["hi"],
        )];
        apply_segment_delta(
            &mut segments,
            vec![
                segment("a", ChannelProfile::DirectMic, 0, 900, &["hello"]),
                segment(
                    "b",
                    ChannelProfile::RemoteParty,
                    1000,
                    2100,
                    &["hi", "there"],
                ),
            ],
            &[],
        );
        assert_eq!(
            segments.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(segments[1].words.len(), 2);
        apply_segment_delta(&mut segments, Vec::new(), &["a".to_string()]);
        assert_eq!(segments.len(), 1);
        let many: Vec<_> = (0..PREVIEW_SEGMENT_LIMIT + 5)
            .map(|i| {
                segment(
                    &format!("s{i}"),
                    ChannelProfile::DirectMic,
                    i as i64 * 10,
                    i as i64 * 10 + 5,
                    &["w"],
                )
            })
            .collect();
        // `b` (1000ms) still sorts among them, so six segments fall off the front.
        apply_segment_delta(&mut segments, many, &[]);
        assert_eq!(segments.len(), PREVIEW_SEGMENT_LIMIT);
        assert_eq!(segments[0].id, "s6");
        assert!(segments.iter().any(|s| s.id == "b"));
    }

    #[test]
    fn bubbles_carry_labels_text_and_overlaps() {
        let ctx = LabelContext {
            title: None,
            owner_user_id: "me".into(),
            participant_human_ids: vec!["me".into(), "h1".into()],
            human_names: HashMap::from([("h1".to_string(), "Grace".to_string())]),
        };
        let segments = vec![
            segment(
                "a",
                ChannelProfile::DirectMic,
                0,
                1000,
                &["Hello", "there", "."],
            ),
            segment("b", ChannelProfile::RemoteParty, 500, 1500, &["Hi"]),
            segment("c", ChannelProfile::RemoteParty, 1600, 1700, &[]),
        ];
        let bubbles = transcript_bubbles(&segments, Some(&ctx));
        assert_eq!(bubbles.len(), 2);
        assert_eq!(bubbles[0].speaker_label, "You");
        assert!(bubbles[0].is_self);
        assert_eq!(bubbles[0].text, "Hello there.");
        assert_eq!(bubbles[1].speaker_label, "Grace");
        // `a` ends 500ms after `b` starts: both sides overlap.
        assert!(bubbles[0].overlaps_next);
        assert!(bubbles[1].overlaps_previous);
        assert!(!bubbles[0].overlaps_previous);
        assert!(!bubbles[1].overlaps_next);
        // Without a context a remote speaker is `Speaker`; with one and no
        // unique remote participant, `Speaker B`.
        let plain = transcript_bubbles(&segments[1..2], None);
        assert_eq!(plain[0].speaker_label, "Speaker");
        let crowd = LabelContext {
            participant_human_ids: vec!["h1".into(), "h2".into()],
            ..ctx.clone()
        };
        assert_eq!(
            transcript_bubbles(&segments[1..2], Some(&crowd))[0].speaker_label,
            "Speaker B"
        );
    }
}
