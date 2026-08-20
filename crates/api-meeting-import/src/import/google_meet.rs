use std::collections::{HashMap, HashSet};

use anlg_meeting_import::{
    TranscriptSegment,
    google_meet::{
        ConferenceRecord, GoogleMeetClient, Participant, conference_origin_ms, entry_segment,
    },
    meeting_file, meeting_has_content, nonempty,
};

use super::{ImportResult, MAX_PAGES};

pub async fn import_meetings(
    client: &GoogleMeetClient<anlg_nango::OwnedNangoHttpClient>,
    known_meeting_ids: &[String],
) -> Result<ImportResult, String> {
    let known = known_meeting_ids.iter().cloned().collect::<HashSet<_>>();
    let mut records = Vec::new();
    let mut page_token = None;
    for _ in 0..MAX_PAGES {
        let page = client
            .list_conference_records(page_token.as_deref())
            .await
            .map_err(|error| format!("could not list Google Meet records: {error}"))?;
        records.extend(page.conference_records);
        let next = page.next_page_token.filter(|token| !token.is_empty());
        if next.as_ref() == page_token.as_ref() {
            break;
        }
        match next {
            Some(token) => page_token = Some(token),
            None => break,
        }
    }

    let mut files = Vec::new();
    let mut without_content = 0;
    for record in records {
        let Some(name) = record.resource_name().map(ToOwned::to_owned) else {
            continue;
        };
        if known.contains(&name) {
            continue;
        }
        let imported = import_one(client, &record).await?;
        if !meeting_has_content(&imported) {
            without_content += 1;
            continue;
        }
        files.push(meeting_file("google-meet", &imported)?);
    }

    let mut warnings = Vec::new();
    if files.is_empty() && without_content == 0 {
        warnings.push(
            "Google Meet did not return any conference transcripts. Transcripts are available for about 30 days after a meeting ends.".to_string(),
        );
    }
    if without_content > 0 {
        warnings.push(format!(
            "{without_content} Google Meet conferences did not include transcripts for this account or Workspace setting."
        ));
    }
    Ok(ImportResult { files, warnings })
}

async fn import_one(
    client: &GoogleMeetClient<anlg_nango::OwnedNangoHttpClient>,
    record: &ConferenceRecord,
) -> Result<anlg_meeting_import::ImportedMeeting, String> {
    let conference_name = record
        .resource_name()
        .ok_or_else(|| "Google Meet conference is missing a name".to_string())?;
    let title = space_title(client, record.space.as_deref()).await;
    let speakers = participant_names(client, conference_name).await?;
    let origin_ms = conference_origin_ms(record);
    let mut transcript = Vec::new();
    let mut page_token = None;
    for _ in 0..MAX_PAGES {
        let page = client
            .list_transcripts(conference_name, page_token.as_deref())
            .await
            .map_err(|error| format!("could not list Google Meet transcripts: {error}"))?;
        for item in page.transcripts {
            let Some(name) = nonempty(item.name.as_deref()) else {
                continue;
            };
            transcript.extend(transcript_entries(client, &name, origin_ms, &speakers).await?);
        }
        let next = page.next_page_token.filter(|token| !token.is_empty());
        if next.as_ref() == page_token.as_ref() {
            break;
        }
        match next {
            Some(token) => page_token = Some(token),
            None => break,
        }
    }
    record
        .imported(title, transcript)
        .ok_or_else(|| "Google Meet conference is missing a name".to_string())
}

async fn transcript_entries(
    client: &GoogleMeetClient<anlg_nango::OwnedNangoHttpClient>,
    transcript_name: &str,
    origin_ms: u64,
    speakers: &HashMap<String, String>,
) -> Result<Vec<TranscriptSegment>, String> {
    let mut entries = Vec::new();
    let mut page_token = None;
    for _ in 0..MAX_PAGES {
        let page = client
            .list_entries(transcript_name, page_token.as_deref())
            .await
            .map_err(|error| format!("could not list Google Meet transcript entries: {error}"))?;
        entries.extend(page.transcript_entries.into_iter().filter_map(|entry| {
            let speaker = entry
                .participant
                .as_deref()
                .and_then(|name| speakers.get(name).cloned())
                .unwrap_or_default();
            entry_segment(&entry, origin_ms, speaker)
        }));
        let next = page.next_page_token.filter(|token| !token.is_empty());
        if next.as_ref() == page_token.as_ref() {
            break;
        }
        match next {
            Some(token) => page_token = Some(token),
            None => break,
        }
    }
    Ok(entries)
}

async fn participant_names(
    client: &GoogleMeetClient<anlg_nango::OwnedNangoHttpClient>,
    conference_name: &str,
) -> Result<HashMap<String, String>, String> {
    let mut names = HashMap::new();
    let mut page_token = None;
    for _ in 0..MAX_PAGES {
        let page = client
            .list_participants(conference_name, page_token.as_deref())
            .await
            .map_err(|error| format!("could not list Google Meet participants: {error}"))?;
        for participant in page.participants {
            remember_participant(&mut names, participant);
        }
        let next = page.next_page_token.filter(|token| !token.is_empty());
        if next.as_ref() == page_token.as_ref() {
            break;
        }
        match next {
            Some(token) => page_token = Some(token),
            None => break,
        }
    }
    Ok(names)
}

fn remember_participant(names: &mut HashMap<String, String>, participant: Participant) {
    let Some(name) = nonempty(participant.name.as_deref()) else {
        return;
    };
    if let Some(display_name) = participant.display_name() {
        names.insert(name, display_name);
    }
}

async fn space_title(
    client: &GoogleMeetClient<anlg_nango::OwnedNangoHttpClient>,
    space_name: Option<&str>,
) -> String {
    let Some(space_name) = space_name.map(str::trim).filter(|value| !value.is_empty()) else {
        return "Google Meet".to_string();
    };
    match client.get_space(space_name).await {
        Ok(Some(space)) => nonempty(space.display_name.as_deref())
            .or_else(|| nonempty(space.meeting_code.as_deref()))
            .unwrap_or_else(|| "Google Meet".to_string()),
        _ => "Google Meet".to_string(),
    }
}
