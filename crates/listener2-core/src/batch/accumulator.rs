use std::collections::BTreeMap;

use owhisper_interface::stream::StreamResponse;

use super::{BatchRunMode, BatchRunOutput};

#[derive(Default)]
pub(super) struct StreamBatchAccumulator {
    channels: BTreeMap<i32, StreamBatchChannel>,
    max_duration_secs: f64,
    terminal_duration_secs: Option<f64>,
    terminal_channels: Option<u32>,
}

#[derive(Default)]
struct StreamBatchChannel {
    words: Vec<owhisper_interface::batch::Word>,
    transcript_segments: Vec<String>,
    final_transcript: Option<String>,
    confidence_sum: f64,
    confidence_count: usize,
}

impl StreamBatchAccumulator {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn observe(&mut self, response: &StreamResponse) {
        match response {
            StreamResponse::TranscriptResponse {
                start,
                duration,
                from_finalize,
                channel,
                channel_index,
                ..
            } => {
                self.max_duration_secs = self.max_duration_secs.max((*start + *duration).max(0.0));

                let channel_id = channel_index.first().copied().unwrap_or(0);
                let Some(alternative) = channel.alternatives.first() else {
                    return;
                };

                let state = self.channels.entry(channel_id).or_default();
                let transcript = alternative.transcript.trim();

                if !alternative.words.is_empty() {
                    let words = alternative
                        .words
                        .iter()
                        .cloned()
                        .map(owhisper_interface::batch::Word::from)
                        .collect::<Vec<_>>();

                    let should_replace = *from_finalize
                        && *start <= 0.0
                        && words.last().map(|word| word.end).unwrap_or_default()
                            >= state.words.last().map(|word| word.end).unwrap_or_default();

                    if should_replace {
                        state.words = words;
                    } else {
                        append_non_overlapping_words(&mut state.words, words);
                    }
                }

                if !transcript.is_empty() {
                    if *from_finalize {
                        state.final_transcript = Some(transcript.to_string());
                    } else if state
                        .transcript_segments
                        .last()
                        .is_none_or(|existing| existing != transcript)
                    {
                        state.transcript_segments.push(transcript.to_string());
                    }
                }

                if alternative.confidence.is_finite() {
                    state.confidence_sum += alternative.confidence;
                    state.confidence_count += 1;
                }
            }
            StreamResponse::TerminalResponse {
                duration, channels, ..
            } => {
                self.terminal_duration_secs = Some(*duration);
                self.terminal_channels = Some(*channels);
            }
            _ => {}
        }
    }

    pub(super) fn finish(self, session_id: &str) -> BatchRunOutput {
        let channel_count = self
            .terminal_channels
            .map(|count| count as usize)
            .unwrap_or_else(|| {
                self.channels
                    .keys()
                    .max()
                    .map(|idx| (*idx as usize).saturating_add(1))
                    .unwrap_or(1)
            });

        let mut channels = Vec::with_capacity(channel_count);
        for idx in 0..channel_count {
            let entry = self.channels.get(&(idx as i32));

            let transcript = entry
                .and_then(|channel| channel.final_transcript.clone())
                .unwrap_or_else(|| {
                    entry
                        .map(StreamBatchChannel::joined_transcript)
                        .unwrap_or_default()
                });

            let confidence = entry
                .map(StreamBatchChannel::average_confidence)
                .unwrap_or_default();

            let words = entry
                .map(|channel| channel.words.clone())
                .unwrap_or_default();

            channels.push(owhisper_interface::batch::Channel {
                alternatives: vec![owhisper_interface::batch::Alternatives {
                    transcript,
                    confidence,
                    words,
                }],
            });
        }

        let duration = self
            .terminal_duration_secs
            .unwrap_or(self.max_duration_secs.max(0.0));

        BatchRunOutput {
            session_id: session_id.to_string(),
            mode: BatchRunMode::Streamed,
            response: owhisper_interface::batch::Response {
                metadata: serde_json::json!({ "duration": duration }),
                results: owhisper_interface::batch::Results { channels },
            },
        }
    }
}

impl StreamBatchChannel {
    fn joined_transcript(&self) -> String {
        self.transcript_segments.join(" ")
    }

    fn average_confidence(&self) -> f64 {
        if self.confidence_count == 0 {
            0.0
        } else {
            self.confidence_sum / self.confidence_count as f64
        }
    }
}

fn append_non_overlapping_words(
    existing: &mut Vec<owhisper_interface::batch::Word>,
    incoming: Vec<owhisper_interface::batch::Word>,
) {
    for word in incoming {
        let is_duplicate = existing.iter().any(|current| {
            current.start == word.start
                && current.end == word.end
                && current.word == word.word
                && current.punctuated_word == word.punctuated_word
        });

        if !is_duplicate {
            existing.push(word);
        }
    }
}

#[cfg(test)]
mod test {
    use owhisper_interface::stream::{Alternatives, Channel, Metadata, ModelInfo, Word};

    use super::*;

    #[test]
    fn streamed_accumulator_uses_finalize_transcript_without_duplication() {
        let mut accumulator = StreamBatchAccumulator::new();

        accumulator.observe(&transcript_response(
            0.0,
            2.0,
            false,
            "hello world",
            vec![
                stream_word("hello", 0.0, 0.8),
                stream_word("world", 0.9, 1.5),
            ],
        ));
        accumulator.observe(&transcript_response(0.0, 2.0, true, "hello world", vec![]));

        let output = accumulator.finish("session-1");
        let channel = &output.response.results.channels[0].alternatives[0];

        assert_eq!(output.mode, BatchRunMode::Streamed);
        assert_eq!(channel.transcript, "hello world");
        assert_eq!(channel.words.len(), 2);
    }

    #[test]
    fn streamed_accumulator_replaces_words_with_full_finalize_snapshot() {
        let mut accumulator = StreamBatchAccumulator::new();

        accumulator.observe(&transcript_response(
            1.0,
            1.0,
            false,
            "world",
            vec![stream_word("world", 1.0, 1.5)],
        ));
        accumulator.observe(&transcript_response(
            0.0,
            2.0,
            true,
            "hello world",
            vec![
                stream_word("hello", 0.0, 0.8),
                stream_word("world", 1.0, 1.5),
            ],
        ));

        let output = accumulator.finish("session-2");
        let channel = &output.response.results.channels[0].alternatives[0];

        assert_eq!(channel.transcript, "hello world");
        assert_eq!(channel.words.len(), 2);
        assert_eq!(channel.words[0].word, "hello");
    }

    fn transcript_response(
        start: f64,
        duration: f64,
        from_finalize: bool,
        transcript: &str,
        words: Vec<Word>,
    ) -> StreamResponse {
        StreamResponse::TranscriptResponse {
            start,
            duration,
            is_final: true,
            speech_final: true,
            from_finalize,
            channel: Channel {
                alternatives: vec![Alternatives {
                    transcript: transcript.to_string(),
                    words,
                    confidence: 0.9,
                    languages: Vec::new(),
                }],
            },
            metadata: Metadata {
                request_id: "r".to_string(),
                model_info: ModelInfo {
                    name: "".to_string(),
                    version: "".to_string(),
                    arch: "".to_string(),
                },
                model_uuid: "m".to_string(),
                extra: None,
            },
            channel_index: vec![0, 1],
        }
    }

    fn stream_word(word: &str, start: f64, end: f64) -> Word {
        Word {
            word: word.to_string(),
            start,
            end,
            confidence: 0.9,
            speaker: None,
            punctuated_word: Some(word.to_string()),
            language: None,
        }
    }
}
