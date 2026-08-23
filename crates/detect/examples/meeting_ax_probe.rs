fn main() {
    let inspections = detect::inspect_meeting_accessibility();
    if inspections.is_empty() {
        println!("no meeting apps reported an AX inspection");
        return;
    }

    for inspection in &inspections {
        println!(
            "=== inspect {} ({}) pid={} ===",
            inspection.app.name, inspection.app.id, inspection.pid
        );
        println!(
            "trusted={} platform={:?} surface={:?} title={:?}",
            inspection.accessibility_trusted,
            inspection.platform,
            inspection.surface,
            inspection.window_title
        );
        if inspection.warnings.is_empty() {
            println!("warnings: none");
        } else {
            println!("warnings:");
            for warning in &inspection.warnings {
                println!("  - {warning}");
            }
        }
        if inspection.surface == detect::MeetingSurface::Web {
            println!("browser ax:");
            for line in detect::describe_browser_ax(inspection.pid) {
                println!("  {line}");
            }
        }

        let capture = detect::capture_meeting_chat_messages(vec![inspection.app.id.clone()]);
        println!(
            "--- capture platform={:?} surface={:?} context={:?} messages={} ---",
            capture.platform,
            capture.surface,
            capture.context_id,
            capture.messages.len()
        );
        for message in &capture.messages {
            println!(
                "  [{:?}] {} at {:?}: {} links={:?}",
                message.direction,
                message.sender.as_deref().unwrap_or("?"),
                message.timestamp,
                message.text,
                message.links
            );
        }
        for warning in &capture.warnings {
            println!("  capture warning: {warning}");
        }
        println!();
    }
}
