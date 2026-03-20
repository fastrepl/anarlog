use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionStatus {
    Checking,
    NotRequested,
    Authorized,
    Denied,
}

pub struct PermissionButton<'a> {
    status: PermissionStatus,
    theme: &'a Theme,
}

impl<'a> PermissionButton<'a> {
    pub fn new(status: PermissionStatus, theme: &'a Theme) -> Self {
        Self { status, theme }
    }
}

impl Widget for PermissionButton<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 2 {
            return;
        }

        let (status_text, status_style) = match self.status {
            PermissionStatus::Checking => ("Checking...", self.theme.muted),
            PermissionStatus::NotRequested => ("Not Requested", self.theme.status_degraded),
            PermissionStatus::Authorized => ("Authorized", self.theme.status_active),
            PermissionStatus::Denied => ("Denied", self.theme.error),
        };

        let status_line = Line::from(vec![
            Span::raw("  Status: "),
            Span::styled(status_text, status_style),
        ]);
        status_line.render(area, buf);

        let hint = match self.status {
            PermissionStatus::Checking => "",
            PermissionStatus::NotRequested => "  [Enter] Request Access",
            PermissionStatus::Authorized => "  [Enter] Continue",
            PermissionStatus::Denied => "  [Enter] Reset in System Settings",
        };

        if !hint.is_empty() && area.height >= 2 {
            let hint_area = Rect {
                y: area.y + 1,
                height: 1,
                ..area
            };
            Line::from(Span::styled(hint, self.theme.muted)).render(hint_area, buf);
        }
    }
}
