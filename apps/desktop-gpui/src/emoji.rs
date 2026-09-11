//! The `@emoji-mart/data` native set the template icon picker offers, in the
//! app's eight categories and category order (`assets/emoji.tsv`, generated
//! from `sets/15/native.json`: category, id, native, name, keywords).

use std::sync::LazyLock;

pub struct Emoji {
    pub category: &'static str,
    pub id: &'static str,
    pub native: &'static str,
    /// `[id, name, ...keywords].join(" ").toLowerCase()`
    pub search: String,
}

pub struct Category {
    pub id: &'static str,
    pub emojis: Vec<&'static Emoji>,
}

const TSV: &str = include_str!("../assets/emoji.tsv");

/// `EMOJI_CATEGORY_IDS` in `data.categories` order.
pub const CATEGORY_IDS: [&str; 8] = [
    "people", "nature", "foods", "activity", "places", "objects", "symbols", "flags",
];

/// `FREQUENT_EMOJI_IDS`
pub const FREQUENT_IDS: [&str; 22] = [
    "ok_hand",
    "heart",
    "white_check_mark",
    "+1",
    "pray",
    "joy",
    "eyes",
    "slightly_smiling_face",
    "grinning",
    "smile",
    "thinking_face",
    "sweat_smile",
    "warning",
    "confused",
    "x",
    "raised_hands",
    "tada",
    "wink",
    "blush",
    "shrug",
    "wave",
    "question",
];

pub static EMOJIS: LazyLock<Vec<Emoji>> = LazyLock::new(|| {
    TSV.lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let category = fields.next()?;
            let id = fields.next()?;
            let native = fields.next()?;
            let name = fields.next()?;
            let keywords = fields.next().unwrap_or("");
            Some(Emoji {
                category,
                id,
                native,
                search: format!("{id} {name} {keywords}").to_lowercase(),
            })
        })
        .collect()
});

/// `EMOJI_CATEGORIES`
pub static CATEGORIES: LazyLock<Vec<Category>> = LazyLock::new(|| {
    CATEGORY_IDS
        .iter()
        .map(|id| Category {
            id,
            emojis: EMOJIS
                .iter()
                .filter(|emoji| emoji.category == *id)
                .collect(),
        })
        .collect()
});

pub fn by_id(id: &str) -> Option<&'static Emoji> {
    EMOJIS.iter().find(|emoji| emoji.id == id)
}

/// `emojiCategoryLabels`
pub fn category_label(id: &str) -> &'static str {
    match id {
        "people" => "Smileys & People",
        "nature" => "Animals & Nature",
        "foods" => "Food & Drink",
        "activity" => "Activity",
        "places" => "Travel & Places",
        "objects" => "Objects",
        "symbols" => "Symbols",
        "flags" => "Flags",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_set_covers_every_category_and_frequent_id() {
        assert_eq!(CATEGORIES.len(), 8);
        assert!(
            CATEGORIES
                .iter()
                .all(|category| !category.emojis.is_empty())
        );
        for id in FREQUENT_IDS {
            assert!(by_id(id).is_some(), "{id} missing");
        }
        let grinning = by_id("grinning").unwrap();
        assert_eq!(grinning.native, "😀");
        assert!(grinning.search.contains("grinning face"));
    }
}
