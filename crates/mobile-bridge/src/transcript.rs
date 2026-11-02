use anlg_transcript::{
    RenderTranscriptHuman, RenderTranscriptRequest, SpeakerContext, StoredLiveTranscriptDelta,
    materialize_live_transcript, normalize_stored_transcript, parse_stored_speaker_hints,
    render_input_from_stored, render_transcript_segments,
};
use serde::{Deserialize, Serialize};

use crate::error::{BridgeError, serialization_error};

#[derive(Deserialize)]
struct Request {
    id: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    provider: String,
    started_at_ms: i64,
    owner_user_id: Option<String>,
    words_json: String,
    speaker_hints_json: String,
    pending_deltas_json: String,
    speaker_context_json: Option<String>,
    humans: Vec<RenderTranscriptHuman>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Segment {
    id: String,
    text: String,
    speaker: String,
    start_ms: i64,
    end_ms: i64,
    word_count: usize,
}

#[uniffi::export]
pub fn render_stored_transcript(request_json: String) -> Result<String, BridgeError> {
    let request: Request =
        serde_json::from_str(&request_json).map_err(|error| BridgeError::InvalidParamsJson {
            reason: error.to_string(),
        })?;
    let deltas: Vec<StoredLiveTranscriptDelta> = serde_json::from_str(&request.pending_deltas_json)
        .map_err(|error| BridgeError::InvalidParamsJson {
            reason: error.to_string(),
        })?;
    let stored = serde_json::from_str(&request.words_json).map_err(serialization_error)?;
    let (words, hints) = normalize_stored_transcript(
        &request.id,
        stored,
        parse_stored_speaker_hints(&request.speaker_hints_json),
    );
    let (mut words, mut hints) = materialize_live_transcript(words, hints, &deltas);
    anlg_transcript::normalize_legacy_batch_channels(
        &request.source,
        &request.provider,
        &mut words,
        &mut hints,
    );
    let input = render_input_from_stored(Some(request.started_at_ms), &words, &hints);
    let context = request
        .speaker_context_json
        .as_deref()
        .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
        .and_then(|value| {
            value
                .get("intervals")
                .and_then(serde_json::Value::as_array)
                .cloned()
        })
        .map(|intervals| SpeakerContext {
            intervals: intervals
                .into_iter()
                .filter_map(|value| {
                    serde_json::from_value::<anlg_transcript::SpeakerContextInterval>(value).ok()
                })
                .filter(|interval| interval.start_ms < interval.end_ms)
                .collect(),
        });
    let segments = render_transcript_segments(RenderTranscriptRequest {
        transcripts: input.into_iter().collect(),
        speaker_context: context,
        preview: None,
        participant_human_ids: vec![],
        self_human_id: request.owner_user_id,
        humans: request.humans,
    })
    .into_iter()
    .map(|segment| Segment {
        id: format!("{}:{}", request.id, segment.id),
        text: segment.text,
        speaker: segment.speaker_label,
        start_ms: request.started_at_ms.saturating_add(segment.start_ms),
        end_ms: request.started_at_ms.saturating_add(segment.end_ms),
        word_count: segment.words.len(),
    })
    .collect::<Vec<_>>();
    serde_json::to_string(&segments).map_err(serialization_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn word(id: &str, text: &str, channel: i32, speaker: i32, start: i64) -> Value {
        json!({"id": id, "text": text, "channel": channel, "speaker_index": speaker,
            "start_ms": start, "end_ms": start + 100, "state": "final"})
    }

    fn request(words: &[Value]) -> Value {
        let hints: Vec<_> = words
            .iter()
            .map(|word| {
                json!({
                    "word_id": word["id"], "type": "provider_speaker_index",
                    "value": {"channel": word["channel"], "speaker_index": word["speaker_index"]}
                })
            })
            .collect();
        json!({
            "id": "transcript", "started_at_ms": 1000, "owner_user_id": "john",
            "words_json": serde_json::to_string(words).unwrap(),
            "speaker_hints_json": serde_json::to_string(&hints).unwrap(),
            "pending_deltas_json": "[]", "speaker_context_json": null,
            "humans": [{"human_id": "john", "name": "John Jeong"},
                {"human_id": "artem", "name": "Artem Savelev"}]
        })
    }

    fn render(request: &Value) -> Vec<Value> {
        serde_json::from_str(&render_stored_transcript(request.to_string()).unwrap()).unwrap()
    }

    #[test]
    fn legacy_words_without_ids_keep_text_and_word_level_speakers() {
        let mut request = request(&[]);
        request["words_json"] = json!(
            r#"[{"text":"Hello","start_ms":0,"end_ms":100,"channel":0,"speaker_index":0},{"text":"Hi","start_ms":100,"end_ms":200,"channel":0,"speaker_index":1}]"#
        );
        let segments = render(&request);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0]["text"], "Hello");
        assert_eq!(segments[1]["speaker"], "Speaker 2");
    }

    #[test]
    fn shared_phone_speakers_survive_live_correction_and_reload() {
        let first = word("a", "Hello", 0, 0, 0);
        let second = word("b", "Hi", 0, 1, 100);
        let third = word("c", "Again", 0, 0, 200);
        let mut live = request(&[first.clone(), word("b", "Hi", 0, 0, 100)]);
        live["pending_deltas_json"] = json!(
            json!([{
                "new_words": [second.clone(), third.clone()], "replaced_ids": ["b"]
            }])
            .to_string()
        );
        let saved = request(&[first, second, third]);
        let segments = render(&saved);
        assert_eq!(render(&live), segments);
        assert_eq!(
            segments
                .iter()
                .map(|s| s["speaker"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["Speaker 1", "Speaker 2", "Speaker 1"]
        );
        assert_eq!(segments[0]["startMs"], 1000);
        assert_eq!(segments[1]["text"], "Hi");
    }

    #[test]
    fn desktop_capture_context_resolves_the_recording_owner_on_mobile() {
        let mut request = request(&[
            word("a", "Yo, what's up?", 0, 0, 0),
            word("b", "Hello.", 1, 0, 100),
            word("c", "Alright.", 0, 0, 200),
        ]);
        request["speaker_context_json"] = json!(
            json!({"intervals": [{
                "start_ms": 1000, "end_ms": 2000, "active_call": true,
                "calendar_call": false, "mic_isolated": true, "shared_microphone": false,
                "title": "DEFCON 1", "self_names": [],
                "participants": [{"human_id": "john", "name": "John Jeong"},
                    {"human_id": "artem", "name": "Artem Savelev"}]
            }]})
            .to_string()
        );
        let mut context: Value =
            serde_json::from_str(request["speaker_context_json"].as_str().unwrap()).unwrap();
        context["intervals"]
            .as_array_mut()
            .unwrap()
            .push(json!({"start_ms": 3000}));
        request["speaker_context_json"] = json!(context.to_string());
        let segments = render(&request);
        assert_eq!(
            segments
                .iter()
                .map(|s| s["speaker"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["John Jeong", "Artem Savelev", "John Jeong"]
        );

        let mut hints: Vec<Value> =
            serde_json::from_str(request["speaker_hints_json"].as_str().unwrap()).unwrap();
        hints.push(
            json!({"word_id": "a", "type": "user_speaker_assignment", "value": {
                "scope": "segment", "word_ids": ["a"], "human_id": "artem"
            }}),
        );
        request["speaker_hints_json"] = json!(serde_json::to_string(&hints).unwrap());
        assert_eq!(render(&request)[0]["speaker"], "Artem Savelev");
    }
}
