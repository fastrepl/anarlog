use std::path::PathBuf;
use std::time::Duration;

use futures_util::StreamExt;
use owhisper_client::{AdapterKind, OpenAIAdapter, StreamingBatchStream};
use owhisper_interface::ListenParams;
use owhisper_interface::batch::{Response, Word};
use owhisper_interface::batch_stream::BatchStreamEvent;

use super::super::upload::{SegmentedUpload, audio_duration, segment_plan, split_batch_upload};

/// The stream loop fails a segment that goes quiet for 30s, so keep the boundary
/// between segments visible while the next request is still uploading.
const SEGMENT_OPEN_KEEPALIVE: Duration = Duration::from_secs(10);

pub(super) struct SegmentedPlan {
    upload: SegmentedUpload,
    segment_duration: Duration,
    total_duration: Option<Duration>,
}

/// `None` when the recording fits OpenAI's per-request limits and can be streamed
/// as a single upload.
pub(super) async fn plan_openai_segments(
    file_path: &str,
    provider: &str,
) -> crate::Result<Option<SegmentedPlan>> {
    let total_duration = audio_duration(file_path);
    let limit = AdapterKind::OpenAI.batch_upload_limit();

    let Some(segment_duration) = segment_plan(file_path, total_duration, limit) else {
        return Ok(None);
    };

    let upload = split_batch_upload(file_path, segment_duration, provider).await?;

    Ok(Some(SegmentedPlan {
        upload,
        segment_duration,
        total_duration,
    }))
}

/// Streams each segment in turn as one logical stream: per-segment timestamps are
/// shifted onto the recording's timeline, progress is rescaled across all
/// segments, and the per-segment results are held back so the stream still ends
/// with exactly one result covering the whole recording.
pub(super) async fn open_segmented_stream(
    plan: SegmentedPlan,
    base_url: String,
    api_key: String,
    mut listen_params: ListenParams,
) -> Result<StreamingBatchStream, owhisper_client::Error> {
    let SegmentedPlan {
        upload,
        segment_duration,
        total_duration,
    } = plan;
    listen_params.channels = 1;

    let paths: Vec<PathBuf> = upload.paths().to_vec();
    let total = paths.len();
    let first = OpenAIAdapter::transcribe_file_streaming(
        &base_url,
        &api_key,
        &listen_params,
        paths
            .first()
            .expect("split_batch_upload rejects no segments"),
    )
    .await?;

    let stream = async_stream::stream! {
        let _upload = upload;
        let mut merged = MergedSegments::default();
        let mut opened = Some(first);

        for (index, path) in paths.iter().enumerate() {
            let offset = segment_duration.as_secs_f64() * index as f64;

            let mut segment = match opened.take() {
                Some(segment) => segment,
                None => {
                    let open = OpenAIAdapter::transcribe_file_streaming(
                        &base_url,
                        &api_key,
                        &listen_params,
                        path,
                    );
                    tokio::pin!(open);

                    loop {
                        match tokio::time::timeout(SEGMENT_OPEN_KEEPALIVE, &mut open).await {
                            Ok(Ok(segment)) => break segment,
                            Ok(Err(error)) => {
                                yield Err(error);
                                return;
                            }
                            Err(_) => yield Ok(BatchStreamEvent::Progress {
                                percentage: index as f64 / total as f64,
                                partial_text: Some(merged.transcript()),
                            }),
                        }
                    }
                }
            };

            let mut completed = false;
            while let Some(item) = segment.next().await {
                match item {
                    Err(error) => {
                        yield Err(error);
                        return;
                    }
                    Ok(BatchStreamEvent::Result { response }) => {
                        merged.absorb(response, offset);
                        completed = true;
                    }
                    Ok(BatchStreamEvent::Terminal { duration, channels, .. }) => {
                        merged.absorb_terminal(duration, channels);
                        completed = true;
                    }
                    Ok(BatchStreamEvent::Progress { percentage, partial_text }) => {
                        yield Ok(BatchStreamEvent::Progress {
                            percentage: rescale(index, percentage, total),
                            partial_text: Some(merged.with_partial(partial_text.as_deref())),
                        });
                    }
                    Ok(BatchStreamEvent::Segment { mut response, percentage }) => {
                        response.apply_offset(offset);
                        yield Ok(BatchStreamEvent::Segment {
                            response,
                            percentage: rescale(index, percentage, total),
                        });
                    }
                    Ok(error @ BatchStreamEvent::Error { .. }) => {
                        yield Ok(error);
                        return;
                    }
                }
            }

            // Leaving without a completion event lets the stream loop report the
            // missing result rather than silently returning a partial transcript.
            if !completed {
                tracing::error!(segment = index, "segmented batch stream ended without a result");
                return;
            }
        }

        if let Some(event) = merged.into_event(total_duration) {
            yield Ok(event);
        }
    };

    Ok(Box::pin(stream))
}

