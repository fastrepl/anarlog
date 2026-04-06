#![cfg(target_os = "macos")]

use objc2::AnyThread;
use objc2_foundation::{NSAppleScript, NSString};

pub(crate) fn run(source: &str) -> Option<String> {
    let source = NSString::from_str(source);
    let script = NSAppleScript::initWithSource(NSAppleScript::alloc(), &source)?;
    let mut error = None;
    let result = unsafe { script.executeAndReturnError(Some(&mut error)) };
    if error.is_some() {
        return None;
    }

    result.stringValue().and_then(|value| {
        let value = value.to_string();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}
