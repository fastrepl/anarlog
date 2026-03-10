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
    use axum::extract::ws::Message;
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
    fn audio_chunk_parsed_over_control() {
        let chunk = owhisper_interface::ListenInputChunk::End;
        let json = serde_json::to_string(&chunk).unwrap();
        let msg = Message::Text(json.into());
        match process_incoming_message(&msg, 1).unwrap() {
            IncomingMessage::Audio(AudioExtract::End) => {}
            other => panic!(
                "expected Audio(End), got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }

    #[test]
    fn close_frame_yields_end() {
        let msg = Message::Close(None);
        match process_incoming_message(&msg, 1).unwrap() {
            IncomingMessage::Audio(AudioExtract::End) => {}
            other => panic!(
                "expected Audio(End), got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }

    #[test]
    fn binary_single_channel_yields_mono() {
        let samples: Vec<i16> = vec![1000, 2000, 3000];
        let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let msg = Message::Binary(data.into());
        match process_incoming_message(&msg, 1).unwrap() {
            IncomingMessage::Audio(AudioExtract::Mono(s)) => assert!(!s.is_empty()),
            other => panic!(
                "expected Audio(Mono), got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }

    #[test]
    fn binary_dual_channel_yields_dual() {
        let samples: Vec<i16> = vec![1000, -1000, 2000, -2000];
        let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let msg = Message::Binary(data.into());
        match process_incoming_message(&msg, 2).unwrap() {
            IncomingMessage::Audio(AudioExtract::Dual { ch0, ch1 }) => {
                assert_eq!(ch0.len(), 2);
                assert_eq!(ch1.len(), 2);
                assert!(ch0[0] > 0.0);
                assert!(ch1[0] < 0.0);
            }
            other => panic!(
                "expected Audio(Dual), got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }

    #[test]
    fn dual_audio_json_yields_dual() {
        let chunk = owhisper_interface::ListenInputChunk::DualAudio {
            mic: vec![0x00, 0x10],
            speaker: vec![0x00, 0x20],
        };
        let json = serde_json::to_string(&chunk).unwrap();
        let msg = Message::Text(json.into());
        match process_incoming_message(&msg, 1).unwrap() {
            IncomingMessage::Audio(AudioExtract::Dual { .. }) => {}
            other => panic!(
                "expected Audio(Dual), got {:?}",
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
