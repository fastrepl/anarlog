use std::time::Instant;

use colored::Colorize;

#[derive(Clone, Copy)]
pub enum ChannelKind {
    Mic,
    Speaker,
}

impl ChannelKind {
    fn confirmed_color(&self) -> (u8, u8, u8) {
        match self {
            ChannelKind::Mic => (255, 190, 190),
            ChannelKind::Speaker => (190, 200, 255),
        }
    }

    fn partial_color(&self) -> (u8, u8, u8) {
        match self {
            ChannelKind::Mic => (128, 95, 95),
            ChannelKind::Speaker => (95, 100, 128),
        }
    }
}

pub enum DisplayMode {
    Single(ChannelKind),
    Dual,
}

fn fmt_ts(secs: f64) -> String {
    let m = (secs / 60.0) as u32;
    let s = secs % 60.0;
    format!("{:02}:{:02}", m, s as u32)
}

struct Segment {
    time: f64,
    text: String,
}

pub struct Transcript {
    segments: Vec<Segment>,
    partial: String,
    t0: Instant,
    kind: ChannelKind,
}

impl Transcript {
    pub fn new(t0: Instant, kind: ChannelKind) -> Self {
        Self {
            segments: Vec::new(),
            partial: String::new(),
            t0,
            kind,
        }
    }

    fn elapsed(&self) -> f64 {
        self.t0.elapsed().as_secs_f64()
    }

    pub fn set_partial(&mut self, text: &str) {
        self.partial = text.to_string();
        self.render();
    }

    pub fn confirm(&mut self, text: &str) {
        self.segments.push(Segment {
            time: self.elapsed(),
            text: text.to_string(),
        });
        self.partial.clear();
        self.trim();
        self.render();
    }

    fn trim(&mut self) {
        const OVERHEAD: usize = 70;
        let max_chars = crossterm::terminal::size()
            .map(|(cols, _)| (cols as usize).saturating_sub(OVERHEAD))
            .unwrap_or(120);

        let partial_len = if self.partial.is_empty() {
            0
        } else {
            self.partial.len() + 1
        };
        let total_len: usize = self
            .segments
            .iter()
            .map(|s| s.text.len() + 1)
            .sum::<usize>()
            + partial_len;
        if total_len > max_chars {
            let drain_count = self.segments.len() * 2 / 3;
            if drain_count > 0 {
                self.segments.drain(..drain_count);
            }
        }
    }

    fn render(&self) {
        let confirmed: String = self
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        if confirmed.is_empty() && self.partial.is_empty() {
            return;
        }

        let to = self.elapsed();
        let from = self.segments.first().map(|s| fmt_ts(s.time));
        let prefix = format!("[{} / {}]", from.as_deref().unwrap_or("--:--"), fmt_ts(to)).dimmed();

        let (r, g, b) = self.kind.confirmed_color();
        let colored_confirmed = confirmed.truecolor(r, g, b).bold();

        let colored_partial = if self.partial.is_empty() {
            None
        } else {
            let (r, g, b) = self.kind.partial_color();
            Some(self.partial.truecolor(r, g, b))
        };

        if let Some(partial) = colored_partial {
            eprintln!("{} {} {}", prefix, colored_confirmed, partial);
        } else {
            eprintln!("{} {}", prefix, colored_confirmed);
        }
    }
}
