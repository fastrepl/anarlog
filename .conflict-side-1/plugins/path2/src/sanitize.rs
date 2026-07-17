pub fn sanitize(name: &str) -> String {
    sanitize_filename::sanitize_with_options(
        name,
        sanitize_filename::Options {
            windows: true,
            truncate: true,
            replacement: "_",
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_valid_name() {
        assert_eq!(sanitize("valid-filename"), "valid-filename");
    }

    #[test]
    fn test_sanitize_illegal_chars() {
        assert_eq!(sanitize("file<name"), "file_name");
        assert_eq!(sanitize("file>name"), "file_name");
        assert_eq!(sanitize("file:name"), "file_name");
        assert_eq!(sanitize("file/name"), "file_name");
        assert_eq!(sanitize("file\\name"), "file_name");
        assert_eq!(sanitize("file|name"), "file_name");
        assert_eq!(sanitize("file?name"), "file_name");
        assert_eq!(sanitize("file*name"), "file_name");
    }

    #[test]
    fn test_sanitize_windows_reserved() {
        assert_eq!(sanitize("CON"), "_");
        assert_eq!(sanitize("PRN"), "_");
        assert_eq!(sanitize("AUX"), "_");
        assert_eq!(sanitize("NUL"), "_");
        assert_eq!(sanitize("COM1"), "_");
        assert_eq!(sanitize("LPT1"), "_");
    }

    #[test]
    fn test_sanitize_trailing_dots_spaces() {
        assert_eq!(sanitize("filename."), "filename_");
        assert_eq!(sanitize("filename "), "filename_");
        assert_eq!(sanitize("filename..."), "filename_");
    }

    #[test]
    fn test_sanitize_reserved_names() {
        assert_eq!(sanitize("."), "_");
        assert_eq!(sanitize(".."), "_");
    }
}
