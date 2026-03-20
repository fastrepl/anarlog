use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::history::ActionKind;
use crate::{Editor, KeyResult, StyleSheet};

impl<S: StyleSheet> Editor<S> {
    pub fn handle_key(&mut self, key: KeyEvent) -> KeyResult {
        if self.vim_enabled {
            return self.handle_key_vim(key);
        }
        self.handle_key_insert(key)
    }

    pub(crate) fn handle_key_insert(&mut self, key: KeyEvent) -> KeyResult {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        if ctrl {
            return match key.code {
                KeyCode::Char('z') if !self.readonly => {
                    self.undo();
                    KeyResult::Consumed
                }
                KeyCode::Char('y') if !self.readonly => {
                    self.redo();
                    KeyResult::Consumed
                }
                KeyCode::Left => {
                    self.cursor.move_word_back(self.buffer.lines());
                    self.ensure_visible();
                    KeyResult::Consumed
                }
                KeyCode::Right => {
                    self.cursor.move_word_forward(self.buffer.lines());
                    self.ensure_visible();
                    KeyResult::Consumed
                }
                _ => KeyResult::Ignored,
            };
        }

        match key.code {
            KeyCode::Char(c) if !self.readonly => {
                self.save_for_undo_coalesced(ActionKind::CharInsert);
                let new_col = self.buffer.insert_char(self.cursor.row, self.cursor.col, c);
                self.cursor.col = new_col;
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Backspace if !self.readonly => {
                self.save_for_undo();
                if let Some((r, c)) = self
                    .buffer
                    .delete_char_before(self.cursor.row, self.cursor.col)
                {
                    self.cursor.row = r;
                    self.cursor.col = c;
                }
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Delete if !self.readonly => {
                self.save_for_undo();
                self.buffer.delete_char_at(self.cursor.row, self.cursor.col);
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Left => {
                self.cursor.move_left(self.buffer.lines());
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Right => {
                self.cursor.move_right(self.buffer.lines());
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Up if !self.single_line => {
                self.cursor.move_up(self.buffer.lines());
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Down if !self.single_line => {
                self.cursor.move_down(self.buffer.lines());
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Home => {
                self.cursor.move_home();
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::End => {
                self.cursor.move_end(self.buffer.lines());
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Enter if self.readonly => {
                if let Some(url) = self.link_at_cursor() {
                    KeyResult::FollowLink(url)
                } else {
                    KeyResult::Consumed
                }
            }
            KeyCode::Enter if !self.single_line => {
                self.save_for_undo();
                let (r, c) = self.buffer.insert_newline(self.cursor.row, self.cursor.col);
                self.cursor.row = r;
                self.cursor.col = c;
                self.ensure_visible();
                KeyResult::Consumed
            }
            KeyCode::Tab if !self.readonly && !self.single_line => {
                self.save_for_undo();
                let (r, c) = self
                    .buffer
                    .insert_str_at(self.cursor.row, self.cursor.col, "    ");
                self.cursor.row = r;
                self.cursor.col = c;
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

    #[test]
    fn single_line_rejects_enter() {
        let mut ed = Editor::single_line();
        ed.handle_key(key(KeyCode::Char('a')));
        let result = ed.handle_key(key(KeyCode::Enter));
        assert_eq!(result, KeyResult::Ignored);
        assert_eq!(ed.lines().len(), 1);
    }

    #[test]
    fn single_line_ignores_up_down() {
        let mut ed = Editor::single_line();
        ed.handle_key(key(KeyCode::Char('x')));
        let up = ed.handle_key(key(KeyCode::Up));
        let down = ed.handle_key(key(KeyCode::Down));
        assert_eq!(up, KeyResult::Ignored);
        assert_eq!(down, KeyResult::Ignored);
    }

    #[test]
    fn readonly_blocks_insert() {
        let mut ed = Editor::new();
        ed.set_readonly(true);
        let result = ed.handle_key(key(KeyCode::Char('a')));
        assert_eq!(result, KeyResult::Ignored);
        assert!(ed.is_empty());
    }

    #[test]
    fn undo_coalescing_groups_chars() {
        let mut ed = Editor::new();
        for c in "hello".chars() {
            ed.handle_key(key(KeyCode::Char(c)));
        }
        assert_eq!(ed.text(), "hello");
        ed.undo();
        assert!(ed.is_empty());
    }

    #[test]
    fn coalescing_breaks_on_non_char_action() {
        let mut ed = Editor::new();
        ed.handle_key(key(KeyCode::Char('a')));
        ed.handle_key(key(KeyCode::Char('b')));
        ed.handle_key(key(KeyCode::Backspace));
        ed.handle_key(key(KeyCode::Char('c')));
        assert_eq!(ed.text(), "ac");
        ed.undo();
        assert_eq!(ed.text(), "a");
    }
}
