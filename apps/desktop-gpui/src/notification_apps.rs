//! `settings/general/notification-app-options.ts`: which installed apps the
//! Notifications page lists as excludable from microphone detection, which
//! it shows as excluded, and how a toggle rewrites the two settings.

use anlg_detect::InstalledApp;

/// `isAppIgnored`: user-ignored, or ignored by default and not re-included.
fn is_app_ignored(
    bundle_id: &str,
    ignored_platforms: &[String],
    included_platforms: &[String],
    default_ignored: &[String],
) -> bool {
    let is_default_ignored = default_ignored.iter().any(|id| id == bundle_id);
    let is_included = included_platforms.iter().any(|id| id == bundle_id);
    let is_user_ignored = ignored_platforms.iter().any(|id| id == bundle_id);
    is_user_ignored || (is_default_ignored && !is_included)
}

/// `getIgnorableApps`: installed apps not yet ignored whose name matches the
/// query (case-insensitive substring; every app for an empty query).
pub fn ignorable_apps<'a>(
    installed: &'a [InstalledApp],
    ignored_platforms: &[String],
    included_platforms: &[String],
    query: &str,
    default_ignored: &[String],
) -> Vec<&'a InstalledApp> {
    let query = query.trim().to_lowercase();
    installed
        .iter()
        .filter(|app| app.name.to_lowercase().contains(&query))
        .filter(|app| {
            !is_app_ignored(
                &app.id,
                ignored_platforms,
                included_platforms,
                default_ignored,
            )
        })
        .collect()
}

/// `getIgnoredBundleIds`: the user's ignored ids, then the installed default
/// ignores that are not re-included (insertion order, deduplicated).
pub fn ignored_bundle_ids(
    installed: &[InstalledApp],
    ignored_platforms: &[String],
    included_platforms: &[String],
    default_ignored: &[String],
) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for id in ignored_platforms {
        if !ids.contains(id) {
            ids.push(id.clone());
        }
    }
    for id in default_ignored {
        if !installed.iter().any(|app| &app.id == id) {
            continue;
        }
        if is_app_ignored(id, ignored_platforms, included_platforms, default_ignored)
            && !ids.contains(id)
        {
            ids.push(id.clone());
        }
    }
    ids
}

/// `toggleIgnoredApp`: the next `(ignored_platforms, included_platforms)`.
pub fn toggle_ignored_app(
    bundle_id: &str,
    ignored_platforms: &[String],
    included_platforms: &[String],
    default_ignored: &[String],
) -> (Vec<String>, Vec<String>) {
    let is_ignored = is_app_ignored(
        bundle_id,
        ignored_platforms,
        included_platforms,
        default_ignored,
    );
    let is_default = default_ignored.iter().any(|id| id == bundle_id);
    if is_ignored {
        // Un-ignore: drop it from the user list and, for a default ignore,
        // record it as included.
        let ignored = ignored_platforms
            .iter()
            .filter(|id| id.as_str() != bundle_id)
            .cloned()
            .collect();
        let mut included = included_platforms.to_vec();
        if is_default {
            included.push(bundle_id.to_string());
        }
        (ignored, included)
    } else {
        let mut ignored = ignored_platforms.to_vec();
        if !is_default {
            ignored.push(bundle_id.to_string());
        }
        let included = included_platforms
            .iter()
            .filter(|id| id.as_str() != bundle_id)
            .cloned()
            .collect();
        (ignored, included)
    }
}

/// The `ignored_platforms` / `included_platforms` settings: JSON arrays
/// stored as strings (`JSON.stringify(value)`), `[]` by default.
pub fn parse_platforms(value: Option<&str>) -> Vec<String> {
    value
        .and_then(|json| serde_json::from_str::<Vec<String>>(json).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str, name: &str) -> InstalledApp {
        InstalledApp {
            id: id.to_string(),
            name: name.to_string(),
        }
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn hides_default_ignored_apps_from_the_matches() {
        let installed = [app("com.microsoft.VSCode", "VS Code")];
        let defaults = strings(&["com.microsoft.VSCode"]);
        assert!(ignorable_apps(&installed, &[], &[], "code", &defaults).is_empty());
        let included = strings(&["com.microsoft.VSCode"]);
        assert_eq!(
            ignorable_apps(&installed, &[], &included, "code", &defaults)
                .iter()
                .map(|app| app.id.as_str())
                .collect::<Vec<_>>(),
            ["com.microsoft.VSCode"]
        );
    }

    #[test]
    fn lists_installed_default_ignores_unless_included() {
        let installed = [
            app("com.microsoft.VSCode", "VS Code"),
            app("us.zoom.xos", "Zoom Workplace"),
        ];
        let defaults = strings(&["com.microsoft.VSCode"]);
        assert_eq!(
            ignored_bundle_ids(&installed, &[], &[], &defaults),
            strings(&["com.microsoft.VSCode"])
        );
        let included = strings(&["com.microsoft.VSCode"]);
        assert!(ignored_bundle_ids(&installed, &[], &included, &defaults).is_empty());
        let ignored = strings(&["us.zoom.xos"]);
        assert_eq!(
            ignored_bundle_ids(&installed, &ignored, &[], &defaults),
            strings(&["us.zoom.xos", "com.microsoft.VSCode"])
        );
    }

    #[test]
    fn toggling_moves_between_the_two_lists() {
        let defaults = strings(&["com.microsoft.VSCode"]);
        // Ignoring a regular app adds it to ignored_platforms.
        assert_eq!(
            toggle_ignored_app("us.zoom.xos", &[], &[], &defaults),
            (strings(&["us.zoom.xos"]), vec![])
        );
        // Un-ignoring it removes it again.
        assert_eq!(
            toggle_ignored_app("us.zoom.xos", &strings(&["us.zoom.xos"]), &[], &defaults),
            (vec![], vec![])
        );
        // Un-ignoring a default ignore records it as included …
        assert_eq!(
            toggle_ignored_app("com.microsoft.VSCode", &[], &[], &defaults),
            (vec![], strings(&["com.microsoft.VSCode"]))
        );
        // … and ignoring it again only drops the inclusion.
        assert_eq!(
            toggle_ignored_app(
                "com.microsoft.VSCode",
                &[],
                &strings(&["com.microsoft.VSCode"]),
                &defaults
            ),
            (vec![], vec![])
        );
    }

    #[test]
    fn platform_settings_are_json_strings() {
        assert_eq!(parse_platforms(Some(r#"["a","b"]"#)), strings(&["a", "b"]));
        assert!(parse_platforms(Some("[]")).is_empty());
        assert!(parse_platforms(None).is_empty());
        assert!(parse_platforms(Some("nonsense")).is_empty());
    }
}
