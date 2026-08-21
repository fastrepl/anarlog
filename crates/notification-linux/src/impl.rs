use std::cell::RefCell;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gdk::prelude::*;
use gtk::gdk::NotifyType;
use gtk::prelude::*;
use gtk::{
    Align, Box as GtkBox, Button, CssProvider, EventBox, Image, Label, Menu, MenuButton, MenuItem,
    Orientation, Overlay, ProgressBar, StyleContext, Window, WindowType,
};
use indexmap::IndexMap;

use anlg_notification_interface::{
    DismissTimer, ParticipantStatus, PrimaryAction, expanded_schedule_text,
};

use crate::callbacks;
use crate::icon;

const NOTIFICATION_WIDTH: i32 = 344;
const COMPACT_HEIGHT: i32 = 64;
const COMPACT_FOOTER_HEIGHT: i32 = 28;
const EXPANDED_HEIGHT: i32 = 380;
const RIGHT_MARGIN: i32 = 15;
const TOP_MARGIN: i32 = 15;
const NOTIFICATION_SPACING: i32 = 10;
const MAX_NOTIFICATIONS: usize = 5;
const TICK_INTERVAL: Duration = Duration::from_millis(50);

thread_local! {
    static NOTIFICATION_MANAGER: RefCell<NotificationManager> =
        RefCell::new(NotificationManager::new());
}

struct NotificationInstance {
    key: String,
    payload: anlg_notification_interface::Notification,
    window: Window,
    timeout_source: Option<glib::SourceId>,
    dismiss_timer: Option<DismissTimer>,
    is_hovered: bool,
    is_expanded: bool,
    message_label: Option<Label>,
    expanded_timer_label: Option<Label>,
    progress_bar: Option<ProgressBar>,
    close_button: Option<Button>,
}

impl NotificationInstance {
    fn content_height(payload: &anlg_notification_interface::Notification, expanded: bool) -> i32 {
        if expanded {
            EXPANDED_HEIGHT
        } else if payload.footer.is_some() {
            COMPACT_HEIGHT + COMPACT_FOOTER_HEIGHT
        } else {
            COMPACT_HEIGHT
        }
    }

    fn cancel_sources(&mut self) {
        if let Some(source) = self.timeout_source.take() {
            source.remove();
        }
    }

    fn schedule_remaining(&self) -> Option<Duration> {
        let start_time = self.payload.start_time?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        Some(Duration::from_secs(
            start_time.saturating_sub(now).max(0) as u64
        ))
    }

    fn compact_remaining(&self, now: Instant) -> Option<Duration> {
        if self.payload.start_time.is_some() {
            self.schedule_remaining()
        } else if self.payload.shows_stop_countdown() {
            self.dismiss_timer
                .as_ref()
                .map(|timer| timer.remaining(now))
        } else {
            None
        }
    }

    fn mount(&mut self) {
        for child in self.window.children() {
            self.window.remove(&child);
        }

        self.message_label = None;
        self.expanded_timer_label = None;
        self.progress_bar = None;
        self.close_button = None;

        let height = Self::content_height(&self.payload, self.is_expanded);
        self.window.resize(NOTIFICATION_WIDTH, height);

        let overlay = Overlay::new();
        overlay.add(&self.build_root());

        let close_button = Button::with_label("×");
        close_button.style_context().add_class("close-button");
        close_button.set_valign(Align::Start);
        close_button.set_halign(Align::Start);
        close_button.set_margin_start(6);
        close_button.set_margin_top(6);
        let close_key = self.key.clone();
        close_button.connect_clicked(move |_| {
            NotificationManager::dismiss_global(&close_key, DismissReason::User);
        });
        overlay.add_overlay(&close_button);
        self.close_button = Some(close_button);

        let hover_box = EventBox::new();
        hover_box.set_visible_window(false);
        hover_box.add(&overlay);
        hover_box.add_events(gdk::EventMask::ENTER_NOTIFY_MASK | gdk::EventMask::LEAVE_NOTIFY_MASK);
        let hover_key = self.key.clone();
        hover_box.connect_enter_notify_event(move |_, event| {
            if event.detail() != NotifyType::Inferior {
                NotificationManager::set_hovered(&hover_key, true);
            }
            glib::Propagation::Proceed
        });
        let hover_key = self.key.clone();
        hover_box.connect_leave_notify_event(move |_, event| {
            if event.detail() != NotifyType::Inferior {
                NotificationManager::set_hovered(&hover_key, false);
            }
            glib::Propagation::Proceed
        });

        self.window.add(&hover_box);
        self.window.show_all();
        if let Some(close_button) = &self.close_button {
            close_button.set_visible(self.is_hovered);
        }
    }

