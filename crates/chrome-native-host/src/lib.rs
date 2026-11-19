//! Chrome native-messaging host for Anarlog's Meet extension: an authenticated proxy between the
//! extension's port (stdin/stdout) and the running app's socket. It holds no recording state. The
//! wire shapes match `anlg_transcription_core::browser_meeting`; both test against the same
//! fixture (`crates/transcription-core/src/browser_meeting/fixtures/wire.json`).

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;
/// Under the user's data directory; the app writes its endpoint record here.
pub const ENDPOINT_DIR_NAME: &str = "anarlog-browser-meeting";
/// Messages from Chrome; the extension's are small JSON objects.
const MAX_FROM_CHROME: usize = 256 * 1024;
/// Chrome refuses messages over 1 MiB from a native host.
const MAX_TO_CHROME: usize = 1024 * 1024;
/// The only messages forwarded to the app: the Meet bridge, nothing else.
const FORWARDED: &[&str] = &[
    "meeting_start",
    "meeting_pause",
    "meeting_resume",
    "meeting_stop",
    "meeting_detach",
    "meeting_cues",
    "meeting_chat",
    "meeting_camera",
];
const NO_APP: &str = "Open Anarlog, then click the extension again.";

#[derive(Debug, Clone, Deserialize)]
pub struct EndpointRecord {
    pub protocol: u32,
    pub app: String,
    pub pid: u32,
    pub socket: String,
    pub secret: String,
    pub allowed_origins: Vec<String>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename = "hello")]
struct Hello<'a> {
    protocol: u32,
    secret: &'a str,
    origin: &'a str,
    browser_pid: Option<u32>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum HelloReply {
    Welcome { protocol: u32 },
    Refused { message: String },
}

#[derive(Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
enum HostEvent<'a> {
    Connected { protocol: u32 },
    Error { message: &'a str },
}

/// The extension's origin, which Chrome passes as an argument, in the form
/// `chrome-extension://<32 letters a-p>/`.
pub fn extension_origin(args: impl IntoIterator<Item = String>) -> Option<String> {
    args.into_iter().find(|arg| {
        arg.strip_prefix("chrome-extension://")
            .and_then(|rest| rest.strip_suffix('/'))
            .is_some_and(|id| id.len() == 32 && id.bytes().all(|b| (b'a'..=b'p').contains(&b)))
    })
}

pub fn default_endpoint_dir() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join(ENDPOINT_DIR_NAME))
}

fn read_frame(reader: &mut impl Read, max: usize, chrome: bool) -> io::Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    match reader.read_exact(&mut len) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error),
    }
    // Chrome frames in native byte order; the app's socket in little-endian.
    let len = if chrome {
        u32::from_ne_bytes(len)
    } else {
        u32::from_le_bytes(len)
    } as usize;
    if len > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "message too large",
        ));
    }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;
    Ok(Some(body))
}

fn write_frame(writer: &mut impl Write, body: &[u8], chrome: bool) -> io::Result<()> {
    let len = body.len() as u32;
    writer.write_all(&if chrome {
        len.to_ne_bytes()
    } else {
        len.to_le_bytes()
    })?;
    writer.write_all(body)?;
    writer.flush()
}

fn tell_chrome(stdout: &mut impl Write, event: &HostEvent) -> io::Result<()> {
    write_frame(stdout, &serde_json::to_vec(event)?, true)
}

/// Whether a Chrome message is one of the Meet bridge's; anything else stays here.
fn forwardable(body: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(body).is_ok_and(|message| {
        message
            .get("type")
            .and_then(|kind| kind.as_str())
            .is_some_and(|kind| FORWARDED.contains(&kind))
    })
}

/// Running apps' records that allow this extension, newest first. A record another user could
/// have written is skipped.
pub fn endpoints(dir: &Path, origin: &str) -> Vec<EndpointRecord> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut found: Vec<(std::time::SystemTime, EndpointRecord)> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|entry| {
            let metadata = std::fs::symlink_metadata(entry.path()).ok()?;
            if !metadata.is_file() || !private(&metadata) {
                return None;
            }
            let record: EndpointRecord =
                serde_json::from_slice(&std::fs::read(entry.path()).ok()?).ok()?;
            (record.protocol == PROTOCOL_VERSION
                && record.allowed_origins.iter().any(|allowed| allowed == origin))
            .then(|| (metadata.modified().unwrap_or(std::time::UNIX_EPOCH), record))
        })
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found.into_iter().map(|(_, record)| record).collect()
}

#[cfg(unix)]
fn private(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o077 == 0
}

#[cfg(not(unix))]
fn private(_metadata: &std::fs::Metadata) -> bool {
    true
}

#[cfg(unix)]
type Socket = std::os::unix::net::UnixStream;

