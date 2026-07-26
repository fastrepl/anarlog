use std::io::{self, Write};
use std::sync::LazyLock;

use regex::Regex;

static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").expect("Invalid regex")
});

static IP_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").expect("Invalid regex"));

pub struct RedactingWriter<W: Write> {
    inner: W,
    buffer: Vec<u8>,
    home_dir: Option<String>,
}

impl<W: Write> RedactingWriter<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            buffer: Vec::with_capacity(8192),
            home_dir: dirs::home_dir().map(|p| p.to_string_lossy().into_owned()),
        }
    }

    #[cfg(test)]
    fn with_home_dir(inner: W, home_dir: Option<String>) -> Self {
        Self {
            inner,
            buffer: Vec::with_capacity(8192),
            home_dir,
        }
    }

    fn redact_line(&self, line: &str) -> String {
        let mut redacted = line.to_string();
        if let Some(home) = &self.home_dir {
            redacted = redacted.replace(home, "[HOME]");
        }
        redacted = EMAIL_REGEX
            .replace_all(&redacted, "[EMAIL_REDACTED]")
            .into_owned();
        redacted = IP_REGEX
            .replace_all(&redacted, "[IP_REDACTED]")
            .into_owned();
        redacted
    }

    fn flush_buffer(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let line = String::from_utf8_lossy(&self.buffer);
        let redacted = self.redact_line(&line);
        self.inner.write_all(redacted.as_bytes())?;

        self.buffer.clear();
        Ok(())
    }
}

impl<W: Write> Write for RedactingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut last_newline_pos = 0;

        for (i, &byte) in buf.iter().enumerate() {
            if byte == b'\n' {
                self.buffer.extend_from_slice(&buf[last_newline_pos..=i]);
                self.flush_buffer()?;
                last_newline_pos = i + 1;
            }
        }

        if last_newline_pos < buf.len() {
            self.buffer.extend_from_slice(&buf[last_newline_pos..]);
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_buffer()?;
        self.inner.flush()
    }
}

impl<W: Write> Drop for RedactingWriter<W> {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn assert_redaction(home: Option<&str>, input: &str, expected: &str) {
        let mut output = Vec::new();
        {
            let mut writer = RedactingWriter::with_home_dir(&mut output, home.map(String::from));
            writeln!(writer, "{}", input).unwrap();
            writer.flush().unwrap();
        }
        let result = String::from_utf8(output).unwrap();
        assert_eq!(result.trim(), expected, "input: {}", input);
    }

    macro_rules! redact_test {
        ($name:ident, home = $home:expr, $input:literal => $expected:literal) => {
            #[test]
            fn $name() {
                assert_redaction($home, $input, $expected);
            }
        };
    }

    redact_test!(redact_home_linux, home = Some("/home/johndoe"), "/home/johndoe/documents/file.txt" => "[HOME]/documents/file.txt");
    redact_test!(redact_home_linux_multiple, home = Some("/home/alice"), "/home/alice/file and /home/alice/other" => "[HOME]/file and [HOME]/other");
    redact_test!(redact_home_macos, home = Some("/Users/janedoe"), "/Users/janedoe/projects/app" => "[HOME]/projects/app");
    redact_test!(redact_home_windows, home = Some(r"C:\Users\johndoe"), r"C:\Users\johndoe\Desktop\file.txt" => r"[HOME]\Desktop\file.txt");
    redact_test!(redact_other_user_paths_preserved, home = Some("/home/alice"), "/home/bob/documents/file.txt" => "/home/bob/documents/file.txt");
    redact_test!(redact_email_single, home = None, "Contact: user@example.com for help" => "Contact: [EMAIL_REDACTED] for help");
    redact_test!(redact_email_multiple, home = None, "From alice@test.org to bob@example.com" => "From [EMAIL_REDACTED] to [EMAIL_REDACTED]");
    redact_test!(redact_email_complex, home = None, "Email: john.doe+tag@sub.example.co.uk" => "Email: [EMAIL_REDACTED]");
    redact_test!(redact_ip_single, home = None, "Connected to 192.168.1.1 successfully" => "Connected to [IP_REDACTED] successfully");
    redact_test!(redact_ip_multiple, home = None, "From 10.0.0.1 to 192.168.0.100" => "From [IP_REDACTED] to [IP_REDACTED]");
    redact_test!(redact_ip_localhost, home = None, "Listening on 127.0.0.1:8080" => "Listening on [IP_REDACTED]:8080");
    redact_test!(redact_mixed_content, home = Some("/home/alice"), "User alice@test.com at /home/alice connected from 192.168.1.50" => "User [EMAIL_REDACTED] at [HOME] connected from [IP_REDACTED]");
    redact_test!(redact_no_sensitive_data, home = None, "Application started successfully" => "Application started successfully");

    #[test]
    fn writer_buffers_partial_lines() {
        let mut output = Vec::new();
        {
            let mut writer = RedactingWriter::with_home_dir(&mut output, Some("/home/user".into()));
            writer.write_all(b"test /home/").unwrap();
            writer.write_all(b"user/file\n").unwrap();
            writer.flush().unwrap();
        }
        let result = String::from_utf8(output).unwrap();
        assert_eq!(result, "test [HOME]/file\n");
    }

    #[test]
    fn writer_handles_empty_input() {
        let mut output = Vec::new();
        {
            let mut writer = RedactingWriter::new(&mut output);
            writer.write_all(b"").unwrap();
            writer.flush().unwrap();
        }
        assert_eq!(String::from_utf8(output).unwrap(), "");
    }

    #[test]
    fn writer_handles_only_newlines() {
        let mut output = Vec::new();
        {
            let mut writer = RedactingWriter::new(&mut output);
            writer.write_all(b"\n\n\n").unwrap();
            writer.flush().unwrap();
        }
        assert_eq!(String::from_utf8(output).unwrap(), "\n\n\n");
    }

    #[test]
    fn writer_handles_interleaved_writes() {
        let mut output = Vec::new();
        {
            let mut writer = RedactingWriter::new(&mut output);
            writer.write_all(b"line1 ").unwrap();
            writer.write_all(b"user@test.com").unwrap();
            writer.write_all(b" end\n").unwrap();
            writer.write_all(b"line2\n").unwrap();
            writer.flush().unwrap();
        }
        let result = String::from_utf8(output).unwrap();
        assert_eq!(result, "line1 [EMAIL_REDACTED] end\nline2\n");
    }

    #[test]
    fn multiline_redaction() {
        let mut output = Vec::new();
        {
            let mut writer =
                RedactingWriter::with_home_dir(&mut output, Some("/home/testuser".into()));
            writeln!(writer, "User logged in from /home/testuser/app").unwrap();
            writeln!(writer, "Email: user@example.com").unwrap();
            writeln!(writer, "Connection from 192.168.1.100").unwrap();
            writer.flush().unwrap();
        }
        let content = String::from_utf8(output).unwrap();
        assert!(content.contains("[HOME]/app"));
        assert!(content.contains("[EMAIL_REDACTED]"));
        assert!(content.contains("[IP_REDACTED]"));
        assert!(!content.contains("/home/testuser"));
        assert!(!content.contains("user@example.com"));
        assert!(!content.contains("192.168.1.100"));
    }
}