fn rescale(index: usize, percentage: f64, total: usize) -> f64 {
    ((index as f64 + percentage.clamp(0.0, 1.0)) / total as f64).clamp(0.0, 1.0)
}

#[derive(Default)]
struct MergedSegments {
    metadata: Option<serde_json::Value>,
    transcripts: Vec<String>,
    words: Vec<Word>,
    saw_result: bool,
    terminal: Option<(f64, u32)>,
}

impl MergedSegments {
    fn absorb(&mut self, response: Response, offset: f64) {
        self.saw_result = true;
        if self.metadata.is_none() {
            self.metadata = Some(response.metadata);
        }

        let Some(alternative) = response
            .results
            .channels
            .into_iter()
            .next()
            .and_then(|channel| channel.alternatives.into_iter().next())
        else {
            return;
        };

        let transcript = alternative.transcript.trim();
        if !transcript.is_empty() {
            self.transcripts.push(transcript.to_string());
        }
        self.words
            .extend(alternative.words.into_iter().map(|mut word| {
                word.start += offset;
                word.end += offset;
                word
            }));
    }

    fn absorb_terminal(&mut self, duration: f64, channels: u32) {
        let (total, _) = self.terminal.unwrap_or((0.0, channels));
        self.terminal = Some((total + duration, channels));
    }

    fn transcript(&self) -> String {
        self.transcripts.join(" ")
    }

    fn with_partial(&self, partial: Option<&str>) -> String {
        let partial = partial.unwrap_or_default().trim();
        if partial.is_empty() {
            return self.transcript();
        }
        if self.transcripts.is_empty() {
            return partial.to_string();
        }
        format!("{} {partial}", self.transcript())
    }

