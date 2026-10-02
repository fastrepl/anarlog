use objc2_app_kit::NSWorkspace;

/// Owned identity of a running application, safe to hold beyond the query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunningApplicationIdentity {
    pub pid: i32,
    pub bundle_identifier: Option<String>,
}

impl RunningApplicationIdentity {
    pub fn matches_pid(&self, pid: i32) -> bool {
        self.pid == pid
    }

    pub fn matches_bundle_identifier(&self, bundle_identifier: &str) -> bool {
        self.bundle_identifier.as_deref() == Some(bundle_identifier)
    }
}

pub fn frontmost_application_identity() -> Option<RunningApplicationIdentity> {
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    identity_from_parts(
        app.processIdentifier(),
        app.bundleIdentifier().map(|value| value.to_string()),
    )
}

fn identity_from_parts(
    pid: i32,
    bundle_identifier: Option<String>,
) -> Option<RunningApplicationIdentity> {
    if pid <= 0 {
        return None;
    }
    Some(RunningApplicationIdentity {
        pid,
        bundle_identifier,
    })
}

#[cfg(test)]
mod tests {
    use super::identity_from_parts;

    #[test]
    fn nonpositive_pids_are_rejected() {
        assert!(identity_from_parts(0, None).is_none());
        assert!(identity_from_parts(-1, Some("com.example".into())).is_none());
        assert!(identity_from_parts(1, None).is_some());
    }
}
