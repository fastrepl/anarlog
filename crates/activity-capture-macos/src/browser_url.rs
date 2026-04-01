#![cfg(target_os = "macos")]

use crate::{app_profile::AppProfile, apple_script};

#[derive(Debug, Clone, Copy, Default)]
pub struct BrowserUrlResolver;

impl BrowserUrlResolver {
    pub fn supports_profile(self, profile: AppProfile) -> bool {
        profile.is_browser()
    }

    pub fn current_url(self, profile: AppProfile) -> Option<String> {
        let script = match profile {
            AppProfile::Safari => Some(
                r#"
                tell application "Safari"
                    if (count of windows) is 0 then return ""
                    return URL of current tab of front window
                end tell
                "#,
            )
            .map(str::to_string),
            AppProfile::Chrome | AppProfile::Arc | AppProfile::Brave | AppProfile::Edge => {
                let bundle_id = profile.browser_bundle_id()?;
                Some(format!(
                    r#"
                tell application id "{bundle_id}"
                    if (count of windows) is 0 then return ""
                    return URL of active tab of front window
                end tell
                "#
                ))
            }
            _ => None,
        }?;

        apple_script::run(&script)
    }

    pub fn front_window_is_private(self, profile: AppProfile) -> Option<bool> {
        if !profile.supports_private_window_detection() {
            return None;
        }
        let bundle_id = profile.browser_bundle_id()?;
        let script = format!(
            r#"
            tell application id "{bundle_id}"
                if (count of windows) is 0 then return ""
                return mode of front window
            end tell
            "#
        );

        apple_script::run(&script).and_then(|value| parse_private_window_mode(&value))
    }
}

fn parse_private_window_mode(mode: &str) -> Option<bool> {
    let normalized = mode.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "incognito" | "private" => Some(true),
        "normal" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_private_window_mode;

    #[test]
    fn parse_private_window_mode_detects_private_states() {
        assert_eq!(parse_private_window_mode("incognito"), Some(true));
        assert_eq!(parse_private_window_mode("private"), Some(true));
        assert_eq!(parse_private_window_mode("normal"), Some(false));
        assert_eq!(parse_private_window_mode(""), None);
    }
}
