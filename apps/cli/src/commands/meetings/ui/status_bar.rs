use ratatui::{Frame, layout::Rect, text::Span};

use crate::theme::Theme;
use crate::widgets::{CommandBar, KeyHints};

use super::Mode;

pub(crate) struct StatusBarConfig<'a> {
    pub mode: Mode,
    pub command_buffer: &'a str,
    pub normal_hints: Vec<(&'a str, &'a str)>,
    pub insert_hints: Vec<(&'a str, &'a str)>,
    pub normal_suffix: Option<Span<'a>>,
    pub insert_suffix: Option<Span<'a>>,
}

pub(crate) fn draw_status_bar(
    frame: &mut Frame,
    config: StatusBarConfig<'_>,
    area: Rect,
    theme: &Theme,
) {
    match config.mode {
        Mode::Command => {
            frame.render_widget(CommandBar::new(config.command_buffer, theme), area);
        }
        Mode::Insert => {
            let mut widget = KeyHints::new(theme)
                .badge(" INSERT ", theme.mode_insert)
                .hints(config.insert_hints);
            if let Some(suffix) = config.insert_suffix {
                widget = widget.suffix(suffix);
            }
            frame.render_widget(widget, area);
        }
        Mode::Normal => {
            let mut widget = KeyHints::new(theme)
                .badge(" NORMAL ", theme.mode_normal)
                .hints(config.normal_hints);
            if let Some(suffix) = config.normal_suffix {
                widget = widget.suffix(suffix);
            }
            frame.render_widget(widget, area);
        }
    }
}
