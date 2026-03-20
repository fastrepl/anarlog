pub(crate) struct Cursor {
    pub row: usize,
    pub col: usize,
}

impl Cursor {
    pub fn new() -> Self {
        Self { row: 0, col: 0 }
    }

    pub fn clamp(&mut self, lines: &[String]) {
        if lines.is_empty() {
            self.row = 0;
            self.col = 0;
            return;
        }
        if self.row >= lines.len() {
            self.row = lines.len() - 1;
        }
        let max_col = lines[self.row].chars().count();
        if self.col > max_col {
            self.col = max_col;
        }
    }

    pub fn move_left(&mut self, lines: &[String]) -> bool {
        if self.col > 0 {
            self.col -= 1;
            true
        } else if self.row > 0 {
            self.row -= 1;
            self.col = lines[self.row].chars().count();
            true
        } else {
            false
        }
    }

    pub fn move_right(&mut self, lines: &[String]) -> bool {
        let max_col = lines.get(self.row).map(|l| l.chars().count()).unwrap_or(0);
        if self.col < max_col {
            self.col += 1;
            true
        } else if self.row + 1 < lines.len() {
            self.row += 1;
            self.col = 0;
            true
        } else {
            false
        }
    }

    pub fn move_up(&mut self, lines: &[String]) {
        if self.row > 0 {
            self.row -= 1;
            let max = lines[self.row].chars().count();
            if self.col > max {
                self.col = max;
            }
        }
    }

    pub fn move_down(&mut self, lines: &[String]) {
        if self.row + 1 < lines.len() {
            self.row += 1;
            let max = lines[self.row].chars().count();
            if self.col > max {
                self.col = max;
            }
        }
    }

    pub fn move_home(&mut self) {
        self.col = 0;
    }

    pub fn move_end(&mut self, lines: &[String]) {
        self.col = lines.get(self.row).map(|l| l.chars().count()).unwrap_or(0);
    }

    pub fn move_word_forward(&mut self, lines: &[String]) {
        let line: Vec<char> = lines
            .get(self.row)
            .map(|s| s.chars().collect())
            .unwrap_or_default();
        let len = line.len();

        if self.col >= len {
            if self.row + 1 < lines.len() {
                self.row += 1;
                self.col = 0;
            }
            return;
        }

        let mut pos = self.col;
        while pos < len && is_word_char(line[pos]) {
            pos += 1;
        }
        while pos < len && !is_word_char(line[pos]) {
            pos += 1;
        }
        self.col = pos;
    }

    pub fn move_to_first_non_blank(&mut self, lines: &[String]) {
        if let Some(line) = lines.get(self.row) {
            self.col = line.chars().take_while(|c| c.is_whitespace()).count();
        }
    }

    pub fn move_word_back(&mut self, lines: &[String]) {
        if self.col == 0 {
            if self.row > 0 {
                self.row -= 1;
                self.col = lines[self.row].chars().count();
            }
            return;
        }

        let line: Vec<char> = lines
            .get(self.row)
            .map(|s| s.chars().collect())
            .unwrap_or_default();
        let mut pos = self.col;
        while pos > 0 && !is_word_char(line[pos - 1]) {
            pos -= 1;
        }
        while pos > 0 && is_word_char(line[pos - 1]) {
            pos -= 1;
        }
        self.col = pos;
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}
