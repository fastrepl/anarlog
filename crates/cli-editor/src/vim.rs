use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{Editor, KeyResult, StyleSheet};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum VimMode {
    Normal,
    Insert,
}

pub(crate) struct VimState {
    pub mode: VimMode,
    pending: Option<char>,
}

impl VimState {
    pub fn new() -> Self {
        Self {
            mode: VimMode::Normal,
            pending: None,
        }
    }
}

impl<S: StyleSheet> Editor<S> {
    pub(crate) fn handle_key_vim(&mut self, key: KeyEvent) -> KeyResult {
        match self.vim_state.mode {
            VimMode::Normal => self.handle_vim_normal(key),
            VimMode::Insert if self.readonly => {
                // Readonly mode should never be in insert mode, force back to normal
                self.vim_state.mode = VimMode::Normal;
                self.handle_vim_normal(key)
            }
            VimMode::Insert => {
                if key.code == KeyCode::Esc {
                    self.vim_state.mode = VimMode::Normal;
                    if self.cursor.col > 0 {
                        self.cursor.col -= 1;
                    }
                    return KeyResult::Consumed;
                }
                self.handle_key_insert(key)
            }
        }
    }

    fn handle_vim_normal(&mut self, key: KeyEvent) -> KeyResult {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return match key.code {
                KeyCode::Char('r') if !self.readonly => {
                    self.redo();
                    KeyResult::Consumed
                }
                KeyCode::Char('l') if self.readonly => {
                    if let Some(url) = self.link_at_cursor() {
                        KeyResult::FollowLink(url)
                    } else {
                        KeyResult::Consumed
                    }
                }
                _ => KeyResult::Ignored,
            };
        }

        if let Some(pending) = self.vim_state.pending.take() {
            return match (pending, key.code) {
                ('d', KeyCode::Char('d')) if !self.readonly => {
                    self.save_for_undo();
                    self.buffer.delete_line(self.cursor.row);
                    self.cursor.clamp(self.buffer.lines());
                    self.ensure_visible();
                    KeyResult::Consumed
                }
                ('g', KeyCode::Char('g')) => {
                    self.cursor.row = 0;
                    self.cursor.col = 0;
                    self.ensure_visible();
                    KeyResult::Consumed
                }
                _ => KeyResult::Consumed,
            };
        }

