use std::cell::RefCell;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{
    Align, Box as GtkBox, Button, CssProvider, EventBox, Image, Label, Menu, MenuButton, MenuItem,
    Orientation, StyleContext, Window, WindowType,
};
use indexmap::IndexMap;

use crate::callbacks;

thread_local! {
    static NOTIFICATION_MANAGER: RefCell<NotificationManager> = RefCell::new(NotificationManager::new());
}

struct NotificationInstance {
    key: String,
    window: Window,
    timeout_source: Option<glib::SourceId>,
}

impl NotificationInstance {
    fn new(window: Window, key: String) -> Self {
        Self {
            key,
            window,
            timeout_source: None,
        }
    }

    fn start_dismiss_timer(&mut self, timeout_seconds: f64) {
        if let Some(source) = self.timeout_source.take() {
            source.remove();
        }

        let key = self.key.clone();
        let window = self.window.clone();
        let source = glib::timeout_add_seconds_local_once(timeout_seconds as u32, move || {
            callbacks::timeout(key.clone());
            Self::dismiss_window_inner(&window, &key, false);
            NotificationManager::remove_notification_global(&key, false);
        });
        self.timeout_source = Some(source);
    }

    fn dismiss_window_inner(window: &Window, key: &str, user_action: bool) {
        if user_action {
            callbacks::dismiss(key.to_string());
        }

        window.set_sensitive(false);
        window.set_opacity(1.0);
        let window_clone = window.clone();
        glib::timeout_add_local_once(Duration::from_millis(200), move || {
            window_clone.close();
        });
    }

    fn dismiss_window(window: &Window, key: &str, user_action: bool) {
        Self::dismiss_window_inner(window, key, user_action);
        NotificationManager::remove_notification_global(key, true);
    }
}

struct NotificationManager {
    active_notifications: IndexMap<String, NotificationInstance>,
    max_notifications: usize,
    #[allow(dead_code)]
    notification_spacing: i32,
}

impl NotificationManager {
    fn new() -> Self {
        Self {
            active_notifications: IndexMap::new(),
            max_notifications: 5,
            notification_spacing: 10,
        }
    }

    fn ensure_gtk(&self) -> bool {
        match gtk::init() {
            Ok(_) => true,
            Err(e) => {
                eprintln!("[notification-linux] Failed to initialize GTK: {}", e);
                false
            }
        }
    }

    fn show(&mut self, notification: hypr_notification_interface::Notification) {
        if !self.ensure_gtk() {
            return;
        }

        let key = notification
            .key
            .clone()
            .unwrap_or_else(|| notification.title.clone());
        let timeout_seconds = notification.timeout.map(|d| d.as_secs_f64()).unwrap_or(0.0);

        while self.active_notifications.len() >= self.max_notifications {
            if let Some((oldest_id, notif)) = self.active_notifications.get_index(0) {
                let oldest_id = oldest_id.clone();
                let window = notif.window.clone();
                NotificationInstance::dismiss_window_inner(&window, &oldest_id, false);
                self.remove_notification_locked(&oldest_id, true);
            } else {
                break;
            }
        }

        let window = Window::new(WindowType::Toplevel);
        window.set_decorated(false);
        window.set_resizable(false);
        window.set_default_size(
            360,
            if notification.footer.is_some() {
                96
            } else {
                64
            },
        );
        window.set_keep_above(true);

        self.setup_window_style(&window);
        self.create_notification_content(&window, &notification, &key);
        self.position_window(&window);

        window.show_all();

        let mut notif = NotificationInstance::new(window, key.clone());
        if timeout_seconds > 0.0 {
            notif.start_dismiss_timer(timeout_seconds);
        }
        self.active_notifications.insert(key, notif);

        self.reposition_notifications();
    }

