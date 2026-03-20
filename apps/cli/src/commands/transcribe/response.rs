use owhisper_interface::batch;
use owhisper_interface::stream::StreamResponse;

pub(super) fn batch_response_from_streams(
    segments: Vec<StreamResponse>,
) -> Option<batch::Response> {
    if segments.is_empty() {
        return None;
    }

    let mut all_words: Vec<batch::Word> = Vec::new();
    let mut all_transcripts: Vec<String> = Vec::new();
    let mut total_confidence = 0.0;
    let mut max_end = 0.0_f64;
    let mut count = 0usize;

    for segment in segments {
        let StreamResponse::TranscriptResponse {
            channel,
            start,
            duration,
            ..
        } = segment
        else {
            continue;
        };

        let Some(alt) = channel.alternatives.into_iter().next() else {
            continue;
        };

        let text = alt.transcript.trim().to_string();
        if text.is_empty() {
            continue;
        }

        let words: Vec<batch::Word> = alt.words.into_iter().map(batch::Word::from).collect();
        all_words.extend(words);
        all_transcripts.push(text);
        total_confidence += alt.confidence;
        max_end = max_end.max(start + duration);
        count += 1;
    }

    if count == 0 {
        return None;
    }

    let transcript = all_transcripts.join(" ");
    let avg_confidence = total_confidence / count as f64;

    Some(batch::Response {
        metadata: serde_json::json!({ "duration": max_end }),
        results: batch::Results {
            channels: vec![batch::Channel {
                alternatives: vec![batch::Alternatives {
                    transcript,
                    confidence: avg_confidence,
                    words: all_words,
                }],
            }],
        },
    })
}
