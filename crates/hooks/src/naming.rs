pub fn cli_flag(field_name: &str) -> String {
    let mut flag = String::with_capacity(field_name.len() + 2);
    flag.push_str("--");

    for ch in field_name.chars() {
        flag.push(if ch == '_' { '-' } else { ch });
    }

    flag
}

#[cfg(test)]
mod tests {
    use super::cli_flag;

    #[test]
    fn cli_flag_prefixes_and_hyphenates() {
        for (input, expected) in [
            ("resource_dir", "--resource-dir"),
            ("app_hyprnote", "--app-hyprnote"),
            ("app_meeting", "--app-meeting"),
            ("already-hyphenated", "--already-hyphenated"),
            ("", "--"),
        ] {
            assert_eq!(cli_flag(input), expected, "input: {input}");
        }
    }
}