    fn setup_window_style(&self, _window: &Window) {
        let css_provider = CssProvider::new();
        let _ = css_provider.load_from_data(
            br#"
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
            .destructive-action-button {
                background-color: rgba(196, 43, 28, 0.95);
                color: white;
            }
            .destructive-action-button:hover {
                background-color: rgba(166, 34, 23, 0.95);
            }
            .notification-footer {
                border-top: 1px solid rgba(0, 0, 0, 0.1);
                padding-top: 4px;
            }
            .notification-footer-label {
                font-size: 10px;
                color: #666666;
            }
            "#,
        );

        if let Some(screen) = gdk::Screen::default() {
            StyleContext::add_provider_for_screen(
                &screen,
                &css_provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    }

    fn create_notification_content(
        &self,
        window: &Window,
        notification: &hypr_notification_interface::Notification,
        key: &str,
    ) {
        let main_box = GtkBox::new(Orientation::Horizontal, 8);
        main_box.set_margin_start(12);
        main_box.set_margin_end(12);
        main_box.set_margin_top(9);
        main_box.set_margin_bottom(9);
        main_box.set_valign(Align::Center);

        let content_box = GtkBox::new(Orientation::Horizontal, 8);
        let icon = Image::from_icon_name(Some("application-x-executable"), gtk::IconSize::Dnd);
        content_box.pack_start(&icon, false, false, 0);

        let text_box = GtkBox::new(Orientation::Vertical, 2);
        text_box.set_hexpand(true);

        let title_label = Label::new(Some(&notification.title));
        title_label.set_halign(Align::Start);
        title_label.set_ellipsize(pango::EllipsizeMode::End);
        title_label.style_context().add_class("notification-title");
        text_box.pack_start(&title_label, false, false, 0);

        let message_label = Label::new(Some(&notification.message));
        message_label.set_halign(Align::Start);
        message_label.set_ellipsize(pango::EllipsizeMode::End);
        message_label
            .style_context()
            .add_class("notification-message");
        text_box.pack_start(&message_label, false, false, 0);

        content_box.pack_start(&text_box, true, true, 0);

        let content_event_box = EventBox::new();
        content_event_box.set_visible_window(false);
        content_event_box.add(&content_box);
        main_box.pack_start(&content_event_box, true, true, 0);

        let options_button = match callbacks::primary_action(notification) {
            callbacks::PrimaryAction::Options(options) => {
                let menu = Menu::new();
                for (index, option) in options.iter().enumerate() {
                    let menu_item = MenuItem::with_label(option);
                    let option_key = key.to_string();
                    let option_window = window.clone();
                    menu_item.connect_activate(move |_| {
                        callbacks::option_selected(option_key.clone(), index as i32);
                        NotificationInstance::dismiss_window(&option_window, &option_key, false);
                    });
                    menu.append(&menu_item);
                }

                let create_new_item = MenuItem::with_label("Create New Note...");
                let create_new_index = options.len() as i32;
                let option_key = key.to_string();
                let option_window = window.clone();
                create_new_item.connect_activate(move |_| {
                    callbacks::option_selected(option_key.clone(), create_new_index);
                    NotificationInstance::dismiss_window(&option_window, &option_key, false);
                });
                menu.append(&create_new_item);
                menu.show_all();

                let menu_button = MenuButton::new();
                menu_button.set_label("Options");
                menu_button.style_context().add_class("action-button");
                menu_button.set_popup(Some(&menu));
                main_box.pack_start(&menu_button, false, false, 0);
                Some(menu_button)
            }
            callbacks::PrimaryAction::Accept { label, destructive } => {
                let action_button = Button::with_label(label);
                action_button.style_context().add_class("action-button");
                if destructive {
                    action_button
                        .style_context()
                        .add_class("destructive-action-button");
                }

                let action_key = key.to_string();
                let action_window = window.clone();
                action_button.connect_clicked(move |_| {
                    callbacks::accept(action_key.clone());
                    NotificationInstance::dismiss_window(&action_window, &action_key, false);
                });
                main_box.pack_start(&action_button, false, false, 0);
                None
            }
        };

        let content_key = key.to_string();
        let content_window = window.clone();
        content_event_box.connect_button_press_event(move |_, _| {
            if let Some(options_button) = &options_button {
                options_button.clicked();
            } else {
                callbacks::confirm(content_key.clone());
                NotificationInstance::dismiss_window(&content_window, &content_key, false);
            }

            glib::Propagation::Stop
        });

        let close_button = Button::new();
        close_button.set_label("×");
        close_button.style_context().add_class("close-button");
        close_button.set_valign(Align::Start);
        close_button.set_halign(Align::End);

        let key_clone = key.to_string();
        let window_clone = window.clone();
        close_button.connect_clicked(move |_| {
            NotificationInstance::dismiss_window(&window_clone, &key_clone, true);
        });
        main_box.pack_start(&close_button, false, false, 0);

        let root_box = GtkBox::new(Orientation::Vertical, 0);
        root_box.pack_start(&main_box, true, true, 0);

        if let Some(footer) = &notification.footer {
            let footer_box = GtkBox::new(Orientation::Horizontal, 8);
            footer_box.set_margin_start(12);
            footer_box.set_margin_end(12);
            footer_box.set_margin_bottom(5);
            footer_box.style_context().add_class("notification-footer");

            let footer_label = Label::new(Some(&footer.text));
            footer_label.set_halign(Align::Start);
            footer_label.set_ellipsize(pango::EllipsizeMode::End);
            footer_label
                .style_context()
                .add_class("notification-footer-label");
            footer_box.pack_start(&footer_label, true, true, 0);

            let footer_button = Button::with_label(&footer.action_label);
            let footer_key = key.to_string();
            let footer_window = window.clone();
            footer_button.connect_clicked(move |_| {
                callbacks::footer_action(footer_key.clone());
                NotificationInstance::dismiss_window(&footer_window, &footer_key, false);
            });
            footer_box.pack_start(&footer_button, false, false, 0);
            root_box.pack_start(&footer_box, false, false, 0);
        }

        window.add(&root_box);
    }

    fn position_window(&self, window: &Window) {
        // Position the window in the top-right corner of the screen
        // Use the default width we set (360) since window.size() returns 0 before realization
        const DEFAULT_WINDOW_WIDTH: i32 = 360;

        if let Some(screen) = gdk::Screen::default()
            && let Some(root_window) = screen.root_window()
        {
            let screen_width = root_window.width();
            let x = screen_width - DEFAULT_WINDOW_WIDTH - 20;
            let y = 50;
            window.move_(x, y);
        }
    }

    fn reposition_notifications(&mut self) {
        // TODO: Reposition existing notifications once we implement a GTK4-compatible positioning strategy.
    }

    fn remove_notification_locked(&mut self, key: &str, cancel_timeout: bool) {
        if let Some(mut notification) = self.active_notifications.swap_remove(key)
            && cancel_timeout
            && let Some(source) = notification.timeout_source.take()
        {
            source.remove();
        }
        self.reposition_notifications();
    }

    fn remove_notification_global(key: &str, cancel_timeout: bool) {
        NOTIFICATION_MANAGER.with(|manager| {
            manager
                .borrow_mut()
                .remove_notification_locked(key, cancel_timeout);
        });
    }

    fn dismiss_all(&mut self) {
        let keys: Vec<String> = self.active_notifications.keys().cloned().collect();
        for key in keys {
            if let Some(notif) = self.active_notifications.get(&key) {
                let window = notif.window.clone();
                NotificationInstance::dismiss_window_inner(&window, &key, false);
                self.remove_notification_locked(&key, true);
            }
        }
    }
}

pub fn show(notification: &hypr_notification_interface::Notification) {
    let notification = notification.clone();

    glib::MainContext::default().invoke(move || {
        NOTIFICATION_MANAGER.with(|manager| {
            manager.borrow_mut().show(notification);
        });
    });
}

pub fn dismiss_all() {
    glib::MainContext::default().invoke(|| {
        NOTIFICATION_MANAGER.with(|manager| {
            manager.borrow_mut().dismiss_all();
        });
    });
}
