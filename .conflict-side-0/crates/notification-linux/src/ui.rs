use std::sync::Arc;

use gtk4::gdk::Display;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, Image, Label,
    Orientation, Overlay, Revealer, RevealerTransitionType,
};
use indexmap::IndexMap;

const NOTIFICATION_WIDTH: i32 = 360;
const NOTIFICATION_HEIGHT: i32 = 64;
const NOTIFICATION_SPACING: i32 = 10;
const NOTIFICATION_MARGIN: i32 = 15;
const MAX_NOTIFICATIONS: usize = 5;
const SLIDE_DURATION_MS: u32 = 250;

pub struct NotificationInstance {
    id: String,
    window: ApplicationWindow,
    revealer: Revealer,
    timeout_source: Option<glib::SourceId>,
    is_hovered: Arc<std::sync::Mutex<bool>>,
}

impl NotificationInstance {
    pub fn new(
        app: &Application,
        id: String,
        title: &str,
        message: &str,
        url: Option<&str>,
        on_confirm: impl Fn(String) + 'static,
        on_dismiss: impl Fn(String) + 'static,
    ) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .decorated(false)
            .resizable(false)
            .default_width(NOTIFICATION_WIDTH)
            .default_height(NOTIFICATION_HEIGHT)
            .build();

        let revealer = Revealer::builder()
            .transition_type(RevealerTransitionType::SlideLeft)
            .transition_duration(SLIDE_DURATION_MS)
            .reveal_child(false)
            .build();

        let content =
            Self::build_content(title, message, url, &id, &window, on_confirm, on_dismiss);
        revealer.set_child(Some(&content));
        window.set_child(Some(&revealer));

        let is_hovered = Arc::new(std::sync::Mutex::new(false));

        let hover_controller = gtk4::EventControllerMotion::new();
        let is_hovered_enter = is_hovered.clone();
        hover_controller.connect_enter(move |_, _, _| {
            *is_hovered_enter.lock().unwrap() = true;
        });

        let is_hovered_leave = is_hovered.clone();
        hover_controller.connect_leave(move |_| {
            *is_hovered_leave.lock().unwrap() = false;
        });

        window.add_controller(hover_controller);

        Self {
            id,
            window,
            revealer,
            timeout_source: None,
            is_hovered,
        }
    }

    fn build_content(
        title: &str,
        message: &str,
        url: Option<&str>,
        id: &str,
        window: &ApplicationWindow,
        on_confirm: impl Fn(String) + 'static,
        on_dismiss: impl Fn(String) + 'static,
    ) -> Overlay {
        let main_box = GtkBox::new(Orientation::Horizontal, 8);
        main_box.set_margin_start(12);
        main_box.set_margin_end(12);
        main_box.set_margin_top(9);
        main_box.set_margin_bottom(9);
        main_box.set_valign(Align::Center);

        let icon = Image::from_icon_name("application-x-executable");
        icon.set_pixel_size(24);
        main_box.append(&icon);

        let text_box = GtkBox::new(Orientation::Vertical, 2);
        text_box.set_hexpand(true);

        let title_label = Label::new(Some(title));
        title_label.set_halign(Align::Start);
        title_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        title_label.add_css_class("notification-title");
        text_box.append(&title_label);

        let message_label = Label::new(Some(message));
        message_label.set_halign(Align::Start);
        message_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        message_label.add_css_class("notification-message");
        text_box.append(&message_label);

        main_box.append(&text_box);

        if let Some(url_str) = url {
            if !url_str.is_empty() {
                let action_button = Button::with_label("Take Notes");
                action_button.add_css_class("action-button");

                let id_clone = id.to_string();
                let url_clone = url_str.to_string();
                let window_clone = window.clone();
                action_button.connect_clicked(move |_| {
                    on_confirm(id_clone.clone());
                    let _ = open::that(&url_clone);
                    Self::dismiss_window_static(&window_clone, &id_clone, true);
                });

                main_box.append(&action_button);
            }
        }

        let close_button = Button::new();
        close_button.set_label("×");
        close_button.add_css_class("close-button");
        close_button.set_valign(Align::Start);
        close_button.set_halign(Align::End);

        let id_clone = id.to_string();
        let window_clone = window.clone();
        close_button.connect_clicked(move |_| {
            on_dismiss(id_clone.clone());
            Self::dismiss_window_static(&window_clone, &id_clone, true);
        });

        let overlay = Overlay::new();
        overlay.set_child(Some(&main_box));
        overlay.add_overlay(&close_button);

        overlay
    }

    pub fn show(&mut self) {
        self.window.present();

        let revealer = self.revealer.clone();
        glib::idle_add_local_once(move || {
            revealer.set_reveal_child(true);
        });
    }

    pub fn start_dismiss_timer(&mut self, timeout_seconds: f64) {
        if let Some(source) = self.timeout_source.take() {
            source.remove();
        }

        let id = self.id.clone();
        let window = self.window.clone();
        let is_hovered = self.is_hovered.clone();
        let timeout_ms = (timeout_seconds * 1000.0) as u64;
        let start_time = std::time::Instant::now();

        let source = glib::timeout_add_local(std::time::Duration::from_millis(100), {
            let id = id.clone();
            let window = window.clone();
            let is_hovered = is_hovered.clone();
            move || {
                if *is_hovered.lock().unwrap() {
                    return glib::ControlFlow::Continue;
                }

                if start_time.elapsed().as_millis() >= timeout_ms as u128 {
                    Self::dismiss_window_static(&window, &id, false);
                    return glib::ControlFlow::Break;
                }

                glib::ControlFlow::Continue
            }
        });

        self.timeout_source = Some(source);
    }

    fn dismiss_window_static(window: &ApplicationWindow, id: &str, _user_action: bool) {
        if let Some(child) = window.child() {
            if let Some(revealer) = child.downcast_ref::<Revealer>() {
                revealer.set_reveal_child(false);

                let window_clone = window.clone();
                let id_clone = id.to_string();
                revealer.connect_child_revealed_notify(move |rev| {
                    if !rev.reveals_child() {
                        window_clone.close();
                        crate::remove_notification(&id_clone);
                    }
                });
            }
        }
    }

    pub fn dismiss(&mut self, user_action: bool) {
        if let Some(source) = self.timeout_source.take() {
            source.remove();
        }
        Self::dismiss_window_static(&self.window, &self.id, user_action);
    }
}

