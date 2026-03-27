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

    let raw_segments: Vec<_> = raw_segments
        .into_iter()
        .filter_map(|segment| {
            let text = segment.text().trim().to_string();
            if text.is_empty() {
                return None;
            }

            Some((
                segment.start(),
                segment.end(),
                Segment {
                    text,
                    start: 0.0,
                    duration: 0.0,
                    confidence: segment.confidence() as f64,
                    language: segment.language().map(|value| value.to_string()),
                },
            ))
        })
        .collect();

    if raw_segments.is_empty() {
        return vec![];
    }

    if raw_segments.len() == 1 {
        return vec![Segment {
            start: chunk_start_sec,
            duration: chunk_duration_sec,
            ..raw_segments.into_iter().next().unwrap().2
        }];
    }

    let timings = normalize_raw_segment_timings(&raw_segments, chunk_duration_sec)
        .unwrap_or_else(|| synthetic_segment_timings(&raw_segments, chunk_duration_sec));

    raw_segments
        .into_iter()
        .zip(timings)
        .map(|((_, _, segment), (start_offset, duration))| Segment {
            start: chunk_start_sec + start_offset,
            duration,
            ..segment
        })
        .collect()
}

fn normalize_raw_segment_timings(
    raw_segments: &[(f64, f64, Segment)],
    chunk_duration_sec: f64,
) -> Option<Vec<(f64, f64)>> {
    let mut clamped_bounds = Vec::with_capacity(raw_segments.len());
    let mut previous_end = 0.0;

    for (start, end, _) in raw_segments {
        if !start.is_finite() || !end.is_finite() {
            return None;
        }

        let start = (*start).max(0.0).max(previous_end);
        let end = (*end).max(0.0);
        if end <= start {
            return None;
        }

        clamped_bounds.push((start, end));
        previous_end = end;
    }

    if previous_end <= 0.0 {
        return None;
    }

    let scale = chunk_duration_sec / previous_end;
    let mut timings = Vec::with_capacity(clamped_bounds.len());

    for (idx, (start, end)) in clamped_bounds.into_iter().enumerate() {
        let start = (start * scale).min(chunk_duration_sec);
        let end = if idx + 1 == raw_segments.len() {
            chunk_duration_sec
        } else {
            (end * scale).min(chunk_duration_sec)
        };

        if end <= start {
            return None;
        }

        timings.push((start, end - start));
    }

    Some(timings)
}

fn synthetic_segment_timings(
    raw_segments: &[(f64, f64, Segment)],
    chunk_duration_sec: f64,
) -> Vec<(f64, f64)> {
    let total_weight: usize = raw_segments
        .iter()
        .map(|(_, _, segment)| segment.text.split_whitespace().count().max(1))
        .sum();
    let mut cursor = 0.0;

    raw_segments
        .iter()
        .enumerate()
        .map(|(idx, (_, _, segment))| {
            let weight = segment.text.split_whitespace().count().max(1) as f64;
            let start = cursor;
            let end = if idx + 1 == raw_segments.len() {
                chunk_duration_sec
            } else {
                cursor + chunk_duration_sec * (weight / total_weight as f64)
            };
            cursor = end;
            (start, end - start)
        })
        .collect()
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
    fn preserves_multiple_segments_with_normalized_timings() {
        let segments = build_chunk_segments(
            vec![
                hypr_whisper_local::Segment {
                    text: "hello".to_string(),
                    language: Some("en".to_string()),
                    start: 0.0,
                    end: 1.0,
                    confidence: 0.8,
                    ..Default::default()
                },
                hypr_whisper_local::Segment {
                    text: "again".to_string(),
                    language: Some("en".to_string()),
                    start: 1.5,
                    end: 2.0,
                    confidence: 1.0,
                    ..Default::default()
                },
            ],
            10.0,
            4.0,
        );

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].start, 10.0);
        assert_eq!(segments[0].duration, 2.0);
        assert_eq!(segments[0].text, "hello");
        assert_eq!(segments[0].language.as_deref(), Some("en"));
        assert!((segments[0].confidence - 0.8).abs() < 1e-6);

        assert_eq!(segments[1].start, 13.0);
        assert_eq!(segments[1].duration, 1.0);
        assert_eq!(segments[1].text, "again");
        assert_eq!(segments[1].language.as_deref(), Some("en"));
        assert!((segments[1].confidence - 1.0).abs() < 1e-6);
    }

    #[test]
    fn falls_back_to_synthetic_timings_when_raw_timings_are_invalid() {
        let segments = build_chunk_segments(
            vec![
                hypr_whisper_local::Segment {
                    text: "hello world".to_string(),
                    language: Some("en".to_string()),
                    start: 0.0,
                    end: 0.0,
                    confidence: 0.8,
                    ..Default::default()
                },
                hypr_whisper_local::Segment {
                    text: "again".to_string(),
                    language: Some("en".to_string()),
                    start: 0.0,
                    end: 0.0,
                    confidence: 1.0,
                    ..Default::default()
                },
            ],
            10.0,
            3.0,
        );

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].start, 10.0);
        assert_eq!(segments[0].duration, 2.0);
        assert_eq!(segments[1].start, 12.0);
        assert_eq!(segments[1].duration, 1.0);
    }
}
