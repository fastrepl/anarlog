//! Inline mention chips (`packages/editor/src/node-views/mention-view.tsx`):
//! the node's display text and the avatar the chip carries.

/// `MAX_MENTION_LENGTH`
pub const MAX_LABEL_CHARS: usize = 20;

/// The `.mention-avatar` box (`1em`, `margin-right: 0.15em`) reserved in the
/// text run: an em space plus a six-per-em space (≈1.167em), the closest
/// fixed-advance pair to 1.15em, so the label lands where WebKit puts it.
pub const AVATAR_PLACEHOLDER: &str = "\u{2003}\u{2006}";

/// `displayLabel`: the label cut to 20 characters with an ellipsis.
pub fn display_label(label: &str) -> String {
    let chars: Vec<char> = label.chars().collect();
    if chars.len() > MAX_LABEL_CHARS {
        let mut cut: String = chars[..MAX_LABEL_CHARS].iter().collect();
        cut.push('\u{2026}');
        cut
    } else {
        label.to_string()
    }
}

/// The text a mention atom occupies in a paragraph's plain text: the avatar
/// placeholder followed by the display label. The editor model and the
/// renderer both use this, so caret offsets and layout agree.
pub fn display_text(label: &str) -> String {
    format!("{AVATAR_PLACEHOLDER}{}", display_label(label))
}

/// facehash's `stringHash`: a positive JavaScript 32-bit hash of the name.
pub fn string_hash(value: &str) -> u32 {
    let mut hash: i32 = 0;
    for unit in value.encode_utf16() {
        hash = (hash << 5).wrapping_sub(hash).wrapping_add(unit as i32);
    }
    hash.unsigned_abs()
}

/// `FACEHASH_BG_CLASSES`: Tailwind `*-50` (light) / `*-950` (dark) pairs for
/// amber, rose, violet, blue, teal, green, cyan, fuchsia, indigo, yellow.
const FACEHASH_BG: [(u32, u32); 10] = [
    (0xfffbeb, 0x451a03),
    (0xfff1f2, 0x4c0519),
    (0xf5f3ff, 0x2e1065),
    (0xeff6ff, 0x172554),
    (0xf0fdfa, 0x042f2e),
    (0xf0fdf4, 0x052e16),
    (0xecfeff, 0x083344),
    (0xfdf4ff, 0x4a044e),
    (0xeef2ff, 0x1e1b4b),
    (0xfefce8, 0x422006),
];

/// `getMentionFacehashBgClass(name)` resolved to a colour.
pub fn facehash_background(name: &str, dark: bool) -> u32 {
    let (light, dark_color) = FACEHASH_BG[(string_hash(name) % FACEHASH_BG.len() as u32) as usize];
    if dark { dark_color } else { light }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_truncate_at_twenty_characters() {
        assert_eq!(display_label("Ada Lovelace"), "Ada Lovelace");
        assert_eq!(
            display_label("A very long meeting title indeed"),
            "A very long meeting \u{2026}"
        );
        assert!(display_text("Ada").starts_with(AVATAR_PLACEHOLDER));
    }

    #[test]
    fn string_hash_matches_the_javascript_reference() {
        // `stringHash("Ada")`: ((65<<5)-65+100)<<5 ... computed with the same
        // 32-bit wraparound the JS engine applies.
        assert_eq!(string_hash(""), 0);
        assert_eq!(string_hash("a"), 97);
        assert_eq!(string_hash("Ada"), 65 * 31 * 31 + 100 * 31 + 97);
        // Long inputs wrap like JavaScript's `hash &= hash`.
        assert_eq!(string_hash("Grace Brewster Murray Hopper"), 226_433_908);
        assert_eq!(facehash_background("Ada", false), 0xf5f3ff);
        assert_eq!(facehash_background("Grace Hopper", true), 0x042f2e);
    }
}
