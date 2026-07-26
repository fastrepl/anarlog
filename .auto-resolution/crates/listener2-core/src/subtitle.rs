use aspasia::{Subtitle as SubtitleTrait, TimedSubtitleFile, WebVttSubtitle};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Token {
    text: String,
    start_time: u64,
    end_time: u64,
    speaker: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct VttWord {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub speaker: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Subtitle {
    tokens: Vec<Token>,
}

impl From<TimedSubtitleFile> for Subtitle {
    fn from(sub: TimedSubtitleFile) -> Self {
        let vtt: WebVttSubtitle = sub.into();

        let tokens = vtt
            .events()
            .iter()
            .map(|cue| Token {
                text: cue.text.clone(),
                start_time: i64::from(cue.start) as u64,
                end_time: i64::from(cue.end) as u64,
                speaker: cue.identifier.as_ref().filter(|s| !s.is_empty()).cloned(),
            })
            .collect();

        Self { tokens }
    }
}

pub fn parse_subtitle_from_path<P: AsRef<std::path::Path>>(
    path: P,
) -> std::result::Result<Subtitle, String> {
    let sub = TimedSubtitleFile::new(path.as_ref()).map_err(|e| e.to_string())?;
    Ok(sub.into())
}

pub fn export_words_to_vtt_file<P: AsRef<std::path::Path>>(
    words: Vec<VttWord>,
    path: P,
) -> std::result::Result<(), String> {
    use aspasia::{Moment, webvtt::WebVttCue};

    let cues: Vec<WebVttCue> = words
        .into_iter()
        .map(|word| {
            let start_i64 = i64::try_from(word.start_ms)
                .map_err(|_| format!("start_ms {} exceeds i64::MAX", word.start_ms))?;
            let end_i64 = i64::try_from(word.end_ms)
                .map_err(|_| format!("end_ms {} exceeds i64::MAX", word.end_ms))?;

            Ok(WebVttCue {
                identifier: word.speaker,
                text: word.text,
                settings: None,
                start: Moment::from(start_i64),
                end: Moment::from(end_i64),
            })
        })
        .collect::<Result<_, String>>()?;

    let vtt = WebVttSubtitle::builder().cues(cues).build();
    vtt.export(path.as_ref()).map_err(|e| e.to_string())?;
    Ok(())
}
