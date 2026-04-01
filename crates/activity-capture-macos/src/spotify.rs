#![cfg(target_os = "macos")]

use hypr_activity_capture_interface::CaptureError;

use crate::{apple_script, ax::merge_fragments};

const FIELD_LIMIT: usize = 300;
const SPOTIFY_BUNDLE_ID: &str = "com.spotify.client";

#[derive(Debug, Clone, PartialEq, Eq)]
struct NowPlaying {
    state: String,
    track: Option<String>,
    artist: Option<String>,
    album: Option<String>,
}

pub(crate) fn collect_visible_text() -> Result<String, CaptureError> {
    let Some(raw_output) = run_script(&script_source()) else {
        return Ok(String::new());
    };
    let Some(now_playing) = parse_now_playing(&raw_output) else {
        return Ok(String::new());
    };

    Ok(format_visible_text(&now_playing))
}

fn script_source() -> String {
    format!(
        r#"
        tell application id "{SPOTIFY_BUNDLE_ID}"
            if not running then return ""
            set playbackState to player state as text
            if playbackState is "stopped" then return playbackState
            set currentTrack to current track
            return playbackState & linefeed & (name of currentTrack) & linefeed & (artist of currentTrack) & linefeed & (album of currentTrack)
        end tell
        "#
    )
}

fn parse_now_playing(raw_output: &str) -> Option<NowPlaying> {
    let mut fields = raw_output.lines().filter_map(sanitize_text);
    let state = fields.next()?;

    Some(NowPlaying {
        state,
        track: fields.next(),
        artist: fields.next(),
        album: fields.next(),
    })
}

fn format_visible_text(now_playing: &NowPlaying) -> String {
    let mut fragments = vec![format!("State: {}", now_playing.state)];
    if let Some(track) = now_playing.track.as_deref() {
        fragments.push(format!("Track: {track}"));
    }
    if let Some(artist) = now_playing.artist.as_deref() {
        fragments.push(format!("Artist: {artist}"));
    }
    if let Some(album) = now_playing.album.as_deref() {
        fragments.push(format!("Album: {album}"));
    }

    merge_fragments(fragments)
}

fn sanitize_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let length = trimmed.chars().count();
    if length > FIELD_LIMIT {
        return Some(trimmed.chars().take(FIELD_LIMIT).collect());
    }

    Some(trimmed.to_string())
}

fn run_script(source: &str) -> Option<String> {
    apple_script::run(source)
}

#[cfg(test)]
mod tests {
    use super::{FIELD_LIMIT, NowPlaying, format_visible_text, parse_now_playing, sanitize_text};

    #[test]
    fn parse_now_playing_reads_track_metadata() {
        assert_eq!(
            parse_now_playing(
                "paused\n2/1 - Remastered 2004\nBrian Eno\nAmbient 1: Music For Airports (Remastered 2004)"
            ),
            Some(NowPlaying {
                state: "paused".to_string(),
                track: Some("2/1 - Remastered 2004".to_string()),
                artist: Some("Brian Eno".to_string()),
                album: Some("Ambient 1: Music For Airports (Remastered 2004)".to_string()),
            })
        );
    }

    #[test]
    fn parse_now_playing_allows_state_only_output() {
        assert_eq!(
            parse_now_playing("stopped"),
            Some(NowPlaying {
                state: "stopped".to_string(),
                track: None,
                artist: None,
                album: None,
            })
        );
    }

    #[test]
    fn format_visible_text_omits_missing_fields() {
        assert_eq!(
            format_visible_text(&NowPlaying {
                state: "playing".to_string(),
                track: Some("An Ending (Ascent)".to_string()),
                artist: Some("Brian Eno".to_string()),
                album: None,
            }),
            "State: playing\nTrack: An Ending (Ascent)\nArtist: Brian Eno"
        );
    }

    #[test]
    fn sanitize_text_trims_and_truncates_unicode_safely() {
        assert_eq!(
            sanitize_text("  Brian Eno  "),
            Some("Brian Eno".to_string())
        );
        assert_eq!(
            sanitize_text(&"界".repeat(FIELD_LIMIT + 5)),
            Some("界".repeat(FIELD_LIMIT))
        );
    }
}
