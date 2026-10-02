use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const MAX_MESSAGE_SIZE: usize = 256 * 1024;
const MAX_URL_LENGTH: usize = 2048;
const MAX_PARTICIPANTS: usize = 30;
const MAX_PARTICIPANT_NAME_LENGTH: usize = 80;

#[derive(Debug, Deserialize)]
struct IncomingMessage {
    #[serde(rename = "type")]
    msg_type: String,
    url: Option<String>,
    is_active: Option<bool>,
    muted: Option<bool>,
    participants: Option<Vec<Participant>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Participant {
    pub name: String,
    pub is_self: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChromeState {
    version: u32,
    timestamp_ms: u64,
    meeting: Option<MeetingState>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct MeetingState {
    url: String,
    is_active: bool,
    muted: bool,
    participants: Vec<Participant>,
}

fn default_state_path() -> io::Result<PathBuf> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "data directory not found"))?;

    Ok(data_dir.join("char").join("chrome_state.json"))
}

fn read_message(reader: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("message too large: {len}"),
        ));
    }

    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    Ok(Some(buf))
}

#[derive(Debug, PartialEq)]
enum ProcessedMessage {
    Ignore,
    Update(Option<MeetingState>),
}

fn normalize_url(url: Option<String>) -> Option<String> {
    let value = url?.trim().to_owned();
    if value.is_empty() || value.len() > MAX_URL_LENGTH {
        return None;
    }

    if !value.starts_with("https://meet.google.com/") {
        return None;
    }

    Some(value)
}

fn normalize_participants(participants: Option<Vec<Participant>>) -> Vec<Participant> {
    participants
        .unwrap_or_default()
        .into_iter()
        .filter_map(|participant| {
            let name = participant.name.trim();
            if name.is_empty() || name.len() > MAX_PARTICIPANT_NAME_LENGTH {
                return None;
            }

            Some(Participant {
                name: name.to_owned(),
                is_self: participant.is_self,
            })
        })
        .take(MAX_PARTICIPANTS)
        .collect()
}

fn process_message(msg: IncomingMessage) -> ProcessedMessage {
    match msg.msg_type.as_str() {
        "meeting_state" => {
            if !msg.is_active.unwrap_or(false) {
                return ProcessedMessage::Update(None);
            }

            let Some(url) = normalize_url(msg.url) else {
                return ProcessedMessage::Ignore;
            };

            ProcessedMessage::Update(Some(MeetingState {
                url,
                is_active: true,
                muted: msg.muted.unwrap_or(false),
                participants: normalize_participants(msg.participants),
            }))
        }
        "meeting_ended" => {
            if msg.is_active == Some(true) {
                return ProcessedMessage::Ignore;
            }

            ProcessedMessage::Update(None)
        }
        _ => ProcessedMessage::Ignore,
    }
}

