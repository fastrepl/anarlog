use std::time::Instant;

use hypr_cli_editor::Editor;

use crate::theme::Theme;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use tachyonfx::{Effect, Interpolation, Motion, fx};

use crate::widgets::ScrollViewState;

pub(crate) use crate::commands::meetings::ui::Mode;

const DEFAULT_NOTEPAD_WIDTH_PERCENT: u16 = 60;

pub(super) struct LiveUiState {
    mode: Mode,
    command_buffer: String,
    notepad_width_percent: u16,
    scroll: ScrollViewState,
    transcript_autoscroll: bool,
    memo: Editor<Theme>,
    last_frame_time: Instant,
    prev_segment_count: usize,
    transcript_effects: Vec<Effect>,
}

impl LiveUiState {
    pub(super) fn new() -> Self {
        let now = Instant::now();
        Self {
            mode: Mode::Normal,
            command_buffer: String::new(),
            notepad_width_percent: DEFAULT_NOTEPAD_WIDTH_PERCENT,
            scroll: ScrollViewState::new(),
            transcript_autoscroll: true,
            memo: Self::init_memo(),
            last_frame_time: now,
            prev_segment_count: 0,
            transcript_effects: Vec::new(),
        }
    }

    pub(super) fn mode(&self) -> Mode {
        self.mode
    }

    pub(super) fn memo(&self) -> &Editor<Theme> {
        &self.memo
    }

    pub(super) fn memo_mut(&mut self) -> &mut Editor<Theme> {
        &mut self.memo
    }

    pub(super) fn reset_memo(&mut self) {
        self.memo = Self::init_memo();
    }

    pub(super) fn command_buffer(&self) -> &str {
        &self.command_buffer
    }

    pub(super) fn clear_command_buffer(&mut self) {
        self.command_buffer.clear();
    }

    pub(super) fn push_command_char(&mut self, c: char) {
        self.command_buffer.push(c);
    }

    pub(super) fn pop_command_char(&mut self) {
        self.command_buffer.pop();
    }

    pub(super) fn notepad_width_percent(&self) -> u16 {
        self.notepad_width_percent
    }

    pub(super) fn scroll_state_mut(&mut self) -> &mut ScrollViewState {
        if self.transcript_autoscroll {
            self.scroll.scroll_to_bottom();
        }
        &mut self.scroll
    }

    pub(super) fn frame_elapsed(&mut self) -> std::time::Duration {
        let now = Instant::now();
        let elapsed = now - self.last_frame_time;
        self.last_frame_time = now;
        elapsed
    }

    pub(super) fn check_new_segments(&mut self, current_count: usize, transcript_area: Rect) {
        if current_count > self.prev_segment_count && self.prev_segment_count > 0 {
            let effect = fx::sweep_in(
                Motion::LeftToRight,
                8,
                0,
                Color::Rgb(0, 60, 80),
                (350u32, Interpolation::CubicOut),
            )
            .with_area(transcript_area);
            self.transcript_effects.push(effect);
        }
        self.prev_segment_count = current_count;
    }

    pub(super) fn process_effects(
        &mut self,
        elapsed: std::time::Duration,
        buf: &mut ratatui::buffer::Buffer,
        area: Rect,
    ) {
        let elapsed: tachyonfx::Duration = elapsed.into();
        self.transcript_effects.retain_mut(|effect| {
            effect.process(elapsed, buf, area);
            !effect.done()
        });
    }

    pub(super) fn has_active_effects(&self) -> bool {
        !self.transcript_effects.is_empty()
    }

    pub(super) fn adjust_notepad_width(&mut self, delta: i16) {
        const MIN_NOTEPAD_WIDTH_PERCENT: u16 = 40;
        const MAX_NOTEPAD_WIDTH_PERCENT: u16 = 75;

        let next = (self.notepad_width_percent as i16 + delta).clamp(
            MIN_NOTEPAD_WIDTH_PERCENT as i16,
            MAX_NOTEPAD_WIDTH_PERCENT as i16,
        ) as u16;
        self.notepad_width_percent = next;
    }

    pub(super) fn enter_command_mode(&mut self) {
        self.mode = Mode::Command;
        self.command_buffer.clear();
    }

    pub(super) fn enter_insert_mode(&mut self) {
        self.mode = Mode::Insert;
    }

    pub(super) fn enter_normal_mode(&mut self) {
        self.mode = Mode::Normal;
    }

    pub(super) fn scroll_down(&mut self) {
        self.scroll.scroll_down();
    }

    pub(super) fn scroll_up(&mut self) {
        self.scroll.scroll_up();
        self.transcript_autoscroll = false;
    }

    pub(super) fn scroll_bottom(&mut self) {
        self.scroll.scroll_to_bottom();
        self.transcript_autoscroll = true;
    }

    pub(super) fn scroll_top(&mut self) {
        self.scroll.scroll_to_top();
        self.transcript_autoscroll = false;
    }

    fn init_memo() -> Editor<Theme> {
        let mut memo = Editor::with_styles(Theme::DEFAULT);
        memo.set_placeholder(
            "press [i] to start writing notes...",
            Style::new()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        );
        memo.set_cursor_line_style(Style::new().add_modifier(Modifier::UNDERLINED));
        memo
    }
}
