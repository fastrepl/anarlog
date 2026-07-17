use std::process::Command;

pub fn is_do_not_disturb() -> bool {
    if !cfg!(target_os = "macos") {
        return false;
    }

    match Command::new("defaults")
        .args([
            "read",
            "com.apple.controlcenter",
            "NSStatusItem Visible FocusModes",
        ])
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                let out = String::from_utf8_lossy(&output.stdout);
                out.trim() == "1"
            } else {
                false
            }
        }
        Err(_) => false,
    }
}
