#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockCommand {
    Paragraph,
    Heading(u8),
    BulletList,
    OrderedList,
    Quote,
    Code,
    Divider,
    TaskList,
    Table,
}

impl BlockCommand {
    pub const ALL: [(Self, &'static str); 11] = [
        (Self::Paragraph, "Text"),
        (Self::Heading(1), "Heading 1"),
        (Self::Heading(2), "Heading 2"),
        (Self::Heading(3), "Heading 3"),
        (Self::BulletList, "Bullet List"),
        (Self::OrderedList, "Numbered List"),
        (Self::Quote, "Quote"),
        (Self::Code, "Code Block"),
        (Self::Divider, "Divider"),
        (Self::TaskList, "Task List"),
        (Self::Table, "Table"),
    ];

    pub fn description(self) -> &'static str {
        match self {
            Self::Paragraph => "Plain text",
            Self::Heading(1) => "Large heading",
            Self::Heading(2) => "Medium heading",
            Self::Heading(_) => "Small heading",
            Self::BulletList => "Unordered list",
            Self::OrderedList => "Ordered list",
            Self::TaskList => "Track a task",
            Self::Quote => "Capture a quote",
            Self::Code => "Monospace code",
            Self::Divider => "Horizontal rule",
            Self::Table => "Three-column grid",
        }
    }

    fn keywords(self) -> &'static str {
        match self {
            Self::Paragraph => "text paragraph plain",
            Self::Heading(1) => "heading h1 title large",
            Self::Heading(2) => "heading h2 subtitle medium",
            Self::Heading(_) => "heading h3 small",
            Self::BulletList => "bullet list unordered ul",
            Self::OrderedList => "numbered list ordered ol",
            Self::TaskList => "task todo checkbox checklist",
            Self::Quote => "quote blockquote",
            Self::Code => "code pre codeblock",
            Self::Divider => "divider rule horizontal hr",
            Self::Table => "table grid",
        }
    }
}

pub struct SlashMenu {
    pub query: String,
    pub selected: usize,
    pub start: usize,
}

impl SlashMenu {
    pub fn items(&self) -> Vec<(BlockCommand, &'static str)> {
        let query = self.query.to_ascii_lowercase();
        BlockCommand::ALL
            .into_iter()
            .filter(|(command, label)| {
                label.to_ascii_lowercase().contains(&query) || command.keywords().contains(&query)
            })
            .collect()
    }

    pub fn step(&mut self, forward: bool) {
        let len = self.items().len();
        if len > 0 {
            self.selected = (self.selected + if forward { 1 } else { len - 1 }) % len;
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MentionTarget {
    Human(String),
    Session(String),
    Organization(String),
}

#[derive(Clone, Debug)]
pub struct MentionCandidate {
    pub label: String,
    pub target: MentionTarget,
}

#[derive(Clone, Debug)]
pub enum EditorRequest {
    MentionSearch { query: String, generation: u64 },
    OpenSession(String),
    OpenOrganization(String),
}

pub struct MentionMenu {
    pub query: String,
    pub start: usize,
    pub selected: usize,
    pub results: MentionResults,
}

#[derive(Default)]
pub struct MentionResults {
    generation: u64,
    active: bool,
    pub candidates: Vec<MentionCandidate>,
    pub error: Option<String>,
    pub loading: bool,
}

impl MentionResults {
    pub fn begin(&mut self) -> u64 {
        self.loading = true;
        self.generation += 1;
        self.active = true;
        self.candidates.clear();
        self.error = None;
        self.generation
    }

    pub fn dismiss(&mut self) {
        self.loading = false;
        self.active = false;
        self.candidates.clear();
        self.error = None;
    }

    pub fn resolve(
        &mut self,
        generation: u64,
        result: Result<Vec<MentionCandidate>, String>,
    ) -> bool {
        if generation != self.generation || !self.active {
            return false;
        }
        self.loading = false;
        match result {
            Ok(mut candidates) => {
                candidates.truncate(5);
                self.candidates = candidates;
                self.error = None;
            }
            Err(error) => {
                self.candidates.clear();
                self.error = Some(error);
            }
        }
        true
    }
}
