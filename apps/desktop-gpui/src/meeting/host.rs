//! What the application hosts for a meeting: the live state every surface reads, start/stop
//! requests, and shipping's floating bar window (`meeting-float/host.tsx`), shown while a
//! meeting is actively listening.

use desktop_runtime::{Result, ServiceError, SessionId};
use gpui::{
    App, AppContext, Bounds, Entity, Subscription, WindowBackgroundAppearance, WindowBounds,
    WindowHandle, WindowKind, WindowOptions, point, px,
};
use tokio::sync::oneshot;

use super::MeetingServices;
use super::floating::{FloatingBar, FloatingEvent, container_size, shows_expand};
use super::live::{LiveState, SessionMode};
use super::playback;

type Reply = oneshot::Receiver<Result<()>>;

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
}

impl MeetingHost {
    pub fn install(cx: &mut App) -> Self {
        Self {
            live: LiveState::install(cx),
            recording: None,
            floating: None,
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

    /// Shows the floating bar while a meeting is listening and closes it otherwise.
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
        match (&self.floating, live.filter(|_| enabled), services) {
            (None, Some(live), Some(services)) => {
                let size = crate::zoom::window_size(container_size(false, shows_expand(&live)), cx);
                let bounds = match cx.primary_display() {
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
                };
                let options = WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: None,
                    focus: false,
                    show: true,
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
                    .open_window(options, |_, cx| {
                        let bar = cx.new(|cx| FloatingBar::new(state, cx));
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
            (Some(_), None, _) | (Some(_), _, None) => {
                if let Some((handle, _)) = self.floating.take() {
                    let _ = handle.update(cx, |_, window, _| window.remove_window());
                }
            }
            _ => {}
        }
        Ok(())
    }
}