    fn build_root(&mut self) -> GtkBox {
        let root = GtkBox::new(Orientation::Vertical, 0);
        if self.is_expanded {
            root.pack_start(&self.build_expanded(), true, true, 0);
        } else {
            root.pack_start(&self.build_compact(), true, true, 0);
            if self
                .payload
                .timeout
                .is_some_and(|timeout| !timeout.is_zero())
            {
                let progress = ProgressBar::new();
                progress.set_fraction(
                    self.dismiss_timer
                        .as_ref()
                        .map(|timer| timer.progress_ratio(Instant::now()))
                        .unwrap_or(1.0),
                );
                progress.set_show_text(false);
                root.pack_start(&progress, false, false, 0);
                self.progress_bar = Some(progress);
            }
        }
        root
    }

    fn build_compact(&mut self) -> GtkBox {
        let payload = &self.payload;
        let main_box = GtkBox::new(Orientation::Horizontal, 8);
        main_box.set_margin_start(12);
        main_box.set_margin_end(12);
        main_box.set_margin_top(9);
        main_box.set_margin_bottom(9);
        main_box.set_valign(Align::Center);

        let content_box = GtkBox::new(Orientation::Horizontal, 8);
        if let Some(pixbuf) = icon::pixbuf_for_icon(payload.icon.as_ref()) {
            let image = Image::from_pixbuf(Some(&pixbuf));
            image.set_pixel_size(28);
            content_box.pack_start(&image, false, false, 0);
        }

        let text_box = GtkBox::new(Orientation::Vertical, 2);
        text_box.set_hexpand(true);

        let title_label = Label::new(Some(payload.compact_title()));
        title_label.set_halign(Align::Start);
        title_label.set_ellipsize(pango::EllipsizeMode::End);
        title_label.style_context().add_class("notification-title");
        text_box.pack_start(&title_label, false, false, 0);

        let body = payload.compact_message(self.compact_remaining(Instant::now()));
        if !body.trim().is_empty() {
            let message_label = Label::new(Some(&body));
            message_label.set_halign(Align::Start);
            message_label.set_ellipsize(pango::EllipsizeMode::End);
            message_label
                .style_context()
                .add_class("notification-message");
            text_box.pack_start(&message_label, false, false, 0);
            self.message_label = Some(message_label);
        }

        content_box.pack_start(&text_box, true, true, 0);

        let content_event_box = EventBox::new();
        content_event_box.set_visible_window(false);
        content_event_box.add(&content_box);
        main_box.pack_start(&content_event_box, true, true, 0);

        let options_button = match payload.primary_action() {
            PrimaryAction::Options(options) => {
                let menu = Menu::new();
                for (index, option) in options.iter().enumerate() {
                    let menu_item = MenuItem::with_label(option);
                    let option_key = self.key.clone();
                    menu_item.connect_activate(move |_| {
                        callbacks::option_selected(option_key.clone(), index as i32);
                        NotificationManager::dismiss_global(&option_key, DismissReason::Action);
                    });
                    menu.append(&menu_item);
                }

                let create_new_item = MenuItem::with_label("Create New Note...");
                let create_new_index = options.len() as i32;
                let option_key = self.key.clone();
                create_new_item.connect_activate(move |_| {
                    callbacks::option_selected(option_key.clone(), create_new_index);
                    NotificationManager::dismiss_global(&option_key, DismissReason::Action);
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
            PrimaryAction::Accept { label, destructive } => {
                let action_button = Button::with_label(label);
                action_button.style_context().add_class("action-button");
                if destructive {
                    action_button
                        .style_context()
                        .add_class("destructive-action-button");
                }
                let action_key = self.key.clone();
                action_button.connect_clicked(move |_| {
                    callbacks::accept(action_key.clone());
                    NotificationManager::dismiss_global(&action_key, DismissReason::Action);
                });
                main_box.pack_start(&action_button, false, false, 0);
                None
            }
        };

        let content_key = self.key.clone();
        let expandable = payload.has_expandable_content();
        content_event_box.connect_button_press_event(move |_, _| {
            if let Some(options_button) = &options_button {
                options_button.clicked();
            } else if expandable {
                NotificationManager::toggle_expanded_global(&content_key);
            } else {
                callbacks::confirm(content_key.clone());
                NotificationManager::dismiss_global(&content_key, DismissReason::Action);
            }
            glib::Propagation::Stop
        });

        let root = GtkBox::new(Orientation::Vertical, 0);
        root.pack_start(&main_box, true, true, 0);

        if let Some(footer) = &payload.footer {
            let footer_box = GtkBox::new(Orientation::Horizontal, 8);
            footer_box.set_margin_start(12);
            footer_box.set_margin_end(12);
            footer_box.set_margin_bottom(5);
            footer_box.style_context().add_class("notification-footer");

            let footer_label_box = GtkBox::new(Orientation::Horizontal, 4);
            if let Some(pixbuf) = icon::pixbuf_for_icon(footer.icon.as_ref()) {
                let image = Image::from_pixbuf(Some(&pixbuf));
                image.set_pixel_size(14);
                footer_label_box.pack_start(&image, false, false, 0);
            }
            let footer_label = Label::new(Some(&footer.text));
            footer_label.set_halign(Align::Start);
            footer_label.set_ellipsize(pango::EllipsizeMode::End);
            footer_label
                .style_context()
                .add_class("notification-footer-label");
            footer_label_box.pack_start(&footer_label, true, true, 0);
            footer_box.pack_start(&footer_label_box, true, true, 0);

            let footer_button = Button::with_label(&footer.action_label);
            footer_button
                .style_context()
                .add_class("footer-action-button");
            let footer_key = self.key.clone();
            footer_button.connect_clicked(move |_| {
                callbacks::footer_action(footer_key.clone());
                NotificationManager::dismiss_global(&footer_key, DismissReason::Action);
            });
            footer_box.pack_start(&footer_button, false, false, 0);
            root.pack_start(&footer_box, false, false, 0);
        }

        root
    }

    fn build_expanded(&mut self) -> GtkBox {
        let payload = &self.payload;
        let container = GtkBox::new(Orientation::Vertical, 12);
        container.set_margin_start(16);
        container.set_margin_end(16);
        container.set_margin_top(14);
        container.set_margin_bottom(14);

        let header = GtkBox::new(Orientation::Horizontal, 8);
        let title = Label::new(Some(payload.expanded_title()));
        title.set_halign(Align::Start);
        title.set_hexpand(true);
        title.set_ellipsize(pango::EllipsizeMode::End);
        title.style_context().add_class("notification-title");
        header.pack_start(&title, true, true, 0);

        let collapse = Button::with_label("Show less");
        let collapse_key = self.key.clone();
        collapse.connect_clicked(move |_| {
            NotificationManager::toggle_expanded_global(&collapse_key);
        });
        header.pack_start(&collapse, false, false, 0);
        container.pack_start(&header, false, false, 0);

        if let Some(participants) = payload
            .participants
            .as_ref()
            .filter(|participants| !participants.is_empty())
        {
            let list = GtkBox::new(Orientation::Vertical, 4);
            for participant in participants {
                let row = GtkBox::new(Orientation::Horizontal, 6);
                let name = participant.name.clone().unwrap_or_default();
                let display = if name.is_empty() {
                    participant.email.clone()
                } else {
                    format!("{name} ({})", participant.email)
                };
                let label = Label::new(Some(&display));
                label.set_halign(Align::Start);
                label.set_hexpand(true);
                row.pack_start(&label, true, true, 0);

                let (icon, class) = match participant.status {
                    ParticipantStatus::Accepted => ("✓", "status-accepted"),
                    ParticipantStatus::Maybe => ("?", "status-maybe"),
                    ParticipantStatus::Declined => ("✗", "status-declined"),
                };
                let status = Label::new(Some(icon));
                status.style_context().add_class(class);
                row.pack_start(&status, false, false, 0);
                list.pack_start(&row, false, false, 0);
            }
            container.pack_start(&list, false, false, 0);
        }

        if let Some(details) = &payload.event_details {
            container.pack_start(&detail_row("What:", &details.what), false, false, 0);
            if let Some(timezone) = &details.timezone {
                container.pack_start(&detail_row("Invitee Time Zone:", timezone), false, false, 0);
            }
            if let Some(location) = &details.location {
                container.pack_start(&detail_row("Where:", location), false, false, 0);
            }
        }

        let action_button = Button::with_label(payload.expanded_action_label());
        action_button.style_context().add_class("action-button");
        if payload.is_destructive_action() {
            action_button
                .style_context()
                .add_class("destructive-action-button");
        }
        let action_key = self.key.clone();
        action_button.connect_clicked(move |_| {
            callbacks::accept(action_key.clone());
            NotificationManager::dismiss_global(&action_key, DismissReason::Action);
        });
        container.pack_start(&action_button, false, false, 0);

        if payload.start_time.is_some() {
            let remaining = self.schedule_remaining().unwrap_or_default();
            let timer = Label::new(Some(&expanded_schedule_text(remaining)));
            timer.style_context().add_class("notification-message");
            container.pack_start(&timer, false, false, 0);
            self.expanded_timer_label = Some(timer);
        }

        container
    }
}

struct NotificationManager {
    active_notifications: IndexMap<String, NotificationInstance>,
}

impl NotificationManager {
    fn new() -> Self {
        Self {
            active_notifications: IndexMap::new(),
        }
    }

    fn ensure_gtk(&self) -> bool {
        match gtk::init() {
            Ok(_) => true,
            Err(error) => {
                tracing::error!(%error, "failed_to_initialize_gtk_notifications");
                false
            }
        }
    }

    fn show(&mut self, notification: anlg_notification_interface::Notification) {
        if !self.ensure_gtk() {
            return;
        }

        Self::install_styles();

        let key = notification
            .key
            .clone()
            .unwrap_or_else(|| notification.title.clone());

        if let Some(existing) = self.active_notifications.shift_remove(&key) {
            Self::close_window(&existing.window);
        }

        while self.active_notifications.len() >= MAX_NOTIFICATIONS {
            if let Some((oldest_id, _)) = self.active_notifications.first() {
                let oldest_id = oldest_id.clone();
                self.dismiss_key(&oldest_id, DismissReason::Superseded);
            } else {
                break;
            }
        }

        let window = Window::new(WindowType::Toplevel);
        window.set_decorated(false);
        window.set_resizable(false);
        window.set_accept_focus(false);
        window.set_skip_taskbar_hint(true);
        window.set_skip_pager_hint(true);
        window.set_keep_above(true);
        window.set_type_hint(gdk::WindowTypeHint::Notification);
        window.stick();
        window.set_default_size(
            NOTIFICATION_WIDTH,
            NotificationInstance::content_height(&notification, false),
        );
        if prefers_dark_theme() {
            window.style_context().add_class("dark");
        }

        let timeout = notification.timeout.filter(|timeout| !timeout.is_zero());
        let mut instance = NotificationInstance {
            key: key.clone(),
            payload: notification,
            window: window.clone(),
            timeout_source: None,
            dismiss_timer: timeout.map(DismissTimer::new),
            is_hovered: false,
            is_expanded: false,
            message_label: None,
            expanded_timer_label: None,
            progress_bar: None,
            close_button: None,
        };
        self.position_window(
            &instance.window,
            NotificationInstance::content_height(&instance.payload, false),
        );
        instance.mount();
        if instance.dismiss_timer.is_some() || instance.payload.start_time.is_some() {
            self.arm_timer(&mut instance);
        }

        self.active_notifications.insert(key, instance);
        self.reposition_notifications();
    }

    fn install_styles() {
        static INSTALLED: std::sync::Once = std::sync::Once::new();
        INSTALLED.call_once(|| {
            let css_provider = CssProvider::new();
            let _ = css_provider.load_from_data(NOTIFICATION_CSS.as_bytes());
            if let Some(screen) = gdk::Screen::default() {
                StyleContext::add_provider_for_screen(
                    &screen,
                    &css_provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }
        });
    }

    fn arm_timer(&mut self, instance: &mut NotificationInstance) {
        instance.cancel_sources();
        let key = instance.key.clone();
        instance.timeout_source = Some(glib::timeout_add_local(TICK_INTERVAL, move || {
            NotificationManager::tick_global(&key);
            glib::ControlFlow::Continue
        }));
    }

    fn tick(&mut self, key: &str) {
        let now = Instant::now();
        let expired = self
            .active_notifications
            .get(key)
            .and_then(|instance| instance.dismiss_timer.as_ref())
            .is_some_and(|timer| timer.is_running() && timer.is_expired(now));
        if expired {
            self.dismiss_key(key, DismissReason::Timeout);
            return;
        }

        let Some(instance) = self.active_notifications.get_mut(key) else {
            return;
        };
        if let Some(progress) = &instance.progress_bar
            && let Some(timer) = &instance.dismiss_timer
        {
            progress.set_fraction(timer.progress_ratio(now));
        }
        if let Some(label) = &instance.message_label {
            label.set_text(
                &instance
                    .payload
                    .compact_message(instance.compact_remaining(now)),
            );
        }
        if let Some(label) = &instance.expanded_timer_label {
            label.set_text(&expanded_schedule_text(
                instance.schedule_remaining().unwrap_or_default(),
            ));
        }
    }

    fn set_hover(&mut self, key: &str, hovered: bool) {
        let Some(instance) = self.active_notifications.get_mut(key) else {
            return;
        };
        if instance.is_hovered == hovered {
            return;
        }
        instance.is_hovered = hovered;
        if let Some(close_button) = &instance.close_button {
            close_button.set_visible(hovered);
        }
        if instance.is_expanded {
            return;
        }
        if let Some(timer) = &mut instance.dismiss_timer {
            if hovered {
                timer.pause(Instant::now());
            } else {
                timer.resume(Instant::now());
            }
        }
    }

    fn toggle_expanded(&mut self, key: &str) {
        {
            let Some(instance) = self.active_notifications.get_mut(key) else {
                return;
            };
            if !instance.payload.has_expandable_content() && !instance.is_expanded {
                return;
            }
            instance.is_expanded = !instance.is_expanded;
            if let Some(timer) = &mut instance.dismiss_timer {
                if instance.is_expanded {
                    timer.pause(Instant::now());
                } else if !instance.is_hovered {
                    timer.resume(Instant::now());
                }
            }
            instance.mount();
        }
        self.reposition_notifications();
    }

    fn position_window(&self, window: &Window, height: i32) {
        if let Some((x, y)) = overlay_origin(self.occupied_height()) {
            window.move_(x, y);
        }
        let _ = height;
    }

    fn occupied_height(&self) -> i32 {
        self.active_notifications
            .values()
            .map(|instance| {
                NotificationInstance::content_height(&instance.payload, instance.is_expanded)
                    + NOTIFICATION_SPACING
            })
            .sum()
    }

    fn reposition_notifications(&mut self) {
        let mut occupied = 0;
        let positions: Vec<(String, i32, i32)> = self
            .active_notifications
            .iter()
            .filter_map(|(key, instance)| {
                let height =
                    NotificationInstance::content_height(&instance.payload, instance.is_expanded);
                let (x, y) = overlay_origin(occupied)?;
                occupied += height + NOTIFICATION_SPACING;
                Some((key.clone(), x, y))
            })
            .collect();

        for (key, x, y) in positions {
            if let Some(instance) = self.active_notifications.get(&key) {
                instance.window.move_(x, y);
            }
        }
    }

    fn dismiss_key(&mut self, key: &str, reason: DismissReason) {
        let Some(mut instance) = self.active_notifications.shift_remove(key) else {
            return;
        };
        instance.cancel_sources();
        match reason {
            DismissReason::User => callbacks::dismiss(key.to_string()),
            DismissReason::Timeout => callbacks::timeout(key.to_string()),
            DismissReason::Action | DismissReason::Superseded => {}
        }
        Self::close_window(&instance.window);
        self.reposition_notifications();
    }

    fn dismiss_all(&mut self) {
        let keys: Vec<String> = self.active_notifications.keys().cloned().collect();
        for key in keys {
            self.dismiss_key(&key, DismissReason::Superseded);
        }
    }

    fn close_window(window: &Window) {
        window.set_sensitive(false);
        let window = window.clone();
        glib::timeout_add_local_once(Duration::from_millis(200), move || {
            window.close();
        });
    }

    fn dismiss_global(key: &str, reason: DismissReason) {
        NOTIFICATION_MANAGER.with(|manager| {
            manager.borrow_mut().dismiss_key(key, reason);
        });
    }

    fn tick_global(key: &str) {
        NOTIFICATION_MANAGER.with(|manager| {
            manager.borrow_mut().tick(key);
        });
    }

    fn set_hovered(key: &str, hovered: bool) {
        NOTIFICATION_MANAGER.with(|manager| {
            manager.borrow_mut().set_hover(key, hovered);
        });
    }

    fn toggle_expanded_global(key: &str) {
        NOTIFICATION_MANAGER.with(|manager| {
            manager.borrow_mut().toggle_expanded(key);
        });
    }
}

#[derive(Clone, Copy)]
enum DismissReason {
    User,
    Timeout,
    Action,
    Superseded,
}

fn overlay_origin(occupied: i32) -> Option<(i32, i32)> {
    let monitor = gdk::Display::default()?.primary_monitor()?;
    let geometry = monitor.workarea();
    Some((
        geometry.x() + geometry.width() - NOTIFICATION_WIDTH - RIGHT_MARGIN,
        geometry.y() + TOP_MARGIN + occupied,
    ))
}

fn prefers_dark_theme() -> bool {
    gtk::Settings::default()
        .map(|settings| settings.is_gtk_application_prefer_dark_theme())
        .unwrap_or(false)
}

fn detail_row(label: &str, value: &str) -> GtkBox {
    let row = GtkBox::new(Orientation::Vertical, 2);
    let name = Label::new(Some(label));
    name.set_halign(Align::Start);
    name.style_context().add_class("notification-footer-label");
    let content = Label::new(Some(value));
    content.set_halign(Align::Start);
    content.set_line_wrap(true);
    row.pack_start(&name, false, false, 0);
    row.pack_start(&content, false, false, 0);
    row
}

const NOTIFICATION_CSS: &str = r#"
window {
    background-color: rgba(255, 255, 255, 0.95);
    border-radius: 14px;
    border: 1px solid rgba(0, 0, 0, 0.1);
}
window.dark {
    background-color: rgba(40, 40, 42, 0.94);
    border: 1px solid rgba(255, 255, 255, 0.12);
}
.notification-title {
    font-size: 14px;
    font-weight: 600;
    color: #1a1a1a;
}
window.dark .notification-title {
    color: #f2f2f2;
}
.notification-message {
    font-size: 11px;
    color: #666666;
}
window.dark .notification-message {
    color: #b3b3b3;
}
.close-button {
    min-width: 22px;
    min-height: 22px;
    border-radius: 11px;
    background-color: rgba(255, 255, 255, 0.72);
    border: 0.5px solid rgba(0, 0, 0, 0.18);
    color: rgba(0, 0, 0, 0.62);
    padding: 0;
    font-weight: 600;
}
.close-button:hover {
    background-color: rgba(255, 255, 255, 0.9);
}
.action-button {
    border-radius: 10px;
    background-color: rgba(242, 242, 242, 0.9);
    border: 0.5px solid rgba(179, 179, 179, 0.5);
    color: rgba(26, 26, 26, 1.0);
    font-size: 12px;
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
.footer-action-button {
    padding: 0 4px;
    min-height: 20px;
    font-size: 10px;
    font-weight: 600;
}
.status-accepted { color: #248a3d; }
.status-maybe { color: #c93400; }
.status-declined { color: #c42b1c; }
progressbar trough {
    min-height: 3px;
    background-color: transparent;
}
progressbar progress {
    min-height: 3px;
    background-color: rgba(102, 153, 230, 0.7);
}
"#;

pub fn show(notification: &anlg_notification_interface::Notification) {
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
