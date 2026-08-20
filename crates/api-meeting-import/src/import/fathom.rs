use std::collections::HashSet;

use anlg_meeting_import::{
    fathom::{FathomClient, FathomMeeting},
    meeting_file, meeting_has_content,
};
use chrono::{Duration, Utc};

use super::{ImportResult, MAX_PAGES};

pub async fn import_meetings(
    client: &FathomClient<anlg_nango::OwnedNangoHttpClient>,
    known_meeting_ids: &[String],
) -> Result<ImportResult, String> {
    let known = known_meeting_ids.iter().cloned().collect::<HashSet<_>>();
    let created_after = (Utc::now() - Duration::days(180)).to_rfc3339();
    let mut meetings = Vec::new();
    let mut cursor = None;
    for _ in 0..MAX_PAGES {
        let page = client
            .list_meetings(&created_after, cursor.as_deref())
            .await
            .map_err(|error| format!("could not list Fathom meetings: {error}"))?;
        meetings.extend(page.items);
        let next = page.next_cursor.filter(|token| !token.is_empty());
        if next.as_ref() == cursor.as_ref() {
            break;
        }
        match next {
            Some(token) => cursor = Some(token),
            None => break,
        }
    }

    let mut files = Vec::new();
    let mut without_content = 0;
    for meeting in meetings {
        let Some(recording_id) = meeting.recording_id().map(ToOwned::to_owned) else {
            continue;
        };
        if known.contains(&recording_id) {
            continue;
        }
        let imported = import_one(client, &meeting).await?;
        if !meeting_has_content(&imported) {
            without_content += 1;
            continue;
        }
        files.push(meeting_file("fathom", &imported)?);
    }

    let mut warnings = Vec::new();
    if files.is_empty() && without_content == 0 {
        warnings
            .push("Fathom did not return any accessible meetings for this account.".to_string());
    }
    if without_content > 0 {
        warnings.push(format!(
            "{without_content} Fathom meetings did not include notes or transcripts."
        ));
    }
    Ok(ImportResult { files, warnings })
}

async fn import_one(
    client: &FathomClient<anlg_nango::OwnedNangoHttpClient>,
    meeting: &FathomMeeting,
) -> Result<anlg_meeting_import::ImportedMeeting, String> {
    let recording_id = meeting
        .recording_id()
        .ok_or_else(|| "Fathom meeting is missing a recording id".to_string())?;
    let summary = client
        .get_summary(recording_id)
        .await
        .map_err(|error| format!("could not read a Fathom summary: {error}"))?;
    let transcript = client
        .get_transcript(recording_id)
        .await
        .map_err(|error| format!("could not read a Fathom transcript: {error}"))?;
    meeting
        .imported(summary, transcript)
        .ok_or_else(|| "Fathom meeting is missing a recording id".to_string())
}
