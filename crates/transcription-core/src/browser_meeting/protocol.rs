use serde::{Deserialize, Serialize};

/// Bumped on any incompatible change to the hello or the messages below.
pub const PROTOCOL_VERSION: u32 = 1;
/// A frame is a 4-byte little-endian length and that many bytes of UTF-8 JSON.
pub const MAX_FRAME_BYTES: usize = 256 * 1024;

/// What the extension sends through the native host. Any other `type` is ignored.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExtensionMessage {
    MeetingStart {
        url: String,
    },
    MeetingPause,
    MeetingResume,
    MeetingStop,
    /// The tab is gone, navigated, unreadable, or showed the call ending.
    MeetingDetach,
    MeetingCues {
        events: Vec<serde_json::Value>,
    },
    MeetingChat {
        outcome: String,
    },
    MeetingCamera {
        outcome: String,
    },
}

/// What the app tells the extension. `started` carries the notice and camera to act on, once.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ExtensionEvent {
    Started {
        pausable: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        notice: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        camera: Option<String>,
    },
    Paused,
    Resumed,
    /// The recording is over and saved.
    Ended,
    /// The tab let go; an app recording keeps going in Anarlog.
    Detached,
    /// The link is over.
    Error {
        message: String,
    },
    /// A command was refused; the link and the recording stay as they were.
    Refused {
        message: String,
    },
}

/// The native host's first frame on the app's socket.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename = "hello")]
pub struct Hello {
    pub protocol: u32,
    pub secret: String,
    /// The `chrome-extension://<id>/` origin Chrome launched the host for.
    pub origin: String,
    /// The host's parent process: the browser that launched it.
    pub browser_pid: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HelloReply {
    Welcome { protocol: u32 },
    Refused { message: String },
}

/// The record a running app writes beside its socket so the native host can find it. Readable only
/// by this user (0600 in a 0700 directory); the secret changes on every launch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EndpointRecord {
    pub protocol: u32,
    pub app: String,
    pub pid: u32,
    pub socket: String,
    pub secret: String,
    pub allowed_origins: Vec<String>,
}

pub fn encode_frame(value: &impl Serialize) -> serde_json::Result<Vec<u8>> {
    let body = serde_json::to_vec(value)?;
    let mut frame = Vec::with_capacity(body.len() + 4);
    frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
    frame.extend_from_slice(&body);
    Ok(frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The native host and the extension speak this JSON; a change here breaks installed copies.
    #[test]
    fn keeps_the_wire_shapes_the_extension_and_host_use() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/wire.json")).unwrap();
        for message in fixture["extension_messages"].as_array().unwrap() {
            serde_json::from_value::<ExtensionMessage>(message.clone())
                .unwrap_or_else(|error| panic!("{message}: {error}"));
        }
        assert!(
            serde_json::from_value::<ExtensionMessage>(serde_json::json!({"type": "save_link"}))
                .is_err()
        );
        let events = [
            ExtensionEvent::Started {
                pausable: false,
                notice: Some("Anarlog is taking notes.".into()),
                camera: None,
            },
            ExtensionEvent::Paused,
            ExtensionEvent::Detached,
            ExtensionEvent::Refused {
                message: "x".into(),
            },
        ];
        assert_eq!(
            serde_json::to_value(events).unwrap(),
            fixture["extension_events"]
        );
        let hello: Hello = serde_json::from_value(fixture["hello"].clone()).unwrap();
        assert_eq!(serde_json::to_value(&hello).unwrap(), fixture["hello"]);
        let record: EndpointRecord =
            serde_json::from_value(fixture["endpoint_record"].clone()).unwrap();
        assert_eq!(record.protocol, PROTOCOL_VERSION);
        assert_eq!(
            serde_json::to_value(HelloReply::Welcome { protocol: 1 }).unwrap(),
            fixture["welcome"]
        );
    }
}
