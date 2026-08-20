use std::collections::HashSet;

use anlg_meeting_import::{meeting_file, meeting_has_content, webex::WebexClient};
use anlg_nango::OwnedNangoProxy;

use super::{ImportResult, MAX_PAGES, download_vtt};

pub async fn import_meetings(
    client: &WebexClient<anlg_nango::OwnedNangoHttpClient>,
    proxy: &OwnedNangoProxy,
    known_meeting_ids: &[String],
) -> Result<ImportResult, String> {
    let known = known_meeting_ids.iter().cloned().collect::<HashSet<_>>();
    let mut transcripts = Vec::new();
    for page_index in 0..MAX_PAGES {
        let page = client
            .list_transcripts((page_index as u32) * 100)
            .await
            .map_err(|error| format!("could not list Webex transcripts: {error}"))?;
        let count = page.items.len();
        transcripts.extend(page.items);
        if count < 100 {
            break;
        }
    }

    let mut files = Vec::new();
    let mut without_content = 0;
    for item in transcripts {
        let Some(id) = item.transcript_id().map(ToOwned::to_owned) else {
            continue;
        };
        if known.contains(&id) {
            continue;
        }
        let segments = match item.vtt_download_link() {
            Some(url) => download_vtt(proxy, url).await,
            None => download_transcript_by_id(proxy, &id).await,
        };
        let Some(imported) = item.imported(segments) else {
            continue;
        };
        if !meeting_has_content(&imported) {
            without_content += 1;
            continue;
        }
        files.push(meeting_file("webex", &imported)?);
    }

    let mut warnings = Vec::new();
    if files.is_empty() && without_content == 0 {
        warnings.push(
            "Webex did not return any accessible transcripts. Transcripts exist only when recording, Webex Assistant, or captions were enabled.".to_string(),
        );
    }
    if without_content > 0 {
        warnings.push(format!(
            "{without_content} Webex transcripts could not be downloaded for this account."
        ));
    }
    Ok(ImportResult { files, warnings })
}

async fn download_transcript_by_id(
    proxy: &OwnedNangoProxy,
    transcript_id: &str,
) -> Vec<anlg_meeting_import::TranscriptSegment> {
    let path = format!(
        "/v1/meetingTranscripts/{}/download?format=vtt",
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
