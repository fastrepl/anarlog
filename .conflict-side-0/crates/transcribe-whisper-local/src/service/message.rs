use axum::extract::ws::Message;
use hypr_audio_utils::{bytes_to_f32_samples, deinterleave_stereo_bytes};
use owhisper_interface::{ControlMessage, ListenInputChunk};

pub(super) enum IncomingMessage {
    Audio(AudioExtract),
    Control(ControlMessage),
}

pub(super) enum AudioExtract {
    Mono(Vec<f32>),
    Dual { ch0: Vec<f32>, ch1: Vec<f32> },
    Empty,
    End,
}

pub(super) fn process_incoming_message(
    msg: &Message,
    channels: u8,
) -> Result<IncomingMessage, crate::Error> {
    match msg {
        Message::Binary(data) => {
            if data.is_empty() {
                Ok(IncomingMessage::Audio(AudioExtract::Empty))
            } else if channels >= 2 {
                let (ch0, ch1) = deinterleave_stereo_bytes(data);
                Ok(IncomingMessage::Audio(AudioExtract::Dual { ch0, ch1 }))
            } else {
                Ok(IncomingMessage::Audio(AudioExtract::Mono(
                    bytes_to_f32_samples(data),
                )))
            }
        }
        Message::Text(data) => {
            if let Ok(ctrl) = serde_json::from_str::<ControlMessage>(data) {
                return Ok(IncomingMessage::Control(ctrl));
            }

            match serde_json::from_str::<ListenInputChunk>(data) {
                Ok(ListenInputChunk::Audio { data }) => {
                    if data.is_empty() {
                        Ok(IncomingMessage::Audio(AudioExtract::Empty))
                    } else {
                        Ok(IncomingMessage::Audio(AudioExtract::Mono(
                            bytes_to_f32_samples(&data),
                        )))
                    }
                }
                Ok(ListenInputChunk::DualAudio { mic, speaker }) => {
                    Ok(IncomingMessage::Audio(AudioExtract::Dual {
                        ch0: bytes_to_f32_samples(&mic),
                        ch1: bytes_to_f32_samples(&speaker),
                    }))
                }
                Ok(ListenInputChunk::End) => Ok(IncomingMessage::Audio(AudioExtract::End)),
                Err(_) => Err(crate::Error::unsupported_websocket_text_payload()),
            }
        }
        Message::Close(_) => Ok(IncomingMessage::Audio(AudioExtract::End)),
        Message::Ping(_) | Message::Pong(_) => Ok(IncomingMessage::Audio(AudioExtract::Empty)),
    }
}

#[cfg(test)]
mod tests {
    use owhisper_interface::ControlMessage;

    use super::*;

    #[test]
    fn control_message_finalize_parsed() {
        let msg = Message::Text(r#"{"type":"Finalize"}"#.into());
        match process_incoming_message(&msg, 1).unwrap() {
            IncomingMessage::Control(ControlMessage::Finalize) => {}
            other => panic!(
                "expected Finalize, got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }

    #[test]
    fn control_message_keep_alive_parsed() {
        let msg = Message::Text(r#"{"type":"KeepAlive"}"#.into());
        match process_incoming_message(&msg, 1).unwrap() {
            IncomingMessage::Control(ControlMessage::KeepAlive) => {}
            other => panic!(
                "expected KeepAlive, got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }

    #[test]
    fn control_message_close_stream_parsed() {
        let msg = Message::Text(r#"{"type":"CloseStream"}"#.into());
        match process_incoming_message(&msg, 1).unwrap() {
            IncomingMessage::Control(ControlMessage::CloseStream) => {}
            other => panic!(
                "expected CloseStream, got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }

    #[test]
    fn invalid_text_payload_returns_protocol_error() {
        let msg = Message::Text(r#"{"type":"Nope"}"#.into());
        let error = process_incoming_message(&msg, 1)
            .err()
            .expect("expected protocol error");
        assert_eq!(error.to_string(), "unsupported websocket text payload");
    }
}
