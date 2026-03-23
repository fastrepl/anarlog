use std::hash::{DefaultHasher, Hash, Hasher};
use sysinfo::System;

pub fn cpu_arch() -> String {
    System::cpu_arch()
}

pub fn long_os_version() -> String {
    System::long_os_version().unwrap_or("Unknown".to_string())
}

pub fn fingerprint() -> String {
    let fingerprint = machine_uid::get().unwrap_or_default();

    let mut hasher = DefaultHasher::new();
    fingerprint.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

pub enum ProcessMatcher {
    Name(String),
    Sidecar,
}

pub fn kill_processes_by_matcher(matcher: ProcessMatcher) -> u16 {
    let targets = match matcher {
        ProcessMatcher::Name(name) => vec![name],
        ProcessMatcher::Sidecar => vec!["char-sidecar".to_string(), "hyprnote-sidecar".to_string()],
    };

    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let mut killed_count = 0;

    for process in sys.processes().values() {
        let process_name = process.name().to_string_lossy();

        if targets.iter().any(|t| process_name.contains(t)) && process.kill() {
            killed_count += 1;
        }
    }

    killed_count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_long_os_version() {
        let a = long_os_version();
        let b = long_os_version();
        let c = long_os_version();
        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    #[test]
    fn test_cpu_arch() {
        let a = cpu_arch();
        let b = cpu_arch();
        let c = cpu_arch();
        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    #[test]
    fn test_fingerprint() {
        let a = fingerprint();
        let b = fingerprint();
        let c = fingerprint();
        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    #[test]
    fn test_kill_processes_by_matcher() {
        let killed_count = kill_processes_by_matcher(ProcessMatcher::Sidecar);
        assert!(killed_count == 0);
    }
}
