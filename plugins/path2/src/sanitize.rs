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
    fn sanitize_replaces_unsafe_filename_parts() {
        for (input, expected) in [
            ("valid-filename", "valid-filename"),
            ("file<name", "file_name"),
            ("file>name", "file_name"),
            ("file:name", "file_name"),
            ("file/name", "file_name"),
            ("file\\name", "file_name"),
            ("file|name", "file_name"),
            ("file?name", "file_name"),
            ("file*name", "file_name"),
            ("CON", "_"),
            ("PRN", "_"),
            ("AUX", "_"),
            ("NUL", "_"),
            ("COM1", "_"),
            ("LPT1", "_"),
            ("filename.", "filename_"),
            ("filename ", "filename_"),
            ("filename...", "filename_"),
            (".", "_"),
            ("..", "_"),
        ] {
            assert_eq!(sanitize(input), expected, "unexpected result for {input:?}");
        }
    }
}