pub struct NotificationManager {
    active_notifications: IndexMap<String, NotificationInstance>,
}

impl NotificationManager {
    pub fn new() -> Self {
        Self {
            active_notifications: IndexMap::new(),
        }
    }

    fn ensure_app(&mut self) -> Application {
        static INIT_ONCE: std::sync::Once = std::sync::Once::new();
        INIT_ONCE.call_once(|| {
            gtk4::init().ok();
            Self::setup_styles();
        });

        Application::builder()
            .application_id("com.hyprnote.notifications")
            .build()
    }

    fn setup_styles() {
        let css_provider = CssProvider::new();
        css_provider.load_from_data(
            r#"
            window {
                background-color: rgba(255, 255, 255, 0.95);
                border-radius: 11px;
                border: 1px solid rgba(0, 0, 0, 0.1);
                box-shadow: 0 2px 12px rgba(0, 0, 0, 0.22);
            }
            .notification-title {
                font-size: 14px;
                font-weight: bold;
                color: #000000;
            }
            .notification-message {
                font-size: 11px;
                color: #666666;
            }
            .close-button {
                min-width: 15px;
                min-height: 15px;
                border-radius: 7.5px;
                background-color: rgba(0, 0, 0, 0.5);
                border: 0.5px solid rgba(0, 0, 0, 0.3);
                color: white;
                padding: 0;
                margin: 5px;
            }
            .close-button:hover {
                background-color: rgba(0, 0, 0, 0.6);
            }
            .action-button {
                border-radius: 10px;
                background-color: rgba(242, 242, 242, 0.9);
                border: 0.5px solid rgba(179, 179, 179, 0.5);
                color: rgba(26, 26, 26, 1.0);
                font-size: 14px;
                font-weight: 600;
                padding: 6px 11px;
                min-height: 28px;
            }
            .action-button:hover {
                background-color: rgba(230, 230, 230, 0.9);
            }
            "#,
        );

        if let Some(display) = Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &css_provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    }

    pub fn show(
        &mut self,
        title: String,
        message: String,
        url: Option<String>,
        timeout_seconds: f64,
        on_confirm: impl Fn(String) + 'static,
        on_dismiss: impl Fn(String) + 'static,
    ) {
        let app = self.ensure_app();

        while self.active_notifications.len() >= MAX_NOTIFICATIONS {
            if let Some((oldest_id, _)) = self.active_notifications.first() {
                let oldest_id = oldest_id.clone();
                if let Some(mut notif) = self.active_notifications.shift_remove(&oldest_id) {
                    notif.dismiss(false);
                }
            }
        }

        let id = uuid::Uuid::new_v4().to_string();

        let mut notif = NotificationInstance::new(
            &app,
            id.clone(),
            &title,
            &message,
            url.as_deref(),
            on_confirm,
            on_dismiss,
        );

        notif.show();
        notif.start_dismiss_timer(timeout_seconds);

        self.active_notifications.insert(id, notif);
    }

    pub fn remove_notification(&mut self, id: &str) {
        self.active_notifications.shift_remove(id);
    }

    pub fn dismiss_all(&mut self) {
        let ids: Vec<String> = self.active_notifications.keys().cloned().collect();
        for id in ids {
            if let Some(mut notif) = self.active_notifications.shift_remove(&id) {
                notif.dismiss(false);
            }
        }
    }
}
