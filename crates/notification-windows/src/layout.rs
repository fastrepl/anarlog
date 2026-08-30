#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

use anlg_notification_interface::{Notification, PrimaryAction};

pub const NOTIFICATION_WIDTH: i32 = 360;
pub const COMPACT_HEIGHT: i32 = 64;
pub const COMPACT_FOOTER_HEIGHT: i32 = 28;
pub const EXPANDED_HEIGHT: i32 = 380;
pub const RIGHT_MARGIN: i32 = 24;
pub const TOP_MARGIN: i32 = 15;
pub const NOTIFICATION_SPACING: i32 = 10;
pub const MAX_NOTIFICATIONS: usize = 5;
pub const CLOSE_BUTTON_SIZE: i32 = 22;
pub const ICON_SIZE: i32 = 28;
pub const ACTION_BUTTON_WIDTH: i32 = 120;
pub const ACTION_BUTTON_HEIGHT: i32 = 28;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }

    pub fn right(self) -> i32 {
        self.x + self.width
    }

    pub fn bottom(self) -> i32 {
        self.y + self.height
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitTarget {
    Close,
    Action,
    Options,
    Footer,
    Collapse,
    Body,
}

#[derive(Debug, Clone)]
pub struct NotificationLayout {
    pub width: i32,
    pub height: i32,
    pub close: Rect,
    pub icon: Option<Rect>,
    pub title: Rect,
    pub message: Rect,
    pub action: Option<Rect>,
    pub footer_action: Option<Rect>,
    pub collapse: Option<Rect>,
    pub progress: Option<Rect>,
}

impl NotificationLayout {
    pub fn compact(notification: &Notification) -> Self {
        let has_footer = notification.footer.is_some();
        let height = if has_footer {
            COMPACT_HEIGHT + COMPACT_FOOTER_HEIGHT
        } else {
            COMPACT_HEIGHT
        };
        let has_icon = !matches!(
            notification.icon,
            Some(anlg_notification_interface::NotificationIcon::Hidden)
        );
        let icon = has_icon.then_some(Rect {
            x: 12,
            y: (COMPACT_HEIGHT - ICON_SIZE) / 2,
            width: ICON_SIZE,
            height: ICON_SIZE,
        });
        let text_x = if has_icon { 48 } else { 12 };
        let action = match notification.primary_action() {
            PrimaryAction::Accept { .. } | PrimaryAction::Options(_) => Some(Rect {
                x: NOTIFICATION_WIDTH - ACTION_BUTTON_WIDTH - 12,
                y: (COMPACT_HEIGHT - ACTION_BUTTON_HEIGHT) / 2,
                width: ACTION_BUTTON_WIDTH,
                height: ACTION_BUTTON_HEIGHT,
            }),
        };
        let text_right = action
            .map(|rect| rect.x - 8)
            .unwrap_or(NOTIFICATION_WIDTH - 12);
        let text_width = (text_right - text_x).max(48);

        Self {
            width: NOTIFICATION_WIDTH,
            height,
            close: Rect {
                x: 6,
                y: 6,
                width: CLOSE_BUTTON_SIZE,
                height: CLOSE_BUTTON_SIZE,
            },
            icon,
            title: Rect {
                x: text_x,
                y: 12,
                width: text_width,
                height: 18,
            },
            message: Rect {
                x: text_x,
                y: 32,
                width: text_width,
                height: 16,
            },
            action,
            footer_action: has_footer.then_some(Rect {
                x: NOTIFICATION_WIDTH - 72,
                y: COMPACT_HEIGHT + 4,
                width: 60,
                height: 20,
            }),
            collapse: None,
            progress: notification
                .timeout
                .filter(|timeout| !timeout.is_zero())
                .map(|_| Rect {
                    x: 12,
                    y: height - 5,
                    width: NOTIFICATION_WIDTH - 24,
                    height: 3,
                }),
        }
    }

    pub fn expanded(_notification: &Notification) -> Self {
        Self {
            width: NOTIFICATION_WIDTH,
            height: EXPANDED_HEIGHT,
            close: Rect {
                x: 6,
                y: 6,
                width: CLOSE_BUTTON_SIZE,
                height: CLOSE_BUTTON_SIZE,
            },
            icon: None,
            title: Rect {
                x: 16,
                y: 14,
                width: NOTIFICATION_WIDTH - 120,
                height: 22,
            },
            message: Rect {
                x: 16,
                y: 48,
                width: NOTIFICATION_WIDTH - 32,
                height: 240,
            },
            action: Some(Rect {
                x: 16,
                y: EXPANDED_HEIGHT - 72,
                width: NOTIFICATION_WIDTH - 32,
                height: 36,
            }),
            footer_action: None,
            collapse: Some(Rect {
                x: NOTIFICATION_WIDTH - 96,
                y: 14,
                width: 80,
                height: 24,
            }),
            progress: None,
        }
    }

    pub fn for_state(notification: &Notification, expanded: bool) -> Self {
        if expanded {
            Self::expanded(notification)
        } else {
            Self::compact(notification)
        }
    }

    pub fn hit_test(&self, x: i32, y: i32) -> HitTarget {
        if self.close.contains(x, y) {
            return HitTarget::Close;
        }
        if self.collapse.is_some_and(|rect| rect.contains(x, y)) {
            return HitTarget::Collapse;
        }
        if self.action.is_some_and(|rect| rect.contains(x, y)) {
            return HitTarget::Action;
        }
        if self.footer_action.is_some_and(|rect| rect.contains(x, y)) {
            return HitTarget::Footer;
        }
        HitTarget::Body
    }

    pub fn action_hit(&self, notification: &Notification, x: i32, y: i32) -> HitTarget {
        match self.hit_test(x, y) {
            HitTarget::Action if notification.has_options() && self.collapse.is_none() => {
                HitTarget::Options
            }
            target => target,
        }
    }
}

pub fn stacked_origin(work_x: i32, work_y: i32, work_width: i32, occupied: i32) -> (i32, i32) {
    (
        work_x + work_width - NOTIFICATION_WIDTH - RIGHT_MARGIN,
        work_y + TOP_MARGIN + occupied,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_notification_interface::{EventDetails, NotificationSource};
    use std::time::Duration;

    fn compact_notification() -> Notification {
        Notification::builder()
            .title("Upcoming event")
            .message("Starting soon")
            .timeout(Duration::from_secs(8))
            .build()
    }

    #[test]
    fn compact_layout_keeps_actions_and_progress_in_bounds() {
        let notification = compact_notification();
        let layout = NotificationLayout::compact(&notification);
        assert_eq!(layout.height, COMPACT_HEIGHT);
        assert!(layout.close.contains(10, 10));
        assert_eq!(layout.hit_test(10, 10), HitTarget::Close);
        assert_eq!(
            layout.action_hit(
                &notification,
                layout.action.unwrap().x + 4,
                layout.action.unwrap().y + 4
            ),
            HitTarget::Action
        );
        assert!(layout.progress.is_some());
    }

    #[test]
    fn options_map_the_primary_button_to_the_options_hit_target() {
        let notification = Notification::builder()
            .title("Choose a meeting")
            .message("")
            .options(vec!["Design sync".to_string()])
            .build();
        let layout = NotificationLayout::compact(&notification);
        let action = layout.action.unwrap();
        assert_eq!(
            layout.action_hit(&notification, action.x + 2, action.y + 2),
            HitTarget::Options
        );
    }

    #[test]
    fn expanded_layout_exposes_collapse_and_accept_targets() {
        let notification = Notification::builder()
            .title("Design sync")
            .message("")
            .source(NotificationSource::Session {
                session_id: "sess".to_string(),
            })
            .event_details(EventDetails {
                what: "Design sync".to_string(),
                timezone: None,
                location: None,
            })
            .build();
        let layout = NotificationLayout::expanded(&notification);
        assert_eq!(layout.height, EXPANDED_HEIGHT);
        assert_eq!(
            layout.hit_test(layout.collapse.unwrap().x + 1, 16),
            HitTarget::Collapse
        );
        assert_eq!(
            layout.hit_test(layout.action.unwrap().x + 8, layout.action.unwrap().y + 8),
            HitTarget::Action
        );
    }

    #[test]
    fn stacks_notifications_from_the_work_area_top_right() {
        assert_eq!(stacked_origin(0, 0, 1920, 0), (1536, 15));
        assert_eq!(stacked_origin(0, 0, 1920, 74), (1536, 89));
    }
}