    fn into_event(self, total_duration: Option<Duration>) -> Option<BatchStreamEvent> {
        if !self.saw_result {
            return self
                .terminal
                .map(|(duration, channels)| BatchStreamEvent::Terminal {
                    request_id: String::new(),
                    created: String::new(),
                    duration: total_duration.map_or(duration, |total| total.as_secs_f64()),
                    channels,
                });
        }

        let mut metadata = self.metadata.unwrap_or_else(|| serde_json::json!({}));
        if let (Some(object), Some(total)) = (metadata.as_object_mut(), total_duration) {
            object.insert(
                "duration".to_string(),
                serde_json::json!(total.as_secs_f64()),
            );
        }

        Some(BatchStreamEvent::Result {
            response: Response {
                metadata,
                results: owhisper_interface::batch::Results {
                    channels: vec![owhisper_interface::batch::Channel {
                        alternatives: vec![owhisper_interface::batch::Alternatives {
                            transcript: self.transcripts.join(" "),
                            confidence: 1.0,
                            words: self.words,
                        }],
                    }],
                },
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result_event(transcript: &str, start: f64) -> Response {
        Response {
            metadata: serde_json::json!({ "timing_source": "synthetic_text" }),
            results: owhisper_interface::batch::Results {
                channels: vec![owhisper_interface::batch::Channel {
                    alternatives: vec![owhisper_interface::batch::Alternatives {
                        transcript: transcript.to_string(),
                        confidence: 1.0,
                        words: vec![Word {
                            word: transcript.to_string(),
                            start,
                            end: start + 0.5,
                            confidence: 1.0,
                            channel: 0,
                            speaker: None,
                            punctuated_word: None,
                        }],
                    }],
                }],
            },
        }
    }

    #[test]
    fn merged_result_shifts_words_and_keeps_total_duration() {
        let mut merged = MergedSegments::default();
        merged.absorb(result_event("first", 1.0), 0.0);
        merged.absorb(result_event("second", 2.0), 600.0);

        let event = merged
            .into_event(Some(Duration::from_secs(1_205)))
            .expect("a result was absorbed");
        let BatchStreamEvent::Result { response } = event else {
            panic!("expected a result event");
        };
        let alternative = &response.results.channels[0].alternatives[0];

        assert_eq!(alternative.transcript, "first second");
        assert_eq!(alternative.words[0].start, 1.0);
        assert_eq!(alternative.words[1].start, 602.0);
        assert_eq!(response.metadata["duration"], 1205.0);
        assert_eq!(response.metadata["timing_source"], "synthetic_text");
    }

    #[test]
    fn partial_text_continues_across_segments() {
        let mut merged = MergedSegments::default();
        assert_eq!(merged.with_partial(Some("hello")), "hello");

        merged.absorb(result_event("first segment", 0.0), 0.0);
        assert_eq!(
            merged.with_partial(Some("and then")),
            "first segment and then"
        );
        assert_eq!(merged.with_partial(None), "first segment");
    }

    #[test]
    fn progress_stays_monotonic_across_segment_boundaries() {
        let end_of_first = rescale(0, 0.99, 5);
        let boundary = 1.0 / 5.0;
        let start_of_second = rescale(1, 0.03, 5);

        assert!(end_of_first < boundary);
        assert!(boundary < start_of_second);
        assert_eq!(rescale(4, 1.0, 5), 1.0);
    }

    async fn transcription_sse(
        axum::extract::State(requests): axum::extract::State<
            std::sync::Arc<std::sync::atomic::AtomicUsize>,
        >,
        _body: axum::body::Bytes,
    ) -> impl axum::response::IntoResponse {
        let index = requests.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let body = format!(
            "data: {{\"type\":\"transcript.text.delta\",\"delta\":\"segment \"}}\n\n\
             data: {{\"type\":\"transcript.text.delta\",\"delta\":\"{index}\"}}\n\n\
             data: {{\"type\":\"transcript.text.done\",\"text\":\"segment {index}\"}}\n\n"
        );

        (
            [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
            body,
        )
    }

    #[tokio::test]
    async fn streams_every_segment_as_one_logical_stream() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("audio.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(&source, spec).unwrap();
        for frame in 0..16_000 * 3 {
            writer
                .write_sample(((frame as f32) * 0.01).sin() * 0.5)
                .unwrap();
        }
        writer.finalize().unwrap();

        let requests = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let app = axum::Router::new()
            .fallback(transcription_sse)
            .with_state(requests.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let upload = split_batch_upload(source.to_str().unwrap(), Duration::from_secs(1), "openai")
            .await
            .unwrap();
        assert_eq!(upload.paths().len(), 3);

        let mut stream = open_segmented_stream(
            SegmentedPlan {
                upload,
                segment_duration: Duration::from_secs(1),
                total_duration: Some(Duration::from_secs(3)),
            },
            format!("http://{address}/v1"),
            "test-key".to_string(),
            ListenParams::default(),
        )
        .await
        .unwrap();

        let mut progress = Vec::new();
        let mut results = Vec::new();
        while let Some(event) = stream.next().await {
            match event.unwrap() {
                BatchStreamEvent::Progress {
                    percentage,
                    partial_text,
                } => progress.push((percentage, partial_text.unwrap_or_default())),
                BatchStreamEvent::Result { response } => results.push(response),
                other => panic!("unexpected event: {other:?}"),
            }
        }

        assert_eq!(requests.load(std::sync::atomic::Ordering::SeqCst), 3);
        assert_eq!(results.len(), 1, "segments must produce one merged result");
        assert_eq!(
            results[0].results.channels[0].alternatives[0].transcript,
            "segment 0 segment 1 segment 2"
        );
        assert_eq!(results[0].metadata["duration"], 3.0);

        assert!(progress.windows(2).all(|pair| pair[0].0 <= pair[1].0));
        assert!(progress.iter().all(|(percentage, _)| *percentage <= 1.0));
        assert_eq!(
            progress.last().map(|(_, text)| text.as_str()),
            Some("segment 0 segment 1 segment 2")
        );
    }

    #[test]
    fn segments_that_only_reach_terminal_do_not_emit_an_empty_result() {
        let mut merged = MergedSegments::default();
        merged.absorb_terminal(600.0, 2);
        merged.absorb_terminal(300.0, 2);

        let event = merged.into_event(None).expect("a terminal was absorbed");
        let BatchStreamEvent::Terminal {
            duration, channels, ..
        } = event
        else {
            panic!("expected a terminal event");
        };

        assert_eq!(duration, 900.0);
        assert_eq!(channels, 2);
    }
}
