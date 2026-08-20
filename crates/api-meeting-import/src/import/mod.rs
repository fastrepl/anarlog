use anlg_meeting_import::parse_vtt;
use anlg_nango::OwnedNangoProxy;
use url::Url;

pub mod fathom;
pub mod google_meet;
pub mod teams;
pub mod webex;

pub use anlg_meeting_import::ImportFile;

pub struct ImportResult {
    pub files: Vec<ImportFile>,
    pub warnings: Vec<String>,
}

const MAX_PAGES: usize = 20;
const LOOKBACK_WINDOWS: i64 = 6;
const WINDOW_DAYS: i64 = 29;

fn recording_windows() -> Vec<(chrono::NaiveDate, chrono::NaiveDate)> {
    let mut end = chrono::Utc::now().date_naive();
    let mut windows = Vec::with_capacity(LOOKBACK_WINDOWS as usize);
    for _ in 0..LOOKBACK_WINDOWS {
        let start = end - chrono::Duration::days(WINDOW_DAYS);
        windows.push((start, end));
        end = start - chrono::Duration::days(1);
    }
    windows
}

async fn download_vtt(
    proxy: &OwnedNangoProxy,
    download_url: &str,
) -> Vec<anlg_meeting_import::TranscriptSegment> {
    let Ok(url) = Url::parse(download_url) else {
        return Vec::new();
    };
    let origin = format!(
        "{}://{}",
        url.scheme(),
        url.host_str().unwrap_or("localhost")
    );
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
