use std::time::{Duration, SystemTime, UNIX_EPOCH};

use activity_capture_macos::MacosCapture;
use futures_util::StreamExt;
use hypr_activity_capture_interface::{ActivityCapture, CapturePolicy, Snapshot, WatchOptions};

fn main() {
    let capture = MacosCapture::with_policy(CapturePolicy::default());

    let caps = capture.capabilities();
    println!("Capabilities:");
    println!("  can_watch:                       {}", caps.can_watch);
    println!(
        "  can_capture_visible_text:         {}",
        caps.can_capture_visible_text
    );
    println!(
        "  can_capture_browser_url:          {}",
        caps.can_capture_browser_url
    );
    println!(
        "  requires_accessibility_permission: {}",
        caps.requires_accessibility_permission
    );
    println!();

    match capture.snapshot() {
        Ok(Some(snap)) => {
            println!("Current snapshot:");
            print_snapshot(&snap);
            println!();
        }
        Ok(None) => println!("No active application.\n"),
        Err(e) => println!("Snapshot error: {e}\n"),
    }

    let options = WatchOptions {
        poll_interval: Duration::from_secs(2),
        emit_initial: true,
    };
    let stream = match capture.watch(options) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to start watch: {e}");
            return;
        }
    };

    println!("Watching for transitions (poll every 2s)... Ctrl+C to stop.\n");

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create tokio runtime");

    rt.block_on(async {
        tokio::pin!(stream);
        while let Some(result) = stream.next().await {
            match result {
                Ok(transition) => {
                    let ts = format_time(SystemTime::now());
                    println!("[{ts}] Transition");
                    if let Some(prev) = &transition.previous {
                        println!(
                            "  from: {} — {}",
                            prev.snapshot.app_name,
                            summary(&prev.snapshot)
                        );
                    } else {
                        println!("  from: (none)");
                    }
                    if let Some(curr) = &transition.current {
                        println!(
                            "  to:   {} — {}",
                            curr.snapshot.app_name,
                            summary(&curr.snapshot)
                        );
                        print_snapshot(&curr.snapshot);
                    } else {
                        println!("  to:   (idle)");
                    }
                    println!();
                }
                Err(e) => {
                    eprintln!("Watch error: {e}");
                }
            }
        }
    });
}

fn print_snapshot(snap: &Snapshot) {
    println!("  app:     {}", snap.app_name);
    println!("  bundle:  {}", snap.bundle_id.as_deref().unwrap_or("—"));
    println!("  window:  {}", snap.window_title.as_deref().unwrap_or("—"));
    println!("  url:     {}", snap.url.as_deref().unwrap_or("—"));
    println!("  level:   {:?}", snap.content_level);
    println!("  source:  {:?}", snap.source);
    if let Some(text) = &snap.visible_text {
        let preview: String = text.chars().take(120).collect();
        let ellipsis = if text.chars().count() > 120 {
            "…"
        } else {
            ""
        };
        println!("  text:    {preview}{ellipsis}");
    }
}

fn summary(snap: &Snapshot) -> String {
    snap.window_title
        .as_deref()
        .or(snap.url.as_deref())
        .unwrap_or("(no title)")
        .to_string()
}

fn format_time(time: SystemTime) -> String {
    let d = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = d.as_secs();
    let hours = (secs / 3600) % 24;
    let minutes = (secs / 60) % 60;
    let seconds = secs % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}
