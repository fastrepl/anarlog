use futures_util::SinkExt;
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;

use super::types::UpstreamSender;

pub const MAX_PENDING_QUEUE_BYTES: usize = 5 * 1024 * 1024; // 5 MiB

#[derive(Debug)]
pub enum FlushError {
    SendFailed,
    InvalidUtf8,
}

#[derive(Debug, Clone)]
pub struct QueuedPayload {
    pub data: Vec<u8>,
    pub is_text: bool,
}

#[derive(Default)]
pub struct PendingState {
    control_messages: Vec<QueuedPayload>,
    data_messages: Vec<QueuedPayload>,
    bytes: usize,
}

impl PendingState {
    pub fn enqueue(
        &mut self,
        payload: QueuedPayload,
        is_control: bool,
    ) -> Result<(), &'static str> {
        let size = payload.data.len();
        if size > MAX_PENDING_QUEUE_BYTES {
            return Err("payload_too_large");
        }
        if self.bytes + size > MAX_PENDING_QUEUE_BYTES {
            return Err("backpressure_limit");
        }
        self.bytes += size;
        if is_control {
            self.control_messages.push(payload);
        } else {
            self.data_messages.push(payload);
        }
        Ok(())
    }

    pub async fn flush_to(&mut self, sender: &mut UpstreamSender) -> Result<(), FlushError> {
        // Take ownership of messages, but don't reset bytes yet
        let control = std::mem::take(&mut self.control_messages);
        let data = std::mem::take(&mut self.data_messages);
        let messages: Vec<_> = control.into_iter().chain(data).collect();

        for queued in messages {
            let msg = if queued.is_text {
                match String::from_utf8(queued.data) {
                    Ok(s) => TungsteniteMessage::Text(s.into()),
                    Err(e) => {
                        tracing::warn!(
                            error.message = ?e,
                            hyprnote.payload.invalid_bytes_len = %e.as_bytes().len(),
                            "invalid_utf8_in_text_message"
                        );
                        // Reset state since we failed
                        self.bytes = 0;
                        return Err(FlushError::InvalidUtf8);
                    }
                }
            } else {
                TungsteniteMessage::Binary(queued.data.into())
            };
            if sender.send(msg).await.is_err() {
                // Reset state since we failed (remaining messages are lost, but state is consistent)
                self.bytes = 0;
                return Err(FlushError::SendFailed);
            }
        }
        // Only reset bytes after successful flush
        self.bytes = 0;
        Ok(())
    }

    #[cfg(test)]
    pub fn total_bytes(&self) -> usize {
        self.bytes
    }

    #[cfg(test)]
    pub fn drain(&mut self) -> impl Iterator<Item = QueuedPayload> {
        self.bytes = 0;
        std::mem::take(&mut self.control_messages)
            .into_iter()
            .chain(std::mem::take(&mut self.data_messages))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enqueue_and_drain() {
        let mut state = PendingState::default();

        let payload1 = QueuedPayload {
            data: vec![1, 2, 3],
            is_text: false,
        };
        let payload2 = QueuedPayload {
            data: vec![4, 5],
            is_text: true,
        };

        assert!(state.enqueue(payload1, false).is_ok());
        assert!(state.enqueue(payload2, false).is_ok());
        assert_eq!(state.total_bytes(), 5);

        let drained: Vec<_> = state.drain().collect();
        assert_eq!(drained.len(), 2);
        assert_eq!(state.total_bytes(), 0);
    }

    #[test]
    fn test_control_messages_prioritized() {
        let mut state = PendingState::default();

        let data_payload = QueuedPayload {
            data: b"data".to_vec(),
            is_text: true,
        };
        let control_payload = QueuedPayload {
            data: b"control".to_vec(),
            is_text: true,
        };

        assert!(state.enqueue(data_payload, false).is_ok());
        assert!(state.enqueue(control_payload, true).is_ok());

        let drained: Vec<_> = state.drain().collect();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].data, b"control");
        assert_eq!(drained[1].data, b"data");
    }

    #[test]
    fn test_payload_too_large() {
        let mut state = PendingState::default();

        let large_payload = QueuedPayload {
            data: vec![0; MAX_PENDING_QUEUE_BYTES + 1],
            is_text: false,
        };

        assert_eq!(
            state.enqueue(large_payload, false),
            Err("payload_too_large")
        );
    }

    #[test]
    fn test_backpressure_limit() {
        let mut state = PendingState::default();

        let half_size = MAX_PENDING_QUEUE_BYTES / 2 + 1;
        let payload1 = QueuedPayload {
            data: vec![0; half_size],
            is_text: false,
        };
        let payload2 = QueuedPayload {
            data: vec![0; half_size],
            is_text: false,
        };

        assert!(state.enqueue(payload1, false).is_ok());
        assert_eq!(state.enqueue(payload2, false), Err("backpressure_limit"));
    }

    #[test]
    fn test_empty_payload() {
        let mut state = PendingState::default();

        let empty_payload = QueuedPayload {
            data: vec![],
            is_text: false,
        };

        assert!(state.enqueue(empty_payload, false).is_ok());
        assert_eq!(state.total_bytes(), 0);

        let drained: Vec<_> = state.drain().collect();
        assert_eq!(drained.len(), 1);
        assert!(drained[0].data.is_empty());
    }

    #[test]
    fn test_exact_limit_payload() {
        let mut state = PendingState::default();

        let exact_payload = QueuedPayload {
            data: vec![0; MAX_PENDING_QUEUE_BYTES],
            is_text: false,
        };

        assert!(state.enqueue(exact_payload, false).is_ok());
        assert_eq!(state.total_bytes(), MAX_PENDING_QUEUE_BYTES);
    }

    #[test]
    fn test_multiple_control_messages_order() {
        let mut state = PendingState::default();

        let control1 = QueuedPayload {
            data: b"control1".to_vec(),
            is_text: true,
        };
        let control2 = QueuedPayload {
            data: b"control2".to_vec(),
            is_text: true,
        };
        let data1 = QueuedPayload {
            data: b"data1".to_vec(),
            is_text: true,
        };

        assert!(state.enqueue(data1, false).is_ok());
        assert!(state.enqueue(control1, true).is_ok());
        assert!(state.enqueue(control2, true).is_ok());

        let drained: Vec<_> = state.drain().collect();
        assert_eq!(drained.len(), 3);
        assert_eq!(drained[0].data, b"control1");
        assert_eq!(drained[1].data, b"control2");
        assert_eq!(drained[2].data, b"data1");
    }

    #[test]
    fn test_drain_resets_state() {
        let mut state = PendingState::default();

        let payload = QueuedPayload {
            data: vec![1, 2, 3],
            is_text: false,
        };

        assert!(state.enqueue(payload.clone(), false).is_ok());
        assert_eq!(state.total_bytes(), 3);

        let _: Vec<_> = state.drain().collect();
        assert_eq!(state.total_bytes(), 0);

        assert!(state.enqueue(payload, false).is_ok());
        assert_eq!(state.total_bytes(), 3);
    }

    #[test]
    fn test_text_and_binary_mixed() {
        let mut state = PendingState::default();

        let text_payload = QueuedPayload {
            data: b"hello".to_vec(),
            is_text: true,
        };
        let binary_payload = QueuedPayload {
            data: vec![0x00, 0x01, 0x02],
            is_text: false,
        };

        assert!(state.enqueue(text_payload, false).is_ok());
        assert!(state.enqueue(binary_payload, false).is_ok());
        assert_eq!(state.total_bytes(), 8);

        let drained: Vec<_> = state.drain().collect();
        assert_eq!(drained.len(), 2);
        assert!(drained[0].is_text);
        assert!(!drained[1].is_text);
    }

    #[test]
    fn test_backpressure_after_partial_fill() {
        let mut state = PendingState::default();

        let small_payload = QueuedPayload {
            data: vec![0; 1000],
            is_text: false,
        };

        for _ in 0..(MAX_PENDING_QUEUE_BYTES / 1000) {
            assert!(state.enqueue(small_payload.clone(), false).is_ok());
        }

        let remaining = MAX_PENDING_QUEUE_BYTES % 1000;
        let final_payload = QueuedPayload {
            data: vec![0; remaining + 1],
            is_text: false,
        };
        assert_eq!(
            state.enqueue(final_payload, false),
            Err("backpressure_limit")
        );
    }

    #[test]
    fn test_default_state() {
        let state = PendingState::default();
        assert_eq!(state.total_bytes(), 0);
    }
}
