use anlg_transcript::{
    ChannelProfile, IdentityAssignment, IdentityScope, RenderTranscriptHuman,
    RenderTranscriptInput, RenderTranscriptRequest, RenderTranscriptWordInput,
};
use desktop_runtime::Result;
use serde_json::Value;

use super::{data::CanonicalMeeting, failure};

pub fn render(meeting: &CanonicalMeeting) -> Result<Vec<anlg_export_core::TranscriptItem>> {
    let mut humans = meeting
        .humans
        .iter()
        .filter_map(|human| {
            Some(RenderTranscriptHuman {
                human_id: human["id"].as_str()?.into(),
                name: human["name"].as_str()?.into(),
            })
        })
        .collect::<Vec<_>>();
    let mut transcripts = Vec::new();
    for transcript in &meeting.transcripts {
        let source: Vec<Value> = serde_json::from_str(
            transcript["words_json"]
                .as_str()
                .ok_or_else(|| failure("Transcript lacks words"))?,
        )
        .map_err(failure)?;
        let mut words = Vec::new();
        let mut assignments = Vec::new();
        for word in source {
            let id = word["id"]
                .as_str()
                .ok_or_else(|| failure("Transcript word lacks id; use canonical JSON"))?
                .to_owned();
            words.push(RenderTranscriptWordInput {
                id: id.clone(),
                text: word["text"]
                    .as_str()
                    .ok_or_else(|| failure("Transcript word lacks text"))?
                    .into(),
                start_ms: word["start_ms"]
                    .as_i64()
                    .ok_or_else(|| failure("Transcript word lacks start"))?,
                end_ms: word["end_ms"]
                    .as_i64()
                    .ok_or_else(|| failure("Transcript word lacks end"))?,
                channel: word["channel"]
                    .as_i64()
                    .unwrap_or(0)
                    .try_into()
                    .map_err(failure)?,
                speaker_index: word["speaker_index"]
                    .as_i64()
                    .map(i32::try_from)
                    .transpose()
                    .map_err(failure)?,
                synthetic_timing: anlg_transcript::synthetic_timing_from_metadata(
                    word.get("metadata"),
                ),
            });
            if let Some(name) = word["speaker"].as_str().filter(|name| !name.is_empty()) {
                let human_id = format!("import-speaker:{name}");
                if !humans.iter().any(|human| human.human_id == human_id) {
                    humans.push(RenderTranscriptHuman {
                        human_id: human_id.clone(),
                        name: name.into(),
                    });
                }
                assignments.push(IdentityAssignment {
                    human_id,
                    scope: IdentityScope::Words { word_ids: vec![id] },
                });
            }
        }
        let hints: Vec<Value> =
            serde_json::from_str(transcript["speaker_hints_json"].as_str().unwrap_or("[]"))
                .map_err(failure)?;
        for kind in [
            "provider_speaker_index",
            "automatic_speaker_assignment",
            "user_speaker_assignment",
        ] {
            for hint in hints.iter().filter(|hint| hint["type"] == kind) {
                let value = match &hint["value"] {
                    Value::String(text) => serde_json::from_str::<Value>(text).map_err(failure)?,
                    value => value.clone(),
                };
                let word = words
                    .iter_mut()
                    .find(|word| hint["word_id"].as_str() == Some(&word.id));
                if kind == "provider_speaker_index" {
                    let word = word.ok_or_else(|| {
                        failure("Speaker hint refers to missing word; use canonical JSON")
                    })?;
                    word.speaker_index = value["speaker_index"]
                        .as_i64()
                        .map(i32::try_from)
                        .transpose()
                        .map_err(failure)?;
                    if let Some(channel) = value["channel"].as_i64() {
                        word.channel = channel.try_into().map_err(failure)?;
                    }
                    continue;
                }
                let human_id = value["human_id"]
                    .as_str()
                    .ok_or_else(|| failure("Speaker assignment lacks human id"))?
                    .to_owned();
                let scope = if value["scope"] == "segment" {
                    IdentityScope::Words {
                        word_ids: serde_json::from_value(value["word_ids"].clone())
                            .map_err(failure)?,
                    }
                } else if let Some(scope) = value.get("scope").filter(|scope| scope.is_object()) {
                    serde_json::from_value(scope.clone()).map_err(failure)?
                } else if let Some(channel) = value["channel"].as_i64() {
                    let channel = ChannelProfile::from(i32::try_from(channel).map_err(failure)?);
                    match value["speaker_index"].as_i64() {
                        Some(index) => IdentityScope::ChannelSpeaker {
                            channel,
                            speaker_index: index.try_into().map_err(failure)?,
                        },
                        None => IdentityScope::Channel { channel },
                    }
                } else {
                    let word =
                        word.ok_or_else(|| failure("Speaker assignment refers to missing word"))?;
                    let channel = ChannelProfile::from(word.channel);
                    match word.speaker_index {
                        Some(speaker_index) => IdentityScope::ChannelSpeaker {
                            channel,
                            speaker_index,
                        },
                        None => IdentityScope::Channel { channel },
                    }
                };
                assignments.push(IdentityAssignment { human_id, scope });
            }
        }
        if hints.iter().any(|hint| {
            ![
                "provider_speaker_index",
                "automatic_speaker_assignment",
                "user_speaker_assignment",
            ]
            .contains(&hint["type"].as_str().unwrap_or(""))
        }) {
            return Err(failure(
                "Unknown speaker hint; use canonical JSON to preserve it",
            ));
        }
        transcripts.push(RenderTranscriptInput {
            started_at: transcript["started_at_ms"].as_i64(),
            words,
            assignments,
        });
    }
    let request = RenderTranscriptRequest {
        speaker_context: None,
        preview: None,
        transcripts,
        humans,
        participant_human_ids: meeting
            .participants
            .iter()
            .filter_map(|participant| {
                participant["human_id"]
                    .as_str()
                    .filter(|id| !id.is_empty())
                    .map(str::to_owned)
            })
            .collect(),
        self_human_id: None,
    };
    Ok(anlg_transcript::render_transcript_segments(request)
        .into_iter()
        .map(|segment| anlg_export_core::TranscriptItem {
            speaker: Some(segment.speaker_label),
            text: segment.text,
        })
        .collect())
}
