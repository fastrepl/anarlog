pub(super) fn escape_typst_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('#', "\\#")
        .replace('$', "\\$")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('{', "\\{")
        .replace('}', "\\}")
        .replace('<', "\\<")
        .replace('>', "\\>")
        .replace('@', "\\@")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('`', "\\`")
}

pub(super) fn escape_typst_literal(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        // `#` interpolates inside quoted strings; the unicode escape is a
        // literal hash in the rendered output.
        .replace('#', "\\u{23}")
}
