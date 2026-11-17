use gpui::{Context, Window};

use super::ApplicationView;
use crate::{
    contracts::{MeetingIntent, ProductRoute},
    meeting::automation::{Observed, Request},
    workspace::navigation::Route,
};

impl ApplicationView {
    /// Carries out what a notification button or meeting detection asks of the app.
    pub(super) fn automation_request(
        &mut self,
        request: &Request,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match request {
            Request::ShowMain => {
                cx.activate(true);
                window.activate_window();
            }
            Request::OpenSession { session, listen } => {
                self.leave_product(Route::Session(session.clone()), cx);
                if *listen && self.product.is_none() {
                    self.open_meeting(
                        MeetingIntent::Start {
                            session_id: session.clone(),
                        },
                        window,
                        cx,
                    );
                }
            }
            Request::NewMeeting if !self.writers_paused && !self.closing => {
                if self.product.is_some() {
                    self.leave_product(Route::Empty, cx);
                }
                self.workspace.update(cx, |view, cx| view.create(true, cx));
            }
            Request::NewMeeting => self.status("Finish pending work before starting capture.", cx),
            Request::OpenSettings(section) => {
                self.open_product(ProductRoute::Settings, window, cx);
                if let Some(product) = &self.product {
                    product.update(cx, |product, cx| product.navigate_section(section, cx));
                }
            }
            Request::Stop => self.open_meeting(MeetingIntent::Stop, window, cx),
        }
    }

    /// Detection's microphone and notification events drive scheduling's auto-stop.
    pub(super) fn meeting_observed(&mut self, observed: &Observed, cx: &mut Context<Self>) {
        match observed {
            Observed::MicDetected { apps, .. } => self.meeting_mic_started(apps, cx),
            Observed::MicStopped(apps) => self.meeting_mic_stopped(apps, cx),
            Observed::Notification { key, action, .. } => {
                self.schedule_notification_action(key, *action, cx);
            }
            Observed::Sleep(_) => {}
        }
    }
}