/// Connects to the first record whose app answers, presenting its secret. A refusal ends the
/// search with the app's reason; a record whose app is gone is skipped.
#[cfg(unix)]
fn connect(records: &[EndpointRecord], origin: &str, browser_pid: Option<u32>) -> Result<Socket, String> {
    use std::time::Duration;
    for record in records {
        let Ok(mut socket) = Socket::connect(&record.socket) else {
            continue;
        };
        let hello = Hello {
            protocol: PROTOCOL_VERSION,
            secret: &record.secret,
            origin,
            browser_pid,
        };
        let Ok(body) = serde_json::to_vec(&hello) else {
            continue;
        };
        if socket.set_read_timeout(Some(Duration::from_secs(5))).is_err()
            || write_frame(&mut socket, &body, false).is_err()
        {
            continue;
        }
        let reply = read_frame(&mut socket, MAX_TO_CHROME, false)
            .ok()
            .flatten()
            .and_then(|body| serde_json::from_slice::<HelloReply>(&body).ok());
        match reply {
            Some(HelloReply::Welcome { protocol }) if protocol == PROTOCOL_VERSION => {
                if socket.set_read_timeout(None).is_err() {
                    continue;
                }
                return Ok(socket);
            }
            Some(HelloReply::Refused { message }) => return Err(message),
            _ => return Err("Update Anarlog and the extension together.".into()),
        }
    }
    Err(NO_APP.into())
}

/// Proxies one extension port to the app until either side closes. Returns once the app's side is
/// closed; when Chrome closes first, the app hears it and lets go of the tab's recording.
#[cfg(unix)]
pub fn run(
    stdin: impl Read + Send + 'static,
    stdout: &mut impl Write,
    dir: &Path,
    origin: &str,
    browser_pid: Option<u32>,
) -> io::Result<()> {
    let records = endpoints(dir, origin);
    let socket = match connect(&records, origin, browser_pid) {
        Ok(socket) => socket,
        Err(message) => return tell_chrome(stdout, &HostEvent::Error { message: &message }),
    };
    tell_chrome(
        stdout,
        &HostEvent::Connected {
            protocol: PROTOCOL_VERSION,
        },
    )?;
    let mut to_app = socket.try_clone()?;
    std::thread::spawn(move || {
        let mut stdin = stdin;
        while let Ok(Some(body)) = read_frame(&mut stdin, MAX_FROM_CHROME, true) {
            if forwardable(&body) && write_frame(&mut to_app, &body, false).is_err() {
                break;
            }
        }
        // Chrome let go: the app reads the end and decides whether the recording stops.
        let _ = to_app.shutdown(std::net::Shutdown::Write);
    });
    let mut from_app = socket;
    while let Some(body) = read_frame(&mut from_app, MAX_TO_CHROME, false)? {
        if serde_json::from_slice::<serde_json::Value>(&body).is_ok() {
            write_frame(stdout, &body, true)?;
        }
    }
    Ok(())
}

#[cfg(not(unix))]
pub fn run(
    _stdin: impl Read + Send + 'static,
    stdout: &mut impl Write,
    _dir: &Path,
    _origin: &str,
    _browser_pid: Option<u32>,
) -> io::Result<()> {
    tell_chrome(
        stdout,
        &HostEvent::Error {
            message: "Recording Meet from the browser isn't available on this system yet.",
        },
    )
}

#[cfg(all(test, unix))]
mod tests {
    use std::io::Cursor;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;

    use serde_json::{Value, json};

    use super::*;

    const ORIGIN: &str = "chrome-extension://abcdefghijklmnopabcdefghijklmnop/";

    fn chrome_frames(messages: &[Value]) -> Vec<u8> {
        let mut input = Vec::new();
        for message in messages {
            write_frame(&mut input, &serde_json::to_vec(message).unwrap(), true).unwrap();
        }
        input
    }

    fn read_chrome(output: &[u8]) -> Vec<Value> {
        let mut cursor = Cursor::new(output);
        let mut messages = Vec::new();
        while let Some(body) = read_frame(&mut cursor, MAX_TO_CHROME, true).unwrap() {
            messages.push(serde_json::from_slice(&body).unwrap());
        }
        messages
    }