        match key.code {
            // Movement
            KeyCode::Char('h') | KeyCode::Left => {
                if self.cursor.col > 0 {
                    self.cursor.col -= 1;
                }
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.cursor.move_down(self.buffer.lines());
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.cursor.move_up(self.buffer.lines());
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Char('l') | KeyCode::Right => {
                let max = self.buffer.line_char_count(self.cursor.row);
                if max > 0 && self.cursor.col < max - 1 {
                    self.cursor.col += 1;
                }
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Char('w') => {
                self.cursor.move_word_forward(self.buffer.lines());
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Char('b') => {
                self.cursor.move_word_back(self.buffer.lines());
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Char('0') => {
                self.cursor.move_home();
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Char('^') => {
                self.cursor.move_to_first_non_blank(self.buffer.lines());
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Char('$') => {
                let max = self.buffer.line_char_count(self.cursor.row);
                self.cursor.col = max.saturating_sub(1);
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Char('g') => {
                self.vim_state.pending = Some('g');
                KeyResult::Consumed
            }
            KeyCode::Char('G') => {
                self.cursor.row = self.buffer.line_count().saturating_sub(1);
                self.cursor.clamp(self.buffer.lines());
                self.ensure_visible();
                KeyResult::Consumed
            }

            // Follow link
            KeyCode::Enter if self.readonly => {
                if let Some(url) = self.link_at_cursor() {
                    KeyResult::FollowLink(url)
                } else {
                    KeyResult::Consumed
                }
            }

            // Editing (blocked in readonly)
            KeyCode::Char('x') if !self.readonly => {
                self.save_for_undo();
                self.buffer.delete_char_at(self.cursor.row, self.cursor.col);
                self.cursor.clamp(self.buffer.lines());
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Char('d') if !self.readonly => {
                self.vim_state.pending = Some('d');
                KeyResult::Consumed
            }
            KeyCode::Char('D') if !self.readonly => {
                self.save_for_undo();
                self.buffer
                    .delete_to_end_of_line(self.cursor.row, self.cursor.col);
                self.cursor.clamp(self.buffer.lines());
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Char('C') if !self.readonly => {
                self.save_for_undo();
                self.buffer
                    .delete_to_end_of_line(self.cursor.row, self.cursor.col);
                self.vim_state.mode = VimMode::Insert;
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Char('u') if !self.readonly => {
                self.undo();
                KeyResult::Consumed
            }

            // Enter insert mode (blocked in readonly)
            KeyCode::Char('i') if !self.readonly => {
                self.vim_state.mode = VimMode::Insert;
                KeyResult::Consumed
            }
            KeyCode::Char('I') if !self.readonly => {
                self.vim_state.mode = VimMode::Insert;
                self.cursor.move_to_first_non_blank(self.buffer.lines());
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Char('a') if !self.readonly => {
                self.vim_state.mode = VimMode::Insert;
                self.cursor.move_right(self.buffer.lines());
                KeyResult::Consumed
            }
            KeyCode::Char('A') if !self.readonly => {
                self.vim_state.mode = VimMode::Insert;
                self.cursor.move_end(self.buffer.lines());
                KeyResult::Consumed
            }
            KeyCode::Char('o') if !self.readonly => {
                self.save_for_undo();
                self.vim_state.mode = VimMode::Insert;
                self.buffer.insert_empty_line_after(self.cursor.row);
                self.cursor.row += 1;
                self.cursor.col = 0;
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Char('O') if !self.readonly => {
                self.save_for_undo();
                self.vim_state.mode = VimMode::Insert;
                self.buffer.insert_empty_line_before(self.cursor.row);
                self.cursor.col = 0;
                self.ensure_visible();
                KeyResult::Consumed
            }
            _ => KeyResult::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::{Editor, KeyResult};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn vim_editor(text: &str) -> Editor {
        let lines: Vec<String> = text.lines().map(String::from).collect();
        let mut ed = Editor::from_lines(lines);
        ed.set_vim_mode(true);
        // Move cursor to top-left via gg
        ed.handle_key(key(KeyCode::Char('g')));
        ed.handle_key(key(KeyCode::Char('g')));
        ed
    }

    #[test]
    fn dd_deletes_line() {
        let mut ed = vim_editor("aaa\nbbb\nccc");
        ed.handle_key(key(KeyCode::Char('j'))); // move to line 1
        ed.handle_key(key(KeyCode::Char('d')));
        ed.handle_key(key(KeyCode::Char('d')));
        assert_eq!(ed.lines(), &["aaa", "ccc"]);
    }

    #[test]
    fn gg_goes_to_top() {
        let mut ed = vim_editor("aaa\nbbb\nccc");
        ed.handle_key(key(KeyCode::Char('G'))); // go to last line
        assert_eq!(ed.cursor().0, 2);
        ed.handle_key(key(KeyCode::Char('g')));
        ed.handle_key(key(KeyCode::Char('g')));
        assert_eq!(ed.cursor(), (0, 0));
    }

    #[test]
    fn mode_transitions() {
        let mut ed = vim_editor("hello");
        assert_eq!(ed.vim_mode_label(), Some("NORMAL"));
        ed.handle_key(key(KeyCode::Char('i')));
        assert_eq!(ed.vim_mode_label(), Some("INSERT"));
        ed.handle_key(key(KeyCode::Esc));
        assert_eq!(ed.vim_mode_label(), Some("NORMAL"));
    }

    #[test]
    fn readonly_blocks_edits() {
        let mut ed = vim_editor("hello");
        ed.set_readonly(true);
        let result = ed.handle_key(key(KeyCode::Char('x')));
        assert_eq!(result, KeyResult::Ignored);
        assert_eq!(ed.lines(), &["hello"]);
        let result = ed.handle_key(key(KeyCode::Char('i')));
        assert_eq!(result, KeyResult::Ignored);
        assert_eq!(ed.vim_mode_label(), Some("NORMAL"));
    }
}
