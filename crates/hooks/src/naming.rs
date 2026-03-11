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
    fn transforms_snake_case_into_cli_flag() {
        assert_eq!(cli_flag("resource_dir"), "--resource-dir");
        assert_eq!(cli_flag("app_hyprnote"), "--app-hyprnote");
        assert_eq!(cli_flag("app_meeting"), "--app-meeting");
    }

    #[test]
    fn leaves_hyphenated_names_intact() {
        assert_eq!(cli_flag("already-hyphenated"), "--already-hyphenated");
    }

    #[test]
    fn handles_empty_strings() {
        assert_eq!(cli_flag(""), "--");
    }
}
