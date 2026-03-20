pub(crate) fn command_match_score(query: &str, command: &str) -> Option<i32> {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return Some(1);
    }

    let command = command.trim_start_matches('/').to_ascii_lowercase();

    let direct_score = single_command_match_score(&query, &command);
    let alias_score = command_aliases(&command)
        .iter()
        .filter_map(|alias| single_command_match_score(&query, alias).map(|score| score - 25))
        .max();

    match (direct_score, alias_score) {
        (Some(direct), Some(alias)) => Some(direct.max(alias)),
        (Some(direct), None) => Some(direct),
        (None, Some(alias)) => Some(alias),
        (None, None) => None,
    }
}

fn single_command_match_score(query: &str, command: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(1);
    }

    if command.starts_with(query) {
        let penalty = (command.len() as i32 - query.len() as i32).max(0);
        return Some(500 - penalty);
    }

    if let Some(pos) = command.find(query) {
        return Some(350 - pos as i32);
    }

    let mut query_chars = query.chars();
    let mut current = query_chars.next()?;
    let mut score = 200;
    let mut matched = 0usize;
    let mut prev_index = None;

    for (i, ch) in command.chars().enumerate() {
        if ch != current {
            continue;
        }

        matched += 1;
        if let Some(prev) = prev_index {
            if i == prev + 1 {
                score += 8;
            } else {
                score -= (i - prev) as i32;
            }
        }
        prev_index = Some(i);

        if let Some(next) = query_chars.next() {
            current = next;
        } else {
            score -= (command.len() as i32 - matched as i32).max(0);
            return Some(score);
        }
    }

    None
}

fn command_aliases(command: &str) -> &'static [&'static str] {
    super::commands::ALL_COMMANDS
        .iter()
        .find(|c| c.name().trim_start_matches('/') == command)
        .map(|c| c.aliases())
        .unwrap_or(&[])
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- empty query --

    #[test]
    fn empty_query_matches_everything() {
        assert_eq!(command_match_score("", "/chat"), Some(1));
        assert_eq!(command_match_score("  ", "/exit"), Some(1));
    }

    // -- prefix matches (score near 500) --

    #[test]
    fn exact_match_scores_500() {
        assert_eq!(single_command_match_score("chat", "chat"), Some(500));
    }

    #[test]
    fn prefix_match_penalizes_remaining_length() {
        // "ch" matches "chat" → 500 - 2
        assert_eq!(single_command_match_score("ch", "chat"), Some(498));
    }

    #[test]
    fn prefix_beats_substring() {
        let prefix = single_command_match_score("con", "connect").unwrap();
        let substring = single_command_match_score("nect", "connect").unwrap();
        assert!(prefix > substring);
    }

    // -- substring matches (score near 350) --

    #[test]
    fn substring_at_start_scores_350() {
        // position 0 → 350 - 0 (but this is also a prefix, so prefix branch wins)
        // use a mid-string match instead
        assert_eq!(single_command_match_score("line", "timeline"), Some(346));
    }

    #[test]
    fn substring_later_position_scores_lower() {
        let early = single_command_match_score("eet", "meetings").unwrap(); // pos 1 → 349
        let late = single_command_match_score("ings", "meetings").unwrap(); // pos 4 → 346
        assert!(early > late);
    }

    // -- fuzzy matches (score near 200) --

    #[test]
    fn fuzzy_smaller_gap_scores_higher() {
        // "mt" in "meetings": m(0), t(3) — gap 3
        let small_gap = single_command_match_score("mt", "meetings").unwrap();
        // "mg" in "meetings": m(0), g(6) — gap 6
        let big_gap = single_command_match_score("mg", "meetings").unwrap();
        assert!(small_gap > big_gap);
    }

    #[test]
    fn fuzzy_no_match_returns_none() {
        assert_eq!(single_command_match_score("xyz", "chat"), None);
        assert_eq!(single_command_match_score("zz", "exit"), None);
    }

    #[test]
    fn fuzzy_longer_command_penalized() {
        // both are fuzzy (no prefix/substring match for "mx")
        let short = single_command_match_score("mx", "mix").unwrap();
        let long = single_command_match_score("mx", "matrix").unwrap();
        assert!(short > long);
    }

    // -- case insensitivity & slash stripping --

    #[test]
    fn case_insensitive() {
        assert_eq!(
            command_match_score("CHAT", "/chat"),
            command_match_score("chat", "/chat"),
        );
    }

    #[test]
    fn strips_leading_slash_from_command() {
        assert_eq!(
            command_match_score("chat", "/chat"),
            command_match_score("chat", "chat"),
        );
    }

    // -- alias matching --

    #[test]
    fn alias_match_is_penalized() {
        // "quit" is an alias for "/exit"
        let direct = command_match_score("exit", "/exit").unwrap();
        let via_alias = command_match_score("quit", "/exit").unwrap();
        assert!(direct > via_alias);
    }

    #[test]
    fn alias_match_still_returns_some() {
        assert!(command_match_score("quit", "/exit").is_some());
    }

    // -- tier ordering --

    #[test]
    fn prefix_beats_substring_beats_fuzzy() {
        let prefix = single_command_match_score("con", "connect").unwrap();
        let substring = single_command_match_score("nect", "connect").unwrap();
        let fuzzy = single_command_match_score("cnt", "connect").unwrap();
        assert!(prefix > substring);
        assert!(substring > fuzzy);
    }
}
