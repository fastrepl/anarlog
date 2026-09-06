//! `packages/editor/src/widgets/mention.tsx`: the `@` suggestion popup. The
//! mention state is derived from the caret on every change (`findMention`),
//! the candidates come from the workspace (`useMentionConfig`), Up / Down /
//! Enter / Escape act on the popup while it shows results, and choosing an
//! item replaces `@query` with the mention node followed by a space.

use std::rc::Rc;

use serde_json::{Value, json};

use super::model::Caret;

pub const TRIGGER: char = '@';
/// `results.slice(0, 5)`
pub const MAX_ITEMS: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionItem {
    pub id: String,
    /// `session`, `human`, or `organization`.
    pub kind: String,
    pub label: String,
}

/// `MentionConfig.handleSearch`, synchronous: the workspace resolves the
/// candidates from the rows it already holds.
pub type Search = Rc<dyn Fn(&str) -> Vec<MentionItem>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionState {
    pub block: usize,
    /// Byte range of `@query` in the block's text.
    pub from: usize,
    pub to: usize,
    pub query: String,
    pub items: Vec<MentionItem>,
    pub selected: usize,
}

/// `findMention`: the trigger closest before the caret, at the block start or
/// after whitespace, with no whitespace between it and the caret. Atom text
/// (mentions) is masked so a chip's own characters never look like a trigger.
pub fn find_mention(
    text: &str,
    atoms: &[std::ops::Range<usize>],
    caret: Caret,
) -> Option<(usize, usize, String)> {
    let before = &text[..caret.offset.min(text.len())];
    let mut masked = before.to_string();
    for range in atoms {
        if range.end <= masked.len() {
            // `textBetween(..., "\ufffc")` puts one object replacement per
            // atom; keep the byte length so offsets stay aligned, ending on
            // the replacement so the character before a following `@` is not
            // whitespace.
            let len = range.end - range.start;
            let marker = '\u{FFFC}';
            let filler = if len >= marker.len_utf8() {
                format!("{}{marker}", " ".repeat(len - marker.len_utf8()))
            } else {
                " ".repeat(len)
            };
            masked.replace_range(range.clone(), &filler);
        }
    }
    let trigger_index = masked.rfind(TRIGGER)?;
    if trigger_index > 0
        && !masked[..trigger_index]
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace)
    {
        return None;
    }
    let query = &masked[trigger_index + TRIGGER.len_utf8()..];
    if query.chars().any(char::is_whitespace) {
        return None;
    }
    Some((trigger_index, caret.offset, query.to_string()))
}

/// The `mention-@` node ProseMirror creates: attrs in schema order.
pub fn mention_node(item: &MentionItem) -> Value {
    json!({
        "type": "mention-@",
        "attrs": { "id": item.id, "type": item.kind, "label": item.label }
    })
}

/// `useMentionConfig().handleSearch` over the rows the workspace holds: an
/// empty query lists sessions with titles, then humans, then organizations;
/// a query matches labels case-insensitively (the web app asks the tantivy
/// index here, which also matches note contents).
pub fn search_candidates(candidates: &[MentionItem], query: &str) -> Vec<MentionItem> {
    let query = query.trim();
    if query.is_empty() {
        return candidates.iter().take(MAX_ITEMS).cloned().collect();
    }
    let needle = query.to_lowercase();
    candidates
        .iter()
        .filter(|item| item.label.to_lowercase().contains(&needle))
        .take(MAX_ITEMS)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caret(offset: usize) -> Caret {
        Caret { block: 0, offset }
    }

    #[test]
    fn finds_the_trigger_at_a_word_start_without_spaces_in_the_query() {
        assert_eq!(
            find_mention("hello @ad", &[], caret(9)),
            Some((6, 9, "ad".into()))
        );
        assert_eq!(
            find_mention("@", &[], caret(1)),
            Some((0, 1, String::new()))
        );
        // Mid-word `@` and a space after the query do not trigger.
        assert_eq!(find_mention("mail@x", &[], caret(6)), None);
        assert_eq!(find_mention("@ada lovelace", &[], caret(13)), None);
        // Text after the caret is ignored.
        assert_eq!(
            find_mention("@ad tail", &[], caret(3)),
            Some((0, 3, "ad".into()))
        );
    }

    #[test]
    fn a_chip_is_not_a_trigger() {
        let chip = crate::mention::display_text("Ada");
        let text = format!("{chip} and @b");
        let atoms: Vec<std::ops::Range<usize>> = std::iter::once(0..chip.len()).collect();
        assert_eq!(
            find_mention(&text, &atoms, caret(text.len())),
            Some((text.len() - 2, text.len(), "b".into()))
        );
        // The caret right after a chip: the masked atom is not whitespace.
        let text = format!("{chip}@x");
        assert_eq!(find_mention(&text, &atoms, caret(text.len())), None);
    }

    #[test]
    fn candidate_search_lists_five_and_filters_by_label() {
        let items: Vec<MentionItem> = (0..7)
            .map(|i| MentionItem {
                id: format!("s{i}"),
                kind: "session".into(),
                label: format!("Meeting {i}"),
            })
            .collect();
        assert_eq!(search_candidates(&items, "").len(), 5);
        assert_eq!(search_candidates(&items, "ing 6").len(), 1);
        assert_eq!(search_candidates(&items, "ING 6")[0].id, "s6");
        assert!(search_candidates(&items, "zzz").is_empty());
    }

    #[test]
    fn mention_node_has_schema_attr_order() {
        let node = mention_node(&MentionItem {
            id: "h1".into(),
            kind: "human".into(),
            label: "Ada".into(),
        });
        assert_eq!(
            node.to_string(),
            r#"{"type":"mention-@","attrs":{"id":"h1","type":"human","label":"Ada"}}"#
        );
    }
}
