use anlg_nango::OwnedNangoProxy;
use anlg_zoom::{
    RecordingMeeting, TranscriptSegment, ZoomClient, meeting_has_content, meeting_json, parse_vtt,
};
use chrono::{Duration, Utc};
use serde_json::Value;
use std::collections::HashSet;
use url::Url;

const RECORDING_WINDOWS: i64 = 6;
const RECORDING_WINDOW_DAYS: i64 = 29;
const MAX_PAGES_PER_WINDOW: usize = 20;

pub struct ZoomImportFile {
    pub path: String,
    pub name: String,
    pub content: String,
}

pub struct ZoomImportResult {
    pub files: Vec<ZoomImportFile>,
    pub warnings: Vec<String>,
}

pub async fn import_meetings(
    client: &ZoomClient<anlg_nango::OwnedNangoHttpClient>,
    proxy: &OwnedNangoProxy,
    known_meeting_ids: &[String],
) -> Result<ZoomImportResult, String> {
    let known = known_meeting_ids.iter().cloned().collect::<HashSet<_>>();
    let mut recordings = Vec::new();
    let mut warnings = Vec::new();

    for (from, to) in recording_windows() {
        let mut next_page_token = None;
        for _ in 0..MAX_PAGES_PER_WINDOW {
            let page = client
                .list_user_recordings(from, to, next_page_token.as_deref())
                .await
                .map_err(|error| format!("could not list Zoom recordings: {error}"))?;
            recordings.extend(page.meetings);
            let next = page.next_page_token.filter(|token| !token.is_empty());
            if next.as_ref() == next_page_token.as_ref() {
                break;
            }
            match next {
                Some(token) => next_page_token = Some(token),
                None => break,
            }
        }
    }

    let mut files = Vec::new();
    let mut without_content = 0;
    for recording in recordings {
        let Some(external_id) = recording.external_id() else {
            continue;
        };
        if known.contains(&external_id) {
            continue;
        }

        let summary = match recording.meeting_id() {
            Some(meeting_id) => client
                .get_meeting_summary(&meeting_id)
                .await
                .map_err(|error| format!("could not read a Zoom meeting summary: {error}"))?,
            None => None,
        };
        let transcript = match recording
            .transcript_file()
            .and_then(|file| file.download_url.as_deref().filter(|url| !url.is_empty()))
        {
            Some(download_url) => download_transcript(proxy, download_url).await,
            None => Vec::new(),
        };

        if !meeting_has_content(summary.as_ref(), &transcript) {
            without_content += 1;
            continue;
        }

        let Some(meeting) = meeting_json(&recording, summary.as_ref(), transcript) else {
            continue;
        };
        files.push(meeting_file(&recording, meeting)?);
    }

    if files.is_empty() && without_content == 0 {
        warnings.push(
            "Zoom did not return any accessible cloud recordings for this account.".to_string(),
        );
    }
    if without_content > 0 {
        warnings.push(format!(
            "{without_content} Zoom recordings did not include notes or transcripts for this account or plan."
        ));
    }

    Ok(ZoomImportResult { files, warnings })
}

fn meeting_file(recording: &RecordingMeeting, meeting: Value) -> Result<ZoomImportFile, String> {
    let id = recording
        .external_id()
        .ok_or_else(|| "Zoom recording is missing an id".to_string())?;
    let safe_id = safe_file_component(&id);
    Ok(ZoomImportFile {
        path: format!("oauth://zoom/{safe_id}.json"),
        name: format!("{safe_id}.json"),
        content: serde_json::to_string(&meeting)
            .map_err(|error| format!("could not save a Zoom meeting: {error}"))?,
    })
}

fn recording_windows() -> Vec<(chrono::NaiveDate, chrono::NaiveDate)> {
    let mut end = Utc::now().date_naive();
    let mut windows = Vec::with_capacity(RECORDING_WINDOWS as usize);
    for _ in 0..RECORDING_WINDOWS {
        let start = end - Duration::days(RECORDING_WINDOW_DAYS);
        windows.push((start, end));
        end = start - Duration::days(1);
    }
    windows
}

async fn download_transcript(
    proxy: &OwnedNangoProxy,
    download_url: &str,
) -> Vec<TranscriptSegment> {
    let Ok(url) = Url::parse(download_url) else {
        return Vec::new();
    };
    let origin = format!("{}://{}", url.scheme(), url.host_str().unwrap_or("zoom.us"));
    let path = match url.query() {
        Some(query) => format!("{}?{query}", url.path()),
        None => url.path().to_string(),
    };
    let response = match proxy.clone().base_url_override(origin).get(&path) {
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
    parse_vtt(&body)
}

fn safe_file_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "meeting".to_string()
    } else {
        sanitized
    }
}
