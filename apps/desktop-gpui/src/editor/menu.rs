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
    /// Shipping's slash commands, in its order. Tables stay insertable through paste and
    /// existing documents, but shipping's slash menu does not offer them.
    pub const ALL: [(Self, &'static str); 10] = [
        (Self::Paragraph, "Text"),
        (Self::Heading(1), "Heading 1"),
        (Self::Heading(2), "Heading 2"),
        (Self::Heading(3), "Heading 3"),
        (Self::BulletList, "Bullet List"),
        (Self::OrderedList, "Numbered List"),
        (Self::TaskList, "Task List"),
        (Self::Quote, "Quote"),
        (Self::Code, "Code Block"),
        (Self::Divider, "Divider"),
    ];

    pub fn icon(self) -> &'static str {
        match self {
            Self::Paragraph => "editor/TextIcon.svg",
            Self::Heading(1) => "editor/Heading01Icon.svg",
            Self::Heading(2) => "editor/Heading02Icon.svg",
            Self::Heading(_) => "editor/Heading03Icon.svg",
            Self::BulletList => "editor/LeftToRightListBulletIcon.svg",
            Self::OrderedList => "editor/LeftToRightListNumberIcon.svg",
            Self::TaskList => "editor/ListChecksIcon.svg",
            Self::Quote => "editor/QuotesIcon.svg",
            Self::Code => "editor/CodeIcon.svg",
            Self::Divider => "editor/MinusIcon.svg",
            Self::Table => "editor/TextIcon.svg",
        }
    }

    fn keywords(self) -> &'static [&'static str] {
        match self {
            Self::Paragraph => &["text", "paragraph", "plain"],
            Self::Heading(1) => &["heading", "h1", "title", "large"],
            Self::Heading(2) => &["heading", "h2", "subtitle", "medium"],
            Self::Heading(_) => &["heading", "h3", "small"],
            Self::BulletList => &["bullet", "list", "unordered", "ul"],
            Self::OrderedList => &["numbered", "list", "ordered", "ol"],
            Self::TaskList => &["task", "todo", "checkbox", "check"],
            Self::Quote => &["quote", "blockquote", "callout"],
            Self::Code => &["code", "pre", "block", "snippet"],
            Self::Divider => &["divider", "horizontal", "rule", "line", "hr"],
            Self::Table => &["table", "grid"],
        }
    }
}

/// Shipping's `findSlashCommand` over the text from the block start to the caret: the last "/"
/// opens the menu when it starts the block or follows whitespace, and the query after it has
/// no whitespace. Returns the byte offset of the "/" and the query.
pub fn slash_query(text: &str) -> Option<(usize, &str)> {
    let slash = text.rfind('/')?;
    let query = &text[slash + 1..];
    let after_space = text[..slash]
        .chars()
        .next_back()
        .is_none_or(char::is_whitespace);
    (after_space && !query.chars().any(char::is_whitespace)).then_some((slash, query))
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
                label.to_ascii_lowercase().contains(&query)
                    || command
                        .keywords()
                        .iter()
                        .any(|keyword| keyword.contains(&query))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slash_opens_after_whitespace_like_shipping() {
        assert_eq!(slash_query("/"), Some((0, "")));
        assert_eq!(slash_query("wiki. /hea"), Some((6, "hea")));
        assert_eq!(slash_query("and/or"), None);
        assert_eq!(slash_query("/heading two"), None);
        assert_eq!(slash_query("plain text"), None);
        let menu = SlashMenu {
            query: "list".into(),
            selected: 0,
            start: 0,
        };
        let labels: Vec<_> = menu.items().into_iter().map(|(_, label)| label).collect();
        assert_eq!(labels, ["Bullet List", "Numbered List", "Task List"]);
    }
}
