mod batch;
mod message;
mod recorded;
mod response;
mod streaming;

pub use recorded::*;
pub use streaming::*;

use std::path::Path;
use std::time::Duration;

use hypr_transcribe_core::TARGET_SAMPLE_RATE;
use owhisper_interface::ListenParams;
use owhisper_interface::stream::{Extra, Metadata, ModelInfo};

pub(crate) const DEFAULT_REDEMPTION_TIME: Duration = Duration::from_millis(400);

#[derive(Debug, Clone)]
pub(crate) struct Segment {
    pub text: String,
    pub start: f64,
    pub duration: f64,
    pub confidence: f64,
    pub language: Option<String>,
}

pub(crate) fn parse_listen_params(query: &str) -> Result<ListenParams, serde_html_form::de::Error> {
    serde_html_form::from_str(query)
}

pub(crate) fn build_metadata(model_path: &Path) -> Metadata {
    let model_name = model_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("whisper-local")
        .to_string();

    Metadata {
        model_info: ModelInfo {
            name: model_name,
            version: "1.0".to_string(),
            arch: "whisper-local".to_string(),
        },
        extra: Some(Extra::default().into()),
        ..Default::default()
    }
}

pub(crate) fn redemption_time(params: &ListenParams) -> Duration {
    params
        .custom_query
        .as_ref()
        .and_then(|q| q.get("redemption_time_ms"))
        .and_then(|v| v.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or(DEFAULT_REDEMPTION_TIME)
}

pub(crate) fn build_model(
    loaded_model: &hypr_whisper_local::LoadedWhisper,
    params: &ListenParams,
) -> Result<hypr_whisper_local::Whisper, crate::Error> {
    build_model_with_languages(
        loaded_model,
        params
            .languages
            .iter()
            .filter_map(|lang| lang.clone().try_into().ok())
            .collect(),
    )
}

pub(crate) fn load_model(
    model_path: &Path,
) -> Result<hypr_whisper_local::LoadedWhisper, crate::Error> {
    hypr_whisper_local::LoadedWhisper::builder()
        .model_path(model_path.to_string_lossy().into_owned())
        .build()
        .map_err(crate::Error::from)
}

pub(crate) fn build_model_with_languages(
    loaded_model: &hypr_whisper_local::LoadedWhisper,
    languages: Vec<hypr_whisper::Language>,
) -> Result<hypr_whisper_local::Whisper, crate::Error> {
    loaded_model.session(languages).map_err(crate::Error::from)
}

pub(crate) fn transcribe_chunk(
    model: &mut hypr_whisper_local::Whisper,
    samples: &[f32],
    chunk_start_sec: f64,
) -> Result<Vec<Segment>, crate::Error> {
    let raw_segments = model.transcribe(samples)?;
    let chunk_duration_sec = samples.len() as f64 / TARGET_SAMPLE_RATE as f64;

    Ok(build_chunk_segments(
        raw_segments,
        chunk_start_sec,
        chunk_duration_sec,
    ))
}

fn build_chunk_segments(
    raw_segments: Vec<hypr_whisper_local::Segment>,
    chunk_start_sec: f64,
    chunk_duration_sec: f64,
) -> Vec<Segment> {
    if chunk_duration_sec <= 0.0 {
        return vec![];
    }

    let mut text_parts = Vec::new();
    let mut confidence_sum = 0.0;
    let mut confidence_count = 0usize;
    let mut language = None;

    for segment in raw_segments {
        let text = segment.text().trim();
        if text.is_empty() {
            continue;
        }

        text_parts.push(text.to_string());
        confidence_sum += segment.confidence() as f64;
        confidence_count += 1;
        if language.is_none() {
            language = segment.language().map(|value| value.to_string());
        }
    }

    if text_parts.is_empty() {
        return vec![];
    }

    vec![Segment {
        text: text_parts.join(" "),
        start: chunk_start_sec,
        duration: chunk_duration_sec,
        confidence: confidence_sum / confidence_count as f64,
        language,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_language() {
        let params = parse_listen_params("language=en").unwrap();
        assert_eq!(params.languages.len(), 1);
        assert_eq!(params.languages[0].iso639().code(), "en");
    }

    #[test]
    fn parse_multiple_languages() {
        let params = parse_listen_params("language=en&language=ko").unwrap();
        assert_eq!(params.languages.len(), 2);
        assert_eq!(params.languages[0].iso639().code(), "en");
        assert_eq!(params.languages[1].iso639().code(), "ko");
    }

    #[test]
    fn parse_no_languages() {
        let params = parse_listen_params("").unwrap();
        assert!(params.languages.is_empty());
    }

    #[test]
    fn parse_with_keywords() {
        let params = parse_listen_params("language=en&keywords=hello&keywords=world").unwrap();
        assert_eq!(params.languages.len(), 1);
        assert_eq!(params.keywords, vec!["hello", "world"]);
    }

    #[test]
    fn defaults_channels_and_sample_rate_when_omitted() {
        let params = parse_listen_params("language=en").unwrap();
        assert_eq!(params.channels, 1);
        assert_eq!(params.sample_rate, TARGET_SAMPLE_RATE);
    }

    #[test]
    fn builds_single_chunk_segment_from_multiple_raw_segments() {
        let segments = build_chunk_segments(
            vec![
                hypr_whisper_local::Segment {
                    text: "hello".to_string(),
                    language: Some("en".to_string()),
                    confidence: 0.8,
                    ..Default::default()
                },
                hypr_whisper_local::Segment {
                    text: "again".to_string(),
                    language: Some("en".to_string()),
                    confidence: 1.0,
                    ..Default::default()
                },
            ],
            10.0,
            3.0,
        );

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].start, 10.0);
        assert_eq!(segments[0].duration, 3.0);
        assert_eq!(segments[0].text, "hello again");
        assert_eq!(segments[0].language.as_deref(), Some("en"));
        assert!((segments[0].confidence - 0.9).abs() < 1e-6);
    }
}
