//! `session/index.tsx`'s top audio player (`shouldShowSessionTopAudioPlayer`)
//! and `AudioPlayer.Timeline`.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui::{AnyElement, Context, SharedString, div, prelude::*, px};

use super::Workspace;
use crate::audio_player::{Playback, PlayerState, Waveform};
use crate::ui::TailwindText;

const TICK_MS: u64 = 100;

/// `AudioPlayerProvider`'s state for the open session.
pub(crate) struct AudioPlayer {
    pub session_id: String,
    /// `audioUrlReady` once decoded; `None` while loading or after a failure.
    pub waveform: Option<Arc<Waveform>>,
    pub failed: bool,
    pub state: PlayerState,
    pub playback: Option<Playback>,
    /// `timeStore.current`
    pub position: Duration,
    /// `playbackRate` (the rate menu is Pro-only, so this stays at 1x).
    pub rate: f32,
    /// `useNativeContextMenu`: the context menu's anchor while open.
    pub menu_at: Option<gpui::Point<gpui::Pixels>>,
    pub deleting: bool,
}

impl Workspace {
    /// Decode the session audio once per session, like the provider's
    /// `["audio", sessionId, "url"]` query.
    pub(crate) fn ensure_audio_player(&mut self, session_id: &str, cx: &mut Context<Self>) {
        if self
            .audio_player
            .as_ref()
            .is_some_and(|player| player.session_id == session_id)
        {
            return;
        }
        self.audio_player = Some(AudioPlayer {
            session_id: session_id.to_string(),
            waveform: None,
            failed: false,
            state: PlayerState::Stopped,
            playback: None,
            position: Duration::ZERO,
            rate: 1.0,
            menu_at: None,
            deleting: false,
        });
        let Some(path) = anlg_fs_sync_core::audio::path(&self.store.session_dir(session_id)) else {
            if let Some(player) = self.audio_player.as_mut() {
                player.failed = true;
            }
            return;
        };
        let decode = self
            .store
            .runtime()
            .spawn_blocking(move || Waveform::decode(&path));
        let session_id = session_id.to_string();
        cx.spawn(async move |this, cx| {
            let decoded = decode.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update(cx, |this, cx| {
                let Some(player) = this
                    .audio_player
                    .as_mut()
                    .filter(|player| player.session_id == session_id)
                else {
                    return;
                };
                match decoded {
                    Ok(waveform) => player.waveform = Some(Arc::new(waveform)),
                    Err(error) => {
                        tracing::error!(%error, "[audio-player] failed to decode session audio");
                        player.failed = true;
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `handleClick`: play from the stopped state, pause while playing,
    /// resume while paused.
    fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        let Some(player) = self.audio_player.as_mut() else {
            return;
        };
        match player.state {
            PlayerState::Playing => {
                if let Some(playback) = &player.playback {
                    playback.pause();
                }
                player.state = PlayerState::Paused;
            }
            PlayerState::Paused => {
                if let Some(playback) = &player.playback {
                    playback.resume();
                }
                player.state = PlayerState::Playing;
                self.tick_audio_player(cx);
            }
            PlayerState::Stopped => {
                let Some(waveform) = player.waveform.clone() else {
                    return;
                };
                let Some(path) =
                    anlg_fs_sync_core::audio::path(&self.store.session_dir(&player.session_id))
                else {
                    return;
                };
                match Playback::start(&path, waveform.duration, player.rate) {
                    Ok(playback) => {
                        // A finished or seeked cursor resumes from where it sits.
                        if player.position > Duration::ZERO && player.position < waveform.duration {
                            playback.seek(player.position);
                        } else {
                            player.position = Duration::ZERO;
                        }
                        player.playback = Some(playback);
                        player.state = PlayerState::Playing;
                        self.tick_audio_player(cx);
                    }
                    Err(error) => {
                        tracing::error!(%error, "[audio-player] failed to start playback");
                    }
                }
            }
        }
        cx.notify();
    }

    /// `timeupdate` → `syncCurrentTime`, and `finish` → stopped at the end.
    fn tick_audio_player(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(TICK_MS))
                    .await;
                let keep_going = this
                    .update(cx, |this, cx| {
                        let Some(player) = this.audio_player.as_mut() else {
                            return false;
                        };
                        let Some(playback) = &player.playback else {
                            return false;
                        };
                        if player.state != PlayerState::Playing {
                            return false;
                        }
                        if playback.finished() {
                            player.position = player
                                .waveform
                                .as_ref()
                                .map(|w| w.duration)
                                .unwrap_or(player.position);
                            player.playback = None;
                            player.state = PlayerState::Stopped;
                            cx.notify();
                            return false;
                        }
                        player.position = playback.position();
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !keep_going {
                    break;
                }
            }
        })
        .detach();
    }

    /// `stop`: pause, drop the source, and put the cursor back at 0.
    fn stop_playback(&mut self, cx: &mut Context<Self>) {
        if let Some(player) = self.audio_player.as_mut() {
            if let Some(playback) = &player.playback {
                playback.pause();
            }
            player.playback = None;
            player.state = PlayerState::Stopped;
            player.position = Duration::ZERO;
            cx.notify();
        }
    }

    /// `deleteRecording`: stop, `deleteSessionAudio`, then `markAudioDeleted`
    /// (the audio queries flip to "absent", so the note reloads).
    fn delete_recording(&mut self, cx: &mut Context<Self>) {
        let Some(player) = self.audio_player.as_mut() else {
            return;
        };
        if player.deleting {
            return;
        }
        // `isSessionAudioIdle`: no import or batch in flight for the session.
        if self
            .recording
            .batch
            .get(&player.session_id)
            .is_some_and(|batch| batch.error.is_none())
        {
            return;
        }
        player.deleting = true;
        let session_id = player.session_id.clone();
        self.stop_playback(cx);
        let task = self.store.delete_session_audio(session_id.clone());
        cx.spawn(async move |this, cx| {
            let result = task.await.map_err(anyhow::Error::from).and_then(|r| r);
            this.update(cx, |this, cx| {
                if let Err(error) = &result {
                    tracing::error!(%error, "[audio-player] failed to delete the recording");
                }
                if let Some(player) = this.audio_player.as_mut() {
                    player.deleting = false;
                }
                if result.is_ok() {
                    this.audio_player = None;
                    if this.selected.as_deref() == Some(session_id.as_str()) {
                        this.reload_note(session_id.clone(), cx);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// `Timeline`'s `useNativeContextMenu`: Play / Pause / Resume, Stop while
    /// not stopped, then Delete recording. The Tauri app shows the OS menu;
    /// here it is the app menu chrome.
    pub(super) fn render_audio_player_menu(
        &self,
        window: &gpui::Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        use super::menu::{Align, Entry, MenuSpec, Trailing};
        let player = self.audio_player.as_ref()?;
        let position = player.menu_at?;
        let mut entries = Vec::new();
        let (label, action): (&str, fn(&mut Workspace, &mut Context<Workspace>)) =
            match player.state {
                PlayerState::Paused => ("Resume", |this, cx| this.toggle_playback(cx)),
                PlayerState::Stopped => ("Play", |this, cx| this.toggle_playback(cx)),
                PlayerState::Playing => ("Pause", |this, cx| this.toggle_playback(cx)),
            };
        entries.push(Entry::Item {
            icon: None,
            dim_icon: false,
            label: label.into(),
            trailing: Trailing::None,
            destructive: false,
            on_select: Some(Box::new(move |this, _, cx| action(this, cx))),
            submenu: None,
        });
        if player.state != PlayerState::Stopped {
            entries.push(Entry::Item {
                icon: None,
                dim_icon: false,
                label: "Stop".into(),
                trailing: Trailing::None,
                destructive: false,
                on_select: Some(Box::new(|this, _, cx| this.stop_playback(cx))),
                submenu: None,
            });
        }
        entries.push(Entry::Separator);
        let deleting = player.deleting;
        entries.push(Entry::Item {
            icon: None,
            dim_icon: false,
            label: "Delete recording".into(),
            trailing: Trailing::None,
            destructive: false,
            on_select: (!deleting).then(|| -> super::menu::Select {
                Box::new(|this, _, cx| this.delete_recording(cx))
            }),
            submenu: None,
        });
        let spec = MenuSpec {
            id: "audio-player-menu",
            width: 176.0,
            entries,
            open_sub: None,
            on_hover_sub: |_, _, _| {},
            on_close: |this, cx| {
                if let Some(player) = this.audio_player.as_mut() {
                    player.menu_at = None;
                    cx.notify();
                }
            },
        };
        Some(self.render_app_menu(spec, position, Align::Start, window, cx))
    }

    /// `seekAndPlay(word)`: `seek(ms / 1000)` then `startPlayback()`.
    pub(super) fn seek_and_play(&mut self, ms: i64, cx: &mut Context<Self>) {
        let Some(player) = self.audio_player.as_mut() else {
            return;
        };
        let position = Duration::from_millis(ms.max(0) as u64);
        player.position = position;
        match player.state {
            PlayerState::Playing => {
                if let Some(playback) = &player.playback {
                    playback.seek(position);
                }
                cx.notify();
            }
            PlayerState::Paused => {
                if let Some(playback) = &player.playback {
                    playback.seek(position);
                    playback.resume();
                }
                player.state = PlayerState::Playing;
                self.tick_audio_player(cx);
                cx.notify();
            }
            PlayerState::Stopped => self.toggle_playback(cx),
        }
    }

    /// `timeStore.current` in milliseconds for the open session's player.
    pub(super) fn audio_position_ms(&self, session_id: &str) -> i64 {
        self.audio_player
            .as_ref()
            .filter(|player| player.session_id == session_id)
            .map(|player| player.position.as_millis() as i64)
            .unwrap_or(0)
    }

    /// wavesurfer `interaction` (`dragToSeek`): a press on the lane seeks.
    fn seek_audio_player(&mut self, fraction: f32, cx: &mut Context<Self>) {
        let Some(player) = self.audio_player.as_mut() else {
            return;
        };
        let Some(waveform) = &player.waveform else {
            return;
        };
        let position = waveform.duration.mul_f32(fraction.clamp(0.0, 1.0));
        player.position = position;
        if let Some(playback) = &player.playback {
            playback.seek(position);
        }
        cx.notify();
    }

    /// `showTopAudioPlayer`: the transcript view, audio present and decoded,
    /// and no capture running for the session.
    pub(super) fn render_top_audio_player(
        &mut self,
        preview: &crate::db::NotePreview,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !preview.audio_exists {
            return None;
        }
        let mode = self.session_mode(&preview.session.id);
        if matches!(
            mode,
            super::recording::SessionMode::Active | super::recording::SessionMode::Finalizing
        ) {
            return None;
        }
        self.ensure_audio_player(&preview.session.id, cx);
        let player = self.audio_player.as_ref()?;
        let waveform = player.waveform.clone()?;
        let theme = self.theme;
        let total = waveform.duration.as_secs_f64();
        let current = player.position.as_secs_f64();
        let playing = player.state == PlayerState::Playing;
        let position_fraction = if total > 0.0 {
            (current / total).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };

        // `h-7 w-7 rounded-full border bg-card shadow-xs` (`rounded-full` is
        // 0.5rem in the app's CSS) with the filled Play / Pause glyph.
        let button = div()
            .id("audio-player-toggle")
            .flex()
            .size(px(28.0))
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .rounded(px(8.0))
            .border_1()
            .border_color(theme.border)
            .bg(theme.card)
            .shadow_xs()
            .cursor_pointer()
            .hover(|s| s.bg(theme.accent))
            .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| this.toggle_playback(cx)))
            .child(crate::ui::icon(
                if playing { "pause" } else { "play" },
                px(14.0),
                theme.foreground,
            ));

        // `TimelineMeta`: `font-mono text-xs tabular-nums text-muted-foreground gap-1`.
        let meta = div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap_1()
            .when_some(self.mono_font_family.clone(), |meta, family| {
                meta.font_family(family)
            })
            .tw_text_xs()
            .text_color(theme.muted_foreground)
            .child(SharedString::from(crate::audio_player::format_time(
                current,
            )))
            .child("/")
            .child(SharedString::from(crate::audio_player::format_time(total)));

        // The lane's laid-out bounds, shared by the canvas and the press
        // handler so a press maps to a fraction of the lane.
        let lane_bounds: Rc<Cell<Option<gpui::Bounds<gpui::Pixels>>>> = Rc::new(Cell::new(None));
        let lane = div()
            .id("audio-player-lane")
            .h(px(crate::audio_player::WAVE_HEIGHT))
            .min_w_0()
            .flex_1()
            .cursor_pointer()
            .on_mouse_down(gpui::MouseButton::Left, {
                let lane_bounds = lane_bounds.clone();
                cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                    if let Some(bounds) = lane_bounds.get()
                        && bounds.size.width > px(0.0)
                    {
                        let fraction = f32::from(event.position.x - bounds.left())
                            / f32::from(bounds.size.width);
                        this.seek_audio_player(fraction, cx);
                    }
                })
            })
            .child(waveform_canvas(waveform, position_fraction, lane_bounds));

        Some(
            div()
                .flex_shrink_0()
                .px_1()
                .pt_1()
                .pb_2()
                .on_mouse_down(
                    gpui::MouseButton::Right,
                    cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                        if let Some(player) = this.audio_player.as_mut() {
                            player.menu_at = Some(event.position);
                            cx.notify();
                        }
                    }),
                )
                .child(
                    div()
                        .overflow_hidden()
                        .rounded(px(22.0))
                        .border_1()
                        .border_color(crate::theme::alpha(theme.border, 0.7))
                        .bg(crate::theme::alpha(theme.card, 0.8))
                        .child(
                            div()
                                .flex()
                                .w_full()
                                .items_center()
                                .gap_2()
                                .pl_1()
                                .pr_3()
                                .py(px(6.0))
                                .child(button)
                                .child(meta)
                                .child(lane),
                        ),
                )
                .into_any_element(),
        )
    }
}

/// wavesurfer's bar renderer: `barWidth 3`, `barGap 2`, `barRadius 2`,
/// bars centred vertically, the first two channels overlaid in their own
/// colours, `progressColor` left of the cursor, and the 2px cursor.
fn waveform_canvas(
    waveform: Arc<Waveform>,
    position: f32,
    lane_bounds: Rc<Cell<Option<gpui::Bounds<gpui::Pixels>>>>,
) -> impl IntoElement {
    gpui::canvas(
        move |bounds, _, _| lane_bounds.set(Some(bounds)),
        move |bounds, _, window, _| {
            let width = f32::from(bounds.size.width);
            let height = f32::from(bounds.size.height);
            let cursor_x = width * position;
            let pitch = crate::audio_player::BAR_WIDTH + crate::audio_player::BAR_GAP;
            for (channel, (wave, progress)) in
                crate::audio_player::CHANNEL_COLORS.iter().enumerate()
            {
                for (index, peak) in waveform.bars(channel, width).into_iter().enumerate() {
                    let x = index as f32 * pitch;
                    let bar_height = (peak * height).max(1.0).round();
                    let top = ((height - bar_height) / 2.0).round();
                    let color = if x < cursor_x { *progress } else { *wave };
                    window.paint_quad(
                        gpui::fill(
                            gpui::Bounds::new(
                                gpui::point(bounds.left() + px(x), bounds.top() + px(top)),
                                gpui::size(px(crate::audio_player::BAR_WIDTH), px(bar_height)),
                            ),
                            gpui::rgb(color),
                        )
                        .corner_radii(px(crate::audio_player::BAR_RADIUS.min(bar_height / 2.0))),
                    );
                }
            }
            let cursor_left = cursor_x
                .min(width - crate::audio_player::CURSOR_WIDTH)
                .max(0.0);
            window.paint_quad(gpui::fill(
                gpui::Bounds::new(
                    gpui::point(bounds.left() + px(cursor_left), bounds.top()),
                    gpui::size(px(crate::audio_player::CURSOR_WIDTH), bounds.size.height),
                ),
                gpui::rgb(crate::audio_player::CURSOR_COLOR),
            ));
        },
    )
    .size_full()
}
