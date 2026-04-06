#![cfg(target_os = "macos")]

use std::process::Command;

use objc2_app_kit::{NSRunningApplication, NSWorkspace};

#[derive(Debug, Clone)]
pub(crate) struct FrontmostApplication {
    pub pid: i32,
    pub app_name: String,
    pub bundle_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedFrontmostApplication {
    pid: i32,
    bundle_id: Option<String>,
}

pub(crate) fn resolve() -> Option<FrontmostApplication> {
    resolve_via_lsappinfo().or_else(resolve_via_workspace)
}

fn resolve_via_lsappinfo() -> Option<FrontmostApplication> {
    let front_output = Command::new("/usr/bin/lsappinfo")
        .arg("front")
        .output()
        .ok()
        .and_then(|output| output.status.success().then_some(output.stdout))
        .and_then(|stdout| String::from_utf8(stdout).ok())?;
    let asn = parse_front_asn(&front_output)?.to_string();

    let info = Command::new("/usr/bin/lsappinfo")
        .args(["info", "-only", "pid", "-only", "bundleID", &asn])
        .output()
        .ok()
        .and_then(|output| output.status.success().then_some(output.stdout))
        .and_then(|stdout| String::from_utf8(stdout).ok())?;
    let parsed = parse_frontmost_application(&front_output, &info)?;
    let pid = parsed.pid;
    let bundle_id = parsed.bundle_id;

    if let Some(application) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
        return from_running_application(&application).or_else(|| {
            Some(FrontmostApplication {
                pid,
                app_name: bundle_id.clone().unwrap_or_else(|| pid.to_string()),
                bundle_id,
            })
        });
    }

    Some(FrontmostApplication {
        pid,
        app_name: bundle_id.clone().unwrap_or_else(|| pid.to_string()),
        bundle_id,
    })
}

fn resolve_via_workspace() -> Option<FrontmostApplication> {
    let workspace = NSWorkspace::sharedWorkspace();
    let application = workspace.frontmostApplication()?;
    from_running_application(&application)
}

fn from_running_application(application: &NSRunningApplication) -> Option<FrontmostApplication> {
    if application.isHidden() {
        return None;
    }

    let pid = application.processIdentifier();
    let bundle_id = application
        .bundleIdentifier()
        .map(|value| value.to_string())
        .filter(|value| !value.is_empty());
    let app_name = application
        .localizedName()
        .map(|value| value.to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| bundle_id.clone())
        .unwrap_or_else(|| pid.to_string());

    Some(FrontmostApplication {
        pid,
        app_name,
        bundle_id,
    })
}

fn parse_front_asn(output: &str) -> Option<&str> {
    output
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("ASN:"))
        .filter(|line| !line.is_empty())
}

fn parse_pid(output: &str) -> Option<i32> {
    output.lines().find_map(|line| {
        let (_, value) = line.split_once("\"pid\"=")?;
        value.trim().parse::<i32>().ok()
    })
}

fn parse_bundle_id(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let (_, value) = line.split_once("\"CFBundleIdentifier\"=")?;
        let value = value.trim().trim_matches('"');
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn parse_frontmost_application(
    front_output: &str,
    info_output: &str,
) -> Option<ParsedFrontmostApplication> {
    let _ = parse_front_asn(front_output)?;

    Some(ParsedFrontmostApplication {
        pid: parse_pid(info_output)?,
        bundle_id: parse_bundle_id(info_output),
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_bundle_id, parse_front_asn, parse_frontmost_application, parse_pid};

    #[test]
    fn parses_front_asn() {
        assert_eq!(
            parse_front_asn("ASN:0x0-0x11011:\n"),
            Some("ASN:0x0-0x11011:")
        );
    }

    #[test]
    fn parses_pid_and_bundle_id() {
        let output = "\"CFBundleIdentifier\"=\"com.example.App\"\n\"pid\"=123\n";

        assert_eq!(parse_pid(output), Some(123));
        assert_eq!(parse_bundle_id(output), Some("com.example.App".to_string()));
    }

    #[test]
    fn rejects_malformed_lsappinfo_front_output() {
        let info_output = "\"CFBundleIdentifier\"=\"com.example.App\"\n\"pid\"=123\n";

        assert_eq!(parse_frontmost_application("not-an-asn", info_output), None);
    }

    #[test]
    fn rejects_malformed_lsappinfo_info_output() {
        let front_output = "ASN:0x0-0x11011:\n";

        assert_eq!(
            parse_frontmost_application(front_output, "\"pid\"=abc\n"),
            None
        );
    }
}
