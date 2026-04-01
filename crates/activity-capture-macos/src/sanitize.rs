#![cfg(target_os = "macos")]

use crate::app_profile::AppProfile;

pub(crate) fn sanitize_snapshot_fields(
    app_name: &str,
    bundle_id: Option<&str>,
    window_title: Option<String>,
    visible_text: Option<String>,
) -> (Option<String>, Option<String>) {
    let profile = AppProfile::from_bundle_id(bundle_id);
    let normalized_title = normalize_window_title(window_title.as_deref(), app_name, profile);
    let filtered_lines = visible_text
        .as_deref()
        .map(|text| filter_lines(text, app_name, profile, normalized_title.as_deref()))
        .unwrap_or_default();

    let window_title = fallback_title(normalized_title, &filtered_lines, app_name);
    let visible_lines =
        filter_duplicate_title_lines(filtered_lines, window_title.as_deref(), app_name, profile);
    let visible_text = (!visible_lines.is_empty()).then(|| visible_lines.join("\n"));

    (window_title, visible_text)
}

fn normalize_window_title(
    window_title: Option<&str>,
    app_name: &str,
    profile: AppProfile,
) -> Option<String> {
    let trimmed = window_title
        .map(str::trim)
        .filter(|value| !value.is_empty())?;

    match profile {
        AppProfile::Slack => Some(normalize_slack_title(trimmed)),
        AppProfile::Generic
        | AppProfile::Safari
        | AppProfile::Chrome
        | AppProfile::Arc
        | AppProfile::Brave
        | AppProfile::Edge => Some(
            strip_app_name_suffix(trimmed, app_name)
                .unwrap_or(trimmed)
                .to_string(),
        ),
        AppProfile::Spotify => Some(trimmed.to_string()),
    }
}

fn fallback_title(
    current_title: Option<String>,
    filtered_lines: &[String],
    app_name: &str,
) -> Option<String> {
    match current_title {
        Some(title) if !is_generic_title(&title, app_name) => Some(title),
        Some(title) => filtered_lines.first().cloned().or(Some(title)),
        None => filtered_lines.first().cloned(),
    }
}

fn normalize_slack_title(title: &str) -> String {
    if !title.ends_with(" - Slack") {
        return title.to_string();
    }

    let without_suffix = &title[..title.len() - " - Slack".len()];
    without_suffix
        .split(" - ")
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or(without_suffix)
        .to_string()
}

fn strip_app_name_suffix<'a>(title: &'a str, app_name: &str) -> Option<&'a str> {
    let app_name = app_name.trim();
    if app_name.is_empty() {
        return None;
    }

    title
        .strip_suffix(app_name)
        .and_then(|value| value.strip_suffix(" - "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn filter_lines(
    visible_text: &str,
    app_name: &str,
    profile: AppProfile,
    title: Option<&str>,
) -> Vec<String> {
    let title_key = title.map(normalized_comparison_key);
    let app_key = normalized_comparison_key(app_name);
    let mut seen = std::collections::BTreeSet::new();
    let mut lines = Vec::new();

    for line in visible_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let normalized = normalized_comparison_key(trimmed);
        if normalized.is_empty() {
            continue;
        }
        if title_key.as_deref() == Some(normalized.as_str()) || app_key == normalized {
            continue;
        }

        let normalized_line_title = normalize_window_title(Some(trimmed), app_name, profile)
            .map(|value| normalized_comparison_key(&value))
            .unwrap_or_default();
        if title_key.as_deref() == Some(normalized_line_title.as_str()) {
            continue;
        }
        if is_low_signal_line(&normalized, profile) {
            continue;
        }
        if seen.insert(normalized) {
            lines.push(trimmed.to_string());
        }
    }

    lines
}

fn filter_duplicate_title_lines(
    lines: Vec<String>,
    title: Option<&str>,
    app_name: &str,
    profile: AppProfile,
) -> Vec<String> {
    let title_key = title.map(normalized_comparison_key);

    lines
        .into_iter()
        .filter(|line| {
            let normalized = normalized_comparison_key(line);
            if title_key.as_deref() == Some(normalized.as_str()) {
                return false;
            }

            let normalized_line_title = normalize_window_title(Some(line), app_name, profile)
                .map(|value| normalized_comparison_key(&value))
                .unwrap_or_default();
            title_key.as_deref() != Some(normalized_line_title.as_str())
        })
        .collect()
}

fn is_generic_title(title: &str, app_name: &str) -> bool {
    let title = normalized_comparison_key(title);
    !title.is_empty() && title == normalized_comparison_key(app_name)
}

fn is_low_signal_line(line: &str, profile: AppProfile) -> bool {
    const BOILERPLATE_LINES: [&str; 10] = [
        "add page to reading list",
        "downloads window",
        "hide sidebar",
        "page menu",
        "pin window",
        "show sidebar",
        "smart search field",
        "start meeting recording",
        "tab group picker",
        "tauri react typescript",
    ];

    if BOILERPLATE_LINES.contains(&line) {
        return true;
    }

    profile.is_slack() && line == "slack"
}

fn normalized_comparison_key(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::sanitize_snapshot_fields;

    #[test]
    fn normalizes_slack_title_and_removes_duplicate_visible_text() {
        let (window_title, visible_text) = sanitize_snapshot_fields(
            "Slack",
            Some("com.tinyspeck.slackmacgap"),
            Some("product (Channel) - Fastrepl - Slack".to_string()),
            Some("product (Channel) - Fastrepl - Slack".to_string()),
        );

        assert_eq!(window_title.as_deref(), Some("product (Channel)"));
        assert_eq!(visible_text.as_deref(), None);
    }

    #[test]
    fn falls_back_to_meaningful_visible_text_when_window_title_is_missing() {
        let (window_title, visible_text) = sanitize_snapshot_fields(
            "Philo",
            Some("com.johnjeong.philo"),
            Some(String::new()),
            Some(
                "Project notes\nStart meeting recording\nPin window\nShip activity tracking fix"
                    .to_string(),
            ),
        );

        assert_eq!(window_title.as_deref(), Some("Project notes"));
        assert_eq!(visible_text.as_deref(), Some("Ship activity tracking fix"));
    }

    #[test]
    fn treats_app_name_titles_as_generic() {
        let (window_title, visible_text) = sanitize_snapshot_fields(
            "Slack",
            Some("com.slack.Slack"),
            Some("Slack".to_string()),
            Some("Roadmap".to_string()),
        );

        assert_eq!(window_title.as_deref(), Some("Roadmap"));
        assert_eq!(visible_text.as_deref(), None);
    }

    #[test]
    fn strips_browser_app_suffixes_from_titles() {
        let (window_title, visible_text) = sanitize_snapshot_fields(
            "Google Chrome",
            Some("com.google.Chrome"),
            Some("Quarterly plan - Google Chrome".to_string()),
            Some("Quarterly plan - Google Chrome\nNext action".to_string()),
        );

        assert_eq!(window_title.as_deref(), Some("Quarterly plan"));
        assert_eq!(visible_text.as_deref(), Some("Next action"));
    }

    #[test]
    fn normalizes_metadata_only_titles_too() {
        let (window_title, visible_text) = sanitize_snapshot_fields(
            "Safari",
            Some("com.apple.Safari"),
            Some("Reading list - Safari".to_string()),
            None,
        );

        assert_eq!(window_title.as_deref(), Some("Reading list"));
        assert_eq!(visible_text, None);
    }
}
