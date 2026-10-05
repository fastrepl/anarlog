pub fn contains_control_or_whitespace(value: &str) -> bool {
    value.chars().any(|c| c.is_whitespace() || c.is_control())
}
