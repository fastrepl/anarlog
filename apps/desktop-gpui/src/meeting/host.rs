//! What the application hosts for a meeting: the live state every surface reads, start/stop
//! requests, and shipping's floating bar window (`meeting-float/host.tsx`), shown while a
//! meeting is actively listening.

use desktop_runtime::{Result, ServiceError, SessionId};
use gpui::{
    App, AppContext, Bounds, Entity, Pixels, Point, Subscription, WindowBackgroundAppearance,
    WindowBounds, WindowHandle, WindowKind, WindowOptions, point, px,
};
use tokio::sync::oneshot;

use super::MeetingServices;
use super::floating::{FloatingBar, FloatingEvent, container_size, shows_expand};
use super::live::{LiveState, SessionMode};
use super::playback;

type Reply = oneshot::Receiver<Result<()>>;

struct SavedFloating {
    session: SessionId,
    origin: Point<Pixels>,
    expanded: bool,
}

/// Starts capture for a session, pausing playback first like shipping's listener.
pub fn start(services: &MeetingServices, session: SessionId) -> Result<Reply> {
    services.playback.send(playback::Command::Pause)?;
    services.capture.start(session)
}

pub fn stop(services: &MeetingServices) -> Result<Reply> {
    services.capture.stop()
}

/// Awaits a capture request's acknowledgement.
pub async fn acknowledged(reply: Result<Reply>) -> Result<()> {
    reply?.await.unwrap_or(Err(ServiceError::Closed))
}

pub struct MeetingHost {
    live: Entity<LiveState>,
    recording: Option<SessionId>,
    floating: Option<(WindowHandle<FloatingBar>, Subscription)>,
    saved_floating: Option<SavedFloating>,
}

impl MeetingHost {
    pub fn install(cx: &mut App) -> Self {
        Self {
            live: LiveState::install(cx),
            recording: None,
            floating: None,
            saved_floating: None,
        }
    }

    pub fn live(&self) -> &Entity<LiveState> {
        &self.live
    }

    pub fn tick(&self, services: &MeetingServices, cx: &mut App) {
        self.live
            .update(cx, |live, cx| live.sync(&services.capture, cx));
    }

    /// Sessions whose live state flipped since the last call, for the workspace's markers.
    pub fn recording_changes(&mut self, cx: &App) -> Vec<(SessionId, bool)> {
        let current = self
            .live
            .read(cx)
            .current()
            .filter(|live| live.live())
            .map(|live| live.session.clone());
        if current == self.recording {
            return Vec::new();
        }
        let mut changes = Vec::new();
        if let Some(previous) = self.recording.take() {
            changes.push((previous, false));
        }
        if let Some(current) = &current {
            changes.push((current.clone(), true));
        }
        self.recording = current;
        changes
    }

    /// Shipping's "Open floating panel": brings the live meeting's floating bar forward.
    pub fn show_floating(&self, cx: &mut App) {
        if let Some((handle, _)) = &self.floating {
            let _ = handle.update(cx, |_, window, _| window.activate_window());
        }
    }

    /// True while the floating bar window is open.
    pub fn floating_open(&self) -> bool {
        self.floating.is_some()
    }

    #[cfg(test)]
    pub(crate) fn floating_expanded(&self, cx: &App) -> Option<bool> {
        self.floating
            .as_ref()
            .and_then(|(handle, _)| handle.read_with(cx, |bar, _| bar.expanded()).ok())
    }

    #[cfg(test)]
    pub(crate) fn floating_bounds(&self, cx: &mut App) -> Option<Bounds<Pixels>> {
        self.floating
            .as_ref()
            .and_then(|(handle, _)| handle.update(cx, |_, window, _| window.bounds()).ok())
    }

    #[cfg(test)]
    pub(crate) fn set_floating_expanded(&self, expanded: bool, cx: &mut App) {
        if let Some((handle, _)) = &self.floating {
            let _ = handle.update(cx, |bar, _, cx| bar.set_expanded(expanded, cx));
        }
    }

    /// Shows the floating bar when enabled while a meeting is listening and closes it otherwise.
    pub fn sync_floating(
        &mut self,
        enabled: bool,
        services: Option<&MeetingServices>,
        cx: &mut App,
    ) -> Result<()> {
        let live = self
            .live
            .read(cx)
            .current()
            .filter(|live| live.mode == SessionMode::Active)
            .cloned();
        if self.saved_floating.as_ref().is_some_and(|saved| {
            live.as_ref()
                .is_none_or(|live| live.session != saved.session)
        }) {
            self.saved_floating = None;
        }
        let target = if enabled && services.is_some() {
            live.as_ref()
        } else {
            None
        };
        match (&self.floating, target, services) {
            (None, Some(live), Some(services)) => {
                let saved = self
                    .saved_floating
                    .take()
                    .filter(|saved| saved.session == live.session);
                let expanded =
                    saved.as_ref().is_some_and(|saved| saved.expanded) && shows_expand(live);
                let size =
                    crate::zoom::window_size(container_size(expanded, shows_expand(live)), cx);
                let bounds = saved
                    .map(|saved| Bounds::new(saved.origin, size))
                    .unwrap_or_else(|| match cx.primary_display() {
                        // Bottom center of the visible screen, 8px above its edge.
                        Some(display) => {
                            let visible = display.visible_bounds();
                            Bounds::new(
                                point(
                                    visible.center().x - size.width / 2.,
                                    visible.bottom() - size.height - px(8.),
                                ),
                                size,
                            )
                        }
                        None => Bounds::centered(None, size, cx),
                    });
                let options = WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: None,
                    focus: false,
                    // On macOS, GPUI orders the window front during `open_window` before the root
                    // builder strips `Titled`; keep it hidden until `configure_floating_panel`
                    // applies the borderless panel configuration and orders it front.
                    show: !cfg!(target_os = "macos"),
                    kind: WindowKind::PopUp,
                    is_movable: true,
                    is_resizable: false,
                    is_minimizable: false,
                    window_background: WindowBackgroundAppearance::Transparent,
                    app_id: crate::platform::package::window_app_id(),
                    ..Default::default()
                };
                let state = self.live.clone();
                let services = services.clone();
                let mut subscription = None;
                let handle = cx
                    .open_window(options, |window, cx| {
                        crate::platform::windows::configure_floating_panel(window);
                        let bar = cx.new(|cx| FloatingBar::new(state, cx));
                        if expanded {
                            bar.update(cx, |bar, cx| bar.set_expanded(true, cx));
                        }
                        subscription = Some(cx.subscribe(&bar, move |_, event, cx| {
                            let FloatingEvent::Stop = event;
                            let reply = stop(&services);
                            cx.background_executor()
                                .spawn(async move {
                                    if let Err(error) = acknowledged(reply).await {
                                        tracing::warn!(%error, "floating bar stop failed");
                                    }
                                })
                                .detach();
                        }));
                        bar
                    })
                    .map_err(|error| ServiceError::Failed(error.to_string().into()))?;
                if let Some(subscription) = subscription {
                    self.floating = Some((handle, subscription));
                }
            }
            (Some(_), None, _) => {
                if let Some((handle, _)) = self.floating.take() {
                    let saved = handle
                        .update(cx, |bar, window, _| {
                            let origin = window.bounds().origin;
                            let expanded = bar.expanded();
                            window.remove_window();
                            (origin, expanded)
                        })
                        .ok();
                    if let (Some(live), Some((origin, expanded))) = (live.as_ref(), saved) {
                        self.saved_floating = Some(SavedFloating {
                            session: live.session.clone(),
                            origin,
                            expanded,
                        });
                    } else {
                        self.saved_floating = None;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}
