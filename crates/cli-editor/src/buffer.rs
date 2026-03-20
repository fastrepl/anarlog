pub(crate) struct Buffer {
    lines: Vec<String>,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
        }
    }

    pub fn from_lines(lines: Vec<String>) -> Self {
        if lines.is_empty() {
            return Self::new();
        }
        Self { lines }
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn line(&self, row: usize) -> &str {
        self.lines.get(row).map(String::as_str).unwrap_or("")
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].is_empty()
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn line_char_count(&self, row: usize) -> usize {
        self.lines.get(row).map(|l| l.chars().count()).unwrap_or(0)
    }

    pub fn insert_char(&mut self, row: usize, col: usize, ch: char) -> usize {
        if let Some(line) = self.lines.get_mut(row) {
            let byte_pos = char_to_byte(line, col);
            line.insert(byte_pos, ch);
            col + 1
        } else {
            col
        }
    }

    pub fn delete_char_before(&mut self, row: usize, col: usize) -> Option<(usize, usize)> {
        if col > 0 {
            if let Some(line) = self.lines.get_mut(row) {
                let start = char_to_byte(line, col - 1);
                let end = char_to_byte(line, col);
                line.drain(start..end);
                return Some((row, col - 1));
            }
        } else if row > 0 {
            let current = self.lines.remove(row);
            let prev_chars = self.lines[row - 1].chars().count();
            self.lines[row - 1].push_str(&current);
            return Some((row - 1, prev_chars));
        }
        None
    }

    pub fn delete_char_at(&mut self, row: usize, col: usize) -> bool {
        let Some(line) = self.lines.get(row) else {
            return false;
        };
        let chars = line.chars().count();
        if col < chars {
            let line = &mut self.lines[row];
            let start = char_to_byte(line, col);
            let end = char_to_byte(line, col + 1);
            line.drain(start..end);
            true
        } else if row + 1 < self.lines.len() {
            let next = self.lines.remove(row + 1);
            self.lines[row].push_str(&next);
            true
        } else {
            false
        }
    }

    pub fn insert_newline(&mut self, row: usize, col: usize) -> (usize, usize) {
        if let Some(line) = self.lines.get_mut(row) {
            let byte_pos = char_to_byte(line, col);
            let remainder = line[byte_pos..].to_string();
            line.truncate(byte_pos);
            self.lines.insert(row + 1, remainder);
            (row + 1, 0)
        } else {
            (row, col)
        }
    }

    pub fn insert_str_at(&mut self, row: usize, col: usize, text: &str) -> (usize, usize) {
        let mut r = row;
        let mut c = col;
        for (i, part) in text.split('\n').enumerate() {
            if i > 0 {
                let pos = self.insert_newline(r, c);
                r = pos.0;
                c = pos.1;
            }
            if let Some(line) = self.lines.get_mut(r) {
                let byte_pos = char_to_byte(line, c);
                line.insert_str(byte_pos, part);
                c += part.chars().count();
            }
        }
        (r, c)
    }

    pub fn delete_line(&mut self, row: usize) -> bool {
        if row >= self.lines.len() {
            return false;
        }
        if self.lines.len() > 1 {
            self.lines.remove(row);
        } else {
            self.lines[0].clear();
        }
        true
    }

    pub fn insert_empty_line_after(&mut self, row: usize) {
        let idx = (row + 1).min(self.lines.len());
        self.lines.insert(idx, String::new());
    }

    pub fn insert_empty_line_before(&mut self, row: usize) {
        let idx = row.min(self.lines.len());
        self.lines.insert(idx, String::new());
    }

    pub fn delete_to_end_of_line(&mut self, row: usize, col: usize) {
        if let Some(line) = self.lines.get_mut(row) {
            let byte_pos = char_to_byte(line, col);
            line.truncate(byte_pos);
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
        self.lines.push(String::new());
    }

    pub fn snapshot(&self) -> Vec<String> {
        self.lines.clone()
    }

    pub fn restore(&mut self, lines: Vec<String>) {
        self.lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };
    }
}

fn char_to_byte(s: &str, char_pos: usize) -> usize {
    s.char_indices()
        .nth(char_pos)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_char_basic() {
        let mut buf = Buffer::new();
        let col = buf.insert_char(0, 0, 'a');
        assert_eq!(col, 1);
        assert_eq!(buf.line(0), "a");
        let col = buf.insert_char(0, col, 'b');
        assert_eq!(col, 2);
        assert_eq!(buf.line(0), "ab");
    }

    #[test]
    fn delete_char_before_joins_lines() {
        let mut buf = Buffer::from_lines(vec!["hello".into(), "world".into()]);
        let result = buf.delete_char_before(1, 0);
        assert_eq!(result, Some((0, 5)));
        assert_eq!(buf.line(0), "helloworld");
        assert_eq!(buf.line_count(), 1);
    }

    #[test]
    fn insert_newline_splits_line() {
        let mut buf = Buffer::from_lines(vec!["helloworld".into()]);
        let (r, c) = buf.insert_newline(0, 5);
        assert_eq!(r, 1);
        assert_eq!(c, 0);
        assert_eq!(buf.line(0), "hello");
        assert_eq!(buf.line(1), "world");
    }

    #[test]
    fn insert_str_multiline() {
        let mut buf = Buffer::new();
        let (r, c) = buf.insert_str_at(0, 0, "hello\nworld");
        assert_eq!(r, 1);
        assert_eq!(c, 5);
        assert_eq!(buf.line(0), "hello");
        assert_eq!(buf.line(1), "world");
    }

    #[test]
    fn is_empty_check() {
        let buf = Buffer::new();
        assert!(buf.is_empty());
        let buf = Buffer::from_lines(vec!["a".into()]);
        assert!(!buf.is_empty());
    }
}
