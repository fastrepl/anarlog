//! The helper's stdout: `kind: u8`, `length: u32 LE`, `payload`. `E` is one event line
//! (`event=<name> key=value ...`, percent-encoded values); `P` is 48 kHz mono PCM for one track,
//! `track: u8`, `frame: u64 LE`, then f32 LE samples. See `tools/meeting-capture/README.md`.

use std::io::{self, Read};

pub(crate) const MAX_PAYLOAD: usize = 1 << 20;
const EVENT: u8 = b'E';
const PCM: u8 = b'P';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Track {
    Microphone,
    Remote,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Message {
    Event(EventLine),
    Pcm {
        track: Track,
        frame: u64,
        samples: Vec<f32>,
    },
}

/// `Ok(None)` at a clean end of stream between messages.
pub(crate) fn read_message(reader: &mut impl Read) -> io::Result<Option<Message>> {
    let mut header = [0u8; 5];
    match reader.read(&mut header[..1])? {
        0 => return Ok(None),
        _ => reader.read_exact(&mut header[1..])?,
    }
    let length = u32::from_le_bytes([header[1], header[2], header[3], header[4]]) as usize;
    if length > MAX_PAYLOAD {
        return Err(invalid("helper message too large"));
    }
    let mut payload = vec![0u8; length];
    reader.read_exact(&mut payload)?;
    match header[0] {
        EVENT => {
            let text = String::from_utf8(payload).map_err(|_| invalid("event is not UTF-8"))?;
            EventLine::parse(&text)
                .map(|line| Some(Message::Event(line)))
                .ok_or_else(|| invalid("malformed event line"))
        }
        PCM => {
            if payload.len() < 9 || !(payload.len() - 9).is_multiple_of(4) {
                return Err(invalid("malformed PCM message"));
            }
            let track = match payload[0] {
                0 => Track::Microphone,
                1 => Track::Remote,
                _ => return Err(invalid("unknown PCM track")),
            };
            let frame = u64::from_le_bytes(payload[1..9].try_into().unwrap());
            let samples = payload[9..]
                .chunks_exact(4)
                .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
                .collect::<Vec<_>>();
            if samples.iter().any(|sample| !sample.is_finite()) {
                return Err(invalid("non-finite PCM sample"));
            }
            Ok(Some(Message::Pcm {
                track,
                frame,
                samples,
            }))
        }
        _ => Err(invalid("unknown helper message kind")),
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EventLine {
    fields: Vec<(String, String)>,
}

impl EventLine {
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let fields = text
            .split(' ')
            .filter(|token| !token.is_empty())
            .map(|token| {
                let (key, value) = token.split_once('=')?;
                Some((key.to_string(), percent_decode(value)?))
            })
            .collect::<Option<Vec<_>>>()?;
        (fields.first()?.0 == "event").then_some(Self { fields })
    }

    pub(crate) fn event(&self) -> &str {
        &self.fields[0].1
    }

    pub(crate) fn get(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }

    pub(crate) fn fields(&self) -> impl Iterator<Item = (&str, &str)> {
        self.fields
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }

    pub(crate) fn u64(&self, key: &str) -> Option<u64> {
        self.get(key)?.parse().ok()
    }

    pub(crate) fn bool(&self, key: &str) -> Option<bool> {
        match self.get(key)? {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn encode(event: &str, fields: &[(&str, &str)]) -> String {
        std::iter::once(("event", event))
            .chain(fields.iter().copied())
            .map(|(key, value)| format!("{key}={}", percent_encode(value)))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = std::str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

#[cfg(test)]
pub(crate) fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-._~:/@+".contains(&byte) {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn encode_event(event: &str, fields: &[(&str, &str)]) -> Vec<u8> {
    frame(EVENT, EventLine::encode(event, fields).as_bytes())
}

#[cfg(test)]
pub(crate) fn encode_pcm(track: Track, frame_index: u64, samples: &[f32]) -> Vec<u8> {
    let mut payload = vec![match track {
        Track::Microphone => 0,
        Track::Remote => 1,
    }];
    payload.extend_from_slice(&frame_index.to_le_bytes());
    for sample in samples {
        payload.extend_from_slice(&sample.to_le_bytes());
    }
    frame(PCM, &payload)
}

#[cfg(test)]
fn frame(kind: u8, payload: &[u8]) -> Vec<u8> {
    let mut data = vec![kind];
    data.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    data.extend_from_slice(payload);
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    // The exact bytes the Swift helper writes for these values (WireTests in tools/meeting-capture).
    #[test]
    fn reads_the_helpers_event_lines_and_pcm() {
        let mut bytes = encode_event(
            "started",
            &[
                ("callUrl", "https://meet.google.com/abc-defg-hij"),
                ("microphoneName", "MacBook Pro Microphone (Built-in) = 100%"),
            ],
        );
        bytes.extend(encode_pcm(Track::Remote, 4_800, &[0.25, -0.5]));
        let mut reader = bytes.as_slice();
        let Some(Message::Event(line)) = read_message(&mut reader).unwrap() else {
            panic!("expected an event");
        };
        assert_eq!(line.event(), "started");
        assert_eq!(
            line.get("callUrl"),
            Some("https://meet.google.com/abc-defg-hij")
        );
        assert_eq!(
            line.get("microphoneName"),
            Some("MacBook Pro Microphone (Built-in) = 100%")
        );
        assert_eq!(
            read_message(&mut reader).unwrap(),
            Some(Message::Pcm {
                track: Track::Remote,
                frame: 4_800,
                samples: vec![0.25, -0.5],
            })
        );
        assert_eq!(read_message(&mut reader).unwrap(), None);
    }

    #[test]
    fn swift_encoding_of_reserved_characters_decodes() {
        let line =
            EventLine::parse("event=failed code=staging_invalid message=The%20call%27s%20app")
                .unwrap();
        assert_eq!(line.get("message"), Some("The call's app"));
        assert!(EventLine::parse("code=x event=failed").is_none());
        assert!(EventLine::parse("event=failed message=%2").is_none());
    }

    #[test]
    fn malformed_or_oversized_messages_are_errors_not_audio() {
        for bytes in [
            frame(b'X', b""),
            frame(PCM, &[0, 1, 2]),
            frame(PCM, &[7, 0, 0, 0, 0, 0, 0, 0, 0]),
            encode_pcm(Track::Microphone, 0, &[f32::NAN]),
            vec![EVENT, 0xff, 0xff, 0xff, 0x7f],
            frame(EVENT, b"no-event-key"),
        ] {
            assert!(read_message(&mut bytes.as_slice()).is_err(), "{bytes:?}");
        }
        // A message cut off by a crash is an error, never a short buffer.
        let cut = encode_pcm(Track::Microphone, 0, &[0.1, 0.2]);
        assert!(read_message(&mut &cut[..cut.len() - 2]).is_err());
    }
}
