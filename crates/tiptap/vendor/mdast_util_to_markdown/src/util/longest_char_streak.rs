//! JS equivalent https://github.com/wooorm/longest-streak/blob/main/index.js

pub fn longest_char_streak(haystack: &str, needle: char) -> usize {
    let mut max = 0;
    let mut chars = haystack.chars();

    while let Some(char) = chars.next() {
        if char == needle {
            let mut count = 1;
            for char in chars.by_ref() {
                if char == needle {
                    count += 1;
                } else {
                    break;
                }
            }
            max = count.max(max);
        }
    }

    max
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longest_streak_tests() {
        assert_eq!(longest_char_streak("", 'f'), 0);
        assert_eq!(longest_char_streak("foo", 'o'), 2);
        assert_eq!(longest_char_streak("fo foo fo", 'o'), 2);
        assert_eq!(longest_char_streak("fo foo foo", 'o'), 2);

        assert_eq!(longest_char_streak("fo fooo fo", 'o'), 3);
        assert_eq!(longest_char_streak("fo fooo foo", 'o'), 3);
        assert_eq!(longest_char_streak("ooo", 'o'), 3);
        assert_eq!(longest_char_streak("fo fooo fooooo", 'o'), 5);

        assert_eq!(longest_char_streak("fo fooooo fooo", 'o'), 5);
        assert_eq!(longest_char_streak("fo fooooo fooooo", 'o'), 5);

        assert_eq!(longest_char_streak("'`'", '`'), 1);
        assert_eq!(longest_char_streak("'`'", '`'), 1);
    }
}