fn write_state(state: &ChromeState, path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let dir = path.parent().unwrap();
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    serde_json::to_writer(&mut tmp, state)?;
    tmp.as_file_mut().flush()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

fn timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn run(reader: &mut impl Read, state_path: &Path) {
    loop {
        match read_message(reader) {
            Ok(Some(data)) => {
                let msg: IncomingMessage = match serde_json::from_slice(&data) {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                let meeting = match process_message(msg) {
                    ProcessedMessage::Ignore => continue,
                    ProcessedMessage::Update(meeting) => meeting,
                };

                let state = ChromeState {
                    version: 1,
                    timestamp_ms: timestamp_ms(),
                    meeting,
                };

                if let Err(e) = write_state(&state, state_path) {
                    eprintln!("failed to write state: {e}");
                }
            }
            Ok(None) => break,
            Err(e) => {
                eprintln!("error reading message: {e}");
                break;
            }
        }
    }
}

fn main() {
    let mut stdin = io::stdin().lock();
    let state_path = match default_state_path() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("failed to resolve state path: {error}");
            return;
        }
    };

    run(&mut stdin, &state_path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn encode_message(json: &str) -> Vec<u8> {
        let bytes = json.as_bytes();
        let len = bytes.len() as u32;
        let mut buf = Vec::with_capacity(4 + bytes.len());
        buf.extend_from_slice(&len.to_le_bytes());
        buf.extend_from_slice(bytes);
        buf
    }

    fn incoming_message(
        msg_type: &str,
        url: Option<&str>,
        is_active: Option<bool>,
        muted: Option<bool>,
        participants: Option<Vec<Participant>>,
    ) -> IncomingMessage {
        IncomingMessage {
            msg_type: msg_type.to_string(),
            url: url.map(str::to_string),
            is_active,
            muted,
            participants,
        }
    }

    fn read_state_json(path: &Path) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    // --- read_message ---

    #[test]
    fn read_message_frames_and_bounds() {
        let payload = r#"{"type":"meeting_state"}"#;
        let encoded = encode_message(payload);
        let mut cursor = Cursor::new(encoded);
        assert_eq!(
            read_message(&mut cursor).unwrap(),
            Some(payload.as_bytes().to_vec())
        );
        assert_eq!(read_message(&mut Cursor::new(vec![])).unwrap(), None);

        // 4-byte header says 10 bytes but body has only 3.
        let mut truncated = 10u32.to_le_bytes().to_vec();
        truncated.extend_from_slice(b"abc");
        assert!(read_message(&mut Cursor::new(truncated)).is_err());

        let oversized = ((MAX_MESSAGE_SIZE + 1) as u32).to_le_bytes().to_vec();
        assert!(read_message(&mut Cursor::new(oversized)).is_err());
    }

    // --- process_message ---

    #[test]
    fn process_active_meeting_state() {
        let active = incoming_message(
            "meeting_state",
            Some("https://meet.google.com/abc"),
            Some(true),
            Some(true),
            Some(vec![Participant {
                name: "Alice".into(),
                is_self: true,
            }]),
        );
        match process_message(active) {
            ProcessedMessage::Update(Some(meeting)) => {
                assert!(meeting.is_active);
                assert!(meeting.muted);
                assert_eq!(meeting.url, "https://meet.google.com/abc");
                assert_eq!(
                    meeting.participants,
                    vec![Participant {
                        name: "Alice".into(),
                        is_self: true,
                    }]
                );
            }
            _ => panic!("expected active meeting state"),
        }

        let defaults_muted_and_sanitizes_participants = incoming_message(
            "meeting_state",
            Some("https://meet.google.com/abc"),
            Some(true),
            None,
            Some(vec![
                Participant {
                    name: "  Alice  ".into(),
                    is_self: false,
                },
                Participant {
                    name: " ".into(),
                    is_self: false,
                },
            ]),
        );
        match process_message(defaults_muted_and_sanitizes_participants) {
            ProcessedMessage::Update(Some(meeting)) => {
                assert!(!meeting.muted);
                assert_eq!(
                    meeting.participants,
                    vec![Participant {
                        name: "Alice".into(),
                        is_self: false,
                    }]
                );
            }
            _ => panic!("expected active meeting state"),
        }
    }

    #[test]
    fn process_non_active_messages() {
        for (msg_type, url, is_active, expected) in [
            (
                "meeting_ended",
                None,
                Some(false),
                ProcessedMessage::Update(None),
            ),
            (
                "meeting_state",
                Some("https://meet.google.com/abc"),
                Some(false),
                ProcessedMessage::Update(None),
            ),
            (
                "unknown",
                Some("https://meet.google.com/abc"),
                Some(true),
                ProcessedMessage::Ignore,
            ),
            (
                "meeting_state",
                Some("https://example.com/abc"),
                Some(true),
                ProcessedMessage::Ignore,
            ),
        ] {
            assert_eq!(
                process_message(incoming_message(
                    msg_type,
                    url,
                    is_active,
                    Some(false),
                    None
                )),
                expected
            );
        }
    }

    // --- write_state + full round-trip ---

    #[test]
    fn test_run_meeting_state_message() {
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir
            .path()
            .join("nested")
            .join("dirs")
            .join("chrome_state.json");

        let msg = r#"{"type":"meeting_state","url":"https://meet.google.com/xyz","is_active":true,"muted":false,"participants":[{"name":"Bob","is_self":false}]}"#;
        let input = encode_message(msg);
        let mut cursor = Cursor::new(input);

        run(&mut cursor, &state_path);

        let parsed = read_state_json(&state_path);
        assert_eq!(parsed["meeting"]["url"], "https://meet.google.com/xyz");
        assert_eq!(parsed["meeting"]["participants"][0]["name"], "Bob");
    }

    #[test]
    fn test_run_meeting_ended_clears_meeting() {
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("chrome_state.json");

        let active = r#"{"type":"meeting_state","is_active":true,"muted":false}"#;
        let ended = r#"{"type":"meeting_ended","is_active":false}"#;

        let mut input = encode_message(active);
        input.extend(encode_message(ended));

        let mut cursor = Cursor::new(input);
        run(&mut cursor, &state_path);

        let parsed = read_state_json(&state_path);
        assert!(parsed["meeting"].is_null());
    }

    #[test]
    fn test_run_invalid_json_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("chrome_state.json");

        let bad = b"not json at all";
        let len = bad.len() as u32;
        let mut input = len.to_le_bytes().to_vec();
        input.extend_from_slice(bad);

        let valid = r#"{"type":"meeting_state","url":"https://meet.google.com/xyz","is_active":true,"muted":true}"#;
        input.extend(encode_message(valid));

        let mut cursor = Cursor::new(input);
        run(&mut cursor, &state_path);

        // second (valid) message should still produce output
        assert!(state_path.exists());
    }

    #[test]
    fn test_run_unknown_type_does_not_clear_existing_state() {
        let dir = tempfile::tempdir().unwrap();
        let state_path = dir.path().join("chrome_state.json");

        let active = r#"{"type":"meeting_state","url":"https://meet.google.com/xyz","is_active":true,"muted":false,"participants":[]}"#;
        let unknown = r#"{"type":"something_else"}"#;

        let mut input = encode_message(active);
        input.extend(encode_message(unknown));

        let mut cursor = Cursor::new(input);
        run(&mut cursor, &state_path);

        let parsed = read_state_json(&state_path);
        assert_eq!(parsed["meeting"]["url"], "https://meet.google.com/xyz");
    }
}
