const STATUS_COLUMN_WIDTH: usize = 6;

#[derive(Debug, Clone, Copy)]
pub struct Styles {
    enabled: bool,
}

impl Styles {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub fn heading(self, value: &str) -> String {
        self.paint(value, "1;36")
    }

    pub fn hint(self, value: &str) -> String {
        self.paint(value, "2;37")
    }

    pub fn error(self, value: &str) -> String {
        self.paint(value, "1;31")
    }

    pub fn policy(self, value: &str) -> String {
        self.paint(value, "1;35")
    }

    pub fn bool(self, value: bool) -> String {
        if value {
            self.paint("yes", "1;32")
        } else {
            self.paint("no", "1;31")
        }
    }

    pub fn status(self, value: &str) -> String {
        let padded = format!("{value:STATUS_COLUMN_WIDTH$}");
        match value {
            "focus" => self.paint(&padded, "1;36"),
            "update" => self.paint(&padded, "1;33"),
            "idle" => self.paint(&padded, "1;90"),
            _ => self.paint(&padded, "1;34"),
        }
    }

    pub fn app(self, value: &str, width: usize) -> String {
        let palette = ["1;34", "1;35", "1;36", "1;32", "1;33", "1;31"];
        let index = value.bytes().fold(0usize, |acc, byte| {
            acc.wrapping_mul(31).wrapping_add(byte as usize)
        }) % palette.len();
        let padded = format!("{value:width$}");
        self.paint(&padded, palette[index])
    }

    fn paint(self, value: &str, ansi: &str) -> String {
        if self.enabled {
            format!("\x1b[{ansi}m{value}\x1b[0m")
        } else {
            value.to_string()
        }
    }
}
