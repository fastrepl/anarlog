use std::collections::HashSet;

use anlg_meeting_import::{
    meeting_file, meeting_has_content,
    teams::{CalendarEvent, TeamsClient},
};
use anlg_nango::OwnedNangoProxy;

use super::{ImportResult, MAX_PAGES, recording_windows};

pub async fn import_meetings(
    client: &TeamsClient<anlg_nango::OwnedNangoHttpClient>,
    proxy: &OwnedNangoProxy,
    known_meeting_ids: &[String],
) -> Result<ImportResult, String> {
    let known = known_meeting_ids.iter().cloned().collect::<HashSet<_>>();
    let mut events = Vec::new();
    let mut calendar_error = None;
    for (start, end) in recording_windows() {
        let start = format!("{start}T00:00:00Z");
        let end = format!("{end}T23:59:59Z");
        let mut next_link = None;
        for _ in 0..MAX_PAGES {
            match client
                .list_calendar_view(&start, &end, next_link.as_deref())
                .await
            {
                Ok(page) => {
                    events.extend(page.value);
                    let next = page.next_link.filter(|link| !link.is_empty());
                    if next.as_ref() == next_link.as_ref() {
                        break;
                    }
                    match next {
                        Some(link) => next_link = Some(link),
                        None => break,
                    }
                }
                Err(error) => {
                    calendar_error = Some(error.to_string());
                    break;
                }
            }
        }
        if calendar_error.is_some() {
            break;
        }
    }

    let mut files = Vec::new();
    let mut without_content = 0;
    let mut transcript_denied = false;
    for event in events {
        if event.join_url().is_none() {
            continue;
        }
        match import_one(client, proxy, &event, &known).await {
            Ok(Some(imported)) => {
                if !meeting_has_content(&imported) {
                    without_content += 1;
                    continue;
                }
                files.push(meeting_file("microsoft-teams", &imported)?);
            }
            Ok(None) => {}
            Err(error) if error.contains("403") => transcript_denied = true,
            Err(_) => without_content += 1,
        }
    }

    let mut warnings = Vec::new();
    if let Some(error) = calendar_error {
        warnings.push(format!(
            "Microsoft Teams calendar could not be read ({error}). Work or school accounts are required."
        ));
    }
    if transcript_denied {
        warnings.push(
            "Microsoft Teams transcripts need admin consent for OnlineMeetingTranscript.Read.All."
                .to_string(),
        );
    }
    if files.is_empty() && without_content == 0 && warnings.is_empty() {
        warnings.push(
            "Microsoft Teams did not return any meeting transcripts for this account.".to_string(),
        );
    }
    if without_content > 0 {
        warnings.push(format!(
            "{without_content} Teams meetings did not include transcripts for this account or policy."
        ));
    }
    Ok(ImportResult { files, warnings })
}

async fn import_one(
    client: &TeamsClient<anlg_nango::OwnedNangoHttpClient>,
    proxy: &OwnedNangoProxy,
    event: &CalendarEvent,
    known: &HashSet<String>,
) -> Result<Option<anlg_meeting_import::ImportedMeeting>, String> {
    let Some(join_url) = event.join_url() else {
        return Ok(None);
    };
    let Some(meeting) = client
        .find_online_meeting(join_url)
        .await
        .map_err(|error| format!("could not look up a Teams online meeting: {error}"))?
    else {
        return Ok(None);
    };
    let Some(meeting_id) = meeting.id.clone().filter(|value| !value.trim().is_empty()) else {
        return Ok(None);
    };
    if known.contains(&meeting_id) {
        return Ok(None);
    }
    let transcripts = client
        .list_transcripts(&meeting_id)
        .await
        .map_err(|error| format!("could not list Teams transcripts: {error}"))?;
    let mut segments = Vec::new();
    for transcript in transcripts {
        let Some(transcript_id) = transcript.id.filter(|value| !value.trim().is_empty()) else {
            continue;
        };
        segments.extend(download_transcript(proxy, &meeting_id, &transcript_id).await);
    }
    Ok(event.imported(&meeting, segments))
}

async fn download_transcript(
    proxy: &OwnedNangoProxy,
    meeting_id: &str,
    transcript_id: &str,
) -> Vec<anlg_meeting_import::TranscriptSegment> {
    let path = format!(
        "/v1.0/me/onlineMeetings/{}/transcripts/{}/content?$format=text/vtt",
        urlencoding::encode(meeting_id),
        urlencoding::encode(transcript_id)
    );
    let response = match proxy.get(&path) {
        Ok(request) => request.send().await,
        Err(_) => return Vec::new(),
    };
    let Ok(response) = response else {
        return Vec::new();
    };
    if !response.status().is_success() {
        return Vec::new();
    }
    let Ok(body) = response.text().await else {
        return Vec::new();
    };
    anlg_meeting_import::parse_vtt(&body)
}