    fn record(dir: &Path, name: &str, socket: &Path, secret: &str, origin: &str) {
        let path = dir.join(format!("{name}.json"));
        let record = json!({
            "protocol": 1, "app": name, "pid": 1, "socket": socket, "secret": secret,
            "allowed_origins": [origin],
        });
        std::fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    /// The app's side as `browser_meeting::serve_connection` speaks it.
    fn fake_app(listener: UnixListener, reply: Value) -> std::thread::JoinHandle<Vec<Value>> {
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut seen = Vec::new();
            let hello = read_frame(&mut stream, MAX_FROM_CHROME, false).unwrap().unwrap();
            seen.push(serde_json::from_slice::<Value>(&hello).unwrap());
            write_frame(&mut stream, &serde_json::to_vec(&reply).unwrap(), false).unwrap();
            if reply["type"] != "welcome" {
                return seen;
            }
            while let Some(body) = read_frame(&mut stream, MAX_FROM_CHROME, false).unwrap() {
                let message: Value = serde_json::from_slice(&body).unwrap();
                if message["type"] == "meeting_start" {
                    let started = json!({"event": "started", "pausable": false});
                    write_frame(&mut stream, &serde_json::to_vec(&started).unwrap(), false)
                        .unwrap();
                }
                seen.push(message);
            }
            seen
        })
    }

    #[test]
    fn proxies_only_meet_messages_to_the_app_that_admitted_this_extension() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("s.sock");
        let app = fake_app(
            UnixListener::bind(&socket).unwrap(),
            json!({"type": "welcome", "protocol": 1}),
        );
        record(dir.path(), "com.hyprnote.dev", &socket, "secret-1", ORIGIN);
        // A gone app's record and another extension's record are skipped.
        record(dir.path(), "old", &dir.path().join("gone.sock"), "x", ORIGIN);
        record(dir.path(), "other", &socket, "x", "chrome-extension://pppppppppppppppppppppppppppppppp/");
        let input = chrome_frames(&[
            json!({"type": "meeting_start", "url": "https://meet.google.com/abc-defg-hij"}),
            json!({"type": "save_link", "url": "https://example.com"}),
            json!({"type": "meeting_detach"}),
        ]);
        let mut output = Vec::new();
        run(Cursor::new(input), &mut output, dir.path(), ORIGIN, Some(77)).unwrap();

        assert_eq!(
            read_chrome(&output),
            vec![
                json!({"event": "connected", "protocol": 1}),
                json!({"event": "started", "pausable": false}),
            ]
        );
        let seen = app.join().unwrap();
        assert_eq!(
            seen[0],
            json!({"type": "hello", "protocol": 1, "secret": "secret-1", "origin": ORIGIN, "browser_pid": 77})
        );
        let types: Vec<&str> = seen[1..].iter().map(|m| m["type"].as_str().unwrap()).collect();
        assert_eq!(types, ["meeting_start", "meeting_detach"]);
    }

    #[test]
    fn tells_the_extension_when_no_app_answers_or_the_app_refuses() {
        let dir = tempfile::tempdir().unwrap();
        let mut output = Vec::new();
        run(Cursor::new(vec![]), &mut output, dir.path(), ORIGIN, Some(77)).unwrap();
        assert_eq!(read_chrome(&output), vec![json!({"event": "error", "message": NO_APP})]);

        let socket = dir.path().join("s.sock");
        let app = fake_app(
            UnixListener::bind(&socket).unwrap(),
            json!({"type": "refused", "message": "Update Anarlog and the extension together."}),
        );
        record(dir.path(), "com.hyprnote.dev", &socket, "secret-1", ORIGIN);
        let mut output = Vec::new();
        run(Cursor::new(vec![]), &mut output, dir.path(), ORIGIN, Some(77)).unwrap();
        app.join().unwrap();
        assert_eq!(
            read_chrome(&output),
            vec![json!({"event": "error", "message": "Update Anarlog and the extension together."})]
        );

        // A record another user could have written is never trusted.
        let path = dir.path().join("com.hyprnote.dev.json");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(endpoints(dir.path(), ORIGIN).is_empty());
    }

    #[test]
    fn reads_only_chrome_extension_origins_and_bounded_frames() {
        assert_eq!(
            extension_origin(["host".into(), ORIGIN.into(), "--parent-window=0".into()]),
            Some(ORIGIN.into())
        );
        for other in [
            "chrome-extension://abc/",
            "chrome-extension://ABCDEFGHIJKLMNOPABCDEFGHIJKLMNOP/",
            "moz-extension://abcdefghijklmnopabcdefghijklmnop/",
            "chrome-extension://abcdefghijklmnopabcdefghijklmnop",
        ] {
            assert_eq!(extension_origin([other.to_string()]), None, "{other}");
        }
        let oversized = ((MAX_FROM_CHROME + 1) as u32).to_ne_bytes().to_vec();
        assert!(read_frame(&mut Cursor::new(oversized), MAX_FROM_CHROME, true).is_err());
        let mut truncated = 10u32.to_ne_bytes().to_vec();
        truncated.extend_from_slice(b"abc");
        assert!(read_frame(&mut Cursor::new(truncated), MAX_FROM_CHROME, true).is_err());
    }

    /// The app's module and this host read the same fixture, so neither drifts alone.
    #[test]
    fn speaks_the_shared_wire_fixture() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../transcription-core/src/browser_meeting/fixtures/wire.json"
        ))
        .unwrap();
        let hello = &fixture["hello"];
        let ours = Hello {
            protocol: hello["protocol"].as_u64().unwrap() as u32,
            secret: hello["secret"].as_str().unwrap(),
            origin: hello["origin"].as_str().unwrap(),
            browser_pid: hello["browser_pid"].as_u64().map(|pid| pid as u32),
        };
        assert_eq!(&serde_json::to_value(&ours).unwrap(), hello);
        assert!(matches!(
            serde_json::from_value(fixture["welcome"].clone()).unwrap(),
            HelloReply::Welcome { protocol: PROTOCOL_VERSION }
        ));
        serde_json::from_value::<EndpointRecord>(fixture["endpoint_record"].clone()).unwrap();
        for message in fixture["extension_messages"].as_array().unwrap() {
            assert!(forwardable(&serde_json::to_vec(message).unwrap()), "{message}");
        }
    }
}
