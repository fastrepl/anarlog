use serde_json::Value;

/// At most this many events in one batch; the page script sends a few per 200 ms.
pub const MAX_EVENTS_PER_BATCH: usize = 500;
const MAX_PARTICIPANTS: usize = 200;
const MAX_ID_CHARS: usize = 256;
const MAX_NAME_CHARS: usize = 120;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeetParticipant {
    pub participant_id: String,
    pub display_name: Option<String>,
    /// `None` when Meet didn't mark the person's own tile.
    pub is_self: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeetSpeaker {
    pub participant_id: String,
    pub display_name: Option<String>,
}

/// One reading of the Meet tab, for `anlg_audio::meeting::SpeakerObservation` with source
/// `BrowserExtension` and platform `GoogleMeet`. Meet's grid shows only visible tiles, so a roster
/// from it is never complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeetObservation {
    /// The page's wall clock (`Date.now()`); map with `instant_from_wall_ms`.
    pub at_wall_ms: i64,
    pub participants: Vec<MeetParticipant>,
    /// Meet's own participant count, including the person, when it shows one.
    pub participant_count: Option<u32>,
    /// `None`: this reading carries no speaking signal. `Some(vec![])`: Meet lit nobody.
    /// Only a lit tile is a speaking cue; a name on the roster never is.
    pub speaking: Option<Vec<MeetSpeaker>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CueError {
    TooMany,
    /// A heartbeat from another call: the tab moved, so none of the batch belongs to this one.
    OtherCall,
}

/// Turns one link's cue batches into observations, carrying the last roster forward so a speaker
/// change names who else is visible. A new link starts a new reader: the page script then sends its
/// speaker, roster and heartbeat afresh.
#[derive(Debug, Default)]
pub struct MeetCueReader {
    roster: Vec<MeetParticipant>,
    count: Option<u32>,
}

enum Cue {
    Roster {
        at: i64,
        participants: Vec<MeetParticipant>,
        count: Option<u32>,
    },
    Speaker {
        at: i64,
        speaker: Option<MeetSpeaker>,
    },
    Heartbeat {
        code: Option<String>,
    },
}

impl MeetCueReader {
    pub fn read(
        &mut self,
        meeting_code: &str,
        events: &[Value],
    ) -> Result<Vec<MeetObservation>, CueError> {
        if events.len() > MAX_EVENTS_PER_BATCH {
            return Err(CueError::TooMany);
        }
        let mut cues: Vec<Cue> = events.iter().filter_map(parse).collect();
        if cues
            .iter()
            .any(|cue| matches!(cue, Cue::Heartbeat { code: Some(code) } if code != meeting_code))
        {
            return Err(CueError::OtherCall);
        }
        // One tick reports the lit tile before the roster read in the same instant; apply the
        // roster first so that speaker cue sees who is visible.
        cues.sort_by_key(|cue| match cue {
            Cue::Roster { at, .. } => (*at, 0),
            Cue::Speaker { at, .. } => (*at, 1),
            Cue::Heartbeat { .. } => (i64::MAX, 2),
        });
        let mut observations = Vec::new();
        for cue in cues {
            match cue {
                Cue::Roster {
                    at,
                    participants,
                    count,
                } => {
                    self.roster = participants;
                    self.count = count;
                    observations.push(self.observation(at, None));
                }
                Cue::Speaker { at, speaker } => {
                    observations.push(self.observation(at, Some(speaker.into_iter().collect())));
                }
                Cue::Heartbeat { .. } => {}
            }
        }
        Ok(observations)
    }

    fn observation(&self, at: i64, speaking: Option<Vec<MeetSpeaker>>) -> MeetObservation {
        MeetObservation {
            at_wall_ms: at,
            participants: self.roster.clone(),
            participant_count: self.count,
            speaking,
        }
    }
}

fn text(value: &Value, limit: usize) -> Option<String> {
    let value = value
        .as_str()?
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    (!value.is_empty() && value.chars().count() <= limit).then_some(value)
}

fn parse(event: &Value) -> Option<Cue> {
    let at = event.get("at").and_then(Value::as_i64).filter(|at| *at > 0);
    match event.get("kind")?.as_str()? {
        "speaker" => {
            let at = at?;
            let speaker = match event.get("participantId")? {
                Value::Null => None,
                id => Some(MeetSpeaker {
                    participant_id: text(id, MAX_ID_CHARS)?,
                    display_name: event
                        .get("name")
                        .and_then(|name| text(name, MAX_NAME_CHARS)),
                }),
            };
            Some(Cue::Speaker { at, speaker })
        }
        "roster" => {
            let at = at?;
            let self_id = event.get("selfId").and_then(|id| text(id, MAX_ID_CHARS));
            let rows = event.get("participants")?.as_array()?;
            if rows.len() > MAX_PARTICIPANTS {
                return None;
            }
            let participants = rows
                .iter()
                .filter_map(|row| {
                    let participant_id = text(row.get("participantId")?, MAX_ID_CHARS)?;
                    Some(MeetParticipant {
                        is_self: self_id.as_ref().map(|own| *own == participant_id),
                        display_name: row.get("name").and_then(|name| text(name, MAX_NAME_CHARS)),
                        participant_id,
                    })
                })
                .collect();
            let count = event
                .get("count")
                .and_then(Value::as_u64)
                .and_then(|count| u32::try_from(count).ok());
            Some(Cue::Roster {
                at,
                participants,
                count,
            })
        }
        "heartbeat" => Some(Cue::Heartbeat {
            code: event
                .get("meetingCode")
                .and_then(Value::as_str)
                .map(str::to_owned),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const CODE: &str = "abc-defg-hij";

    fn first_tick() -> Vec<Value> {
        vec![
            json!({"kind": "speaker", "at": 10, "participantId": "p2", "name": "Alice Kim"}),
            json!({
                "kind": "roster", "at": 10, "selfId": "p1", "count": 3,
                "participants": [
                    {"participantId": "p1", "name": "John Jeong"},
                    {"participantId": "p2", "name": "Alice  Kim"},
                    {"participantId": "p3", "name": ""},
                ],
            }),
            json!({"kind": "heartbeat", "at": 10, "meetingCode": CODE}),
        ]
    }

    #[test]
    fn a_lit_tile_is_the_only_speaking_cue_and_sees_the_roster_from_its_tick() {
        let mut reader = MeetCueReader::default();
        let observations = reader.read(CODE, &first_tick()).unwrap();
        assert_eq!(observations.len(), 2);
        let roster = &observations[0];
        assert_eq!(roster.speaking, None);
        assert_eq!(roster.participant_count, Some(3));
        assert_eq!(
            roster.participants,
            vec![
                MeetParticipant {
                    participant_id: "p1".into(),
                    display_name: Some("John Jeong".into()),
                    is_self: Some(true),
                },
                MeetParticipant {
                    participant_id: "p2".into(),
                    display_name: Some("Alice Kim".into()),
                    is_self: Some(false),
                },
                MeetParticipant {
                    participant_id: "p3".into(),
                    display_name: None,
                    is_self: Some(false),
                },
            ]
        );
        assert_eq!(
            observations[1].speaking,
            Some(vec![MeetSpeaker {
                participant_id: "p2".into(),
                display_name: Some("Alice Kim".into()),
            }])
        );
        assert_eq!(observations[1].participants.len(), 3);

        let nobody = reader
            .read(
                CODE,
                &[json!({"kind": "speaker", "at": 20, "participantId": null, "name": null})],
            )
            .unwrap();
        assert_eq!(nobody[0].speaking, Some(vec![]));
        assert_eq!(nobody[0].participants.len(), 3);
    }

    #[test]
    fn refuses_a_batch_from_another_call_and_drops_malformed_cues() {
        let mut reader = MeetCueReader::default();
        let mut moved = first_tick();
        moved[2] = json!({"kind": "heartbeat", "at": 10, "meetingCode": "xxx-yyyy-zzz"});
        assert_eq!(reader.read(CODE, &moved), Err(CueError::OtherCall));
        assert_eq!(
            reader.read(CODE, &vec![json!({}); MAX_EVENTS_PER_BATCH + 1]),
            Err(CueError::TooMany)
        );
        let junk = [
            json!({"kind": "speaker", "participantId": "p2"}),
            json!({"kind": "speaker", "at": -1, "participantId": "p2"}),
            json!({"kind": "speaker", "at": 5, "participantId": "x".repeat(MAX_ID_CHARS + 1)}),
            json!({"kind": "roster", "at": 5, "participants": "everyone"}),
            json!({"kind": "camera", "at": 5}),
            json!("speaker"),
        ];
        assert_eq!(reader.read(CODE, &junk), Ok(vec![]));
    }
}
