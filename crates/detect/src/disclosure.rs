#[cfg(target_os = "macos")]
mod platform {
    use std::collections::HashSet;
    use std::time::Duration;

    use cidre::{arc, ax, cf, cg, ns};
    use objc2_app_kit::{
        NSApplicationActivationOptions, NSPasteboard, NSPasteboardTypeString, NSRunningApplication,
    };
    use objc2_foundation::NSString;

    const MAX_AX_ELEMENTS: usize = 900;
    const APP_ACTIVATE_WAIT: Duration = Duration::from_millis(250);
    const CHAT_OPEN_WAIT: Duration = Duration::from_millis(700);
    const INPUT_FOCUS_WAIT: Duration = Duration::from_millis(120);
    const PASTE_WAIT: Duration = Duration::from_millis(250);
    const ANSI_V_KEY_CODE: cg::KeyCode = 0x09;

    const BROWSER_BUNDLE_IDS: &[&str] = &[
        "app.zen-browser.zen",
        "com.apple.Safari",
        "com.brave.Browser",
        "com.google.Chrome",
        "com.google.Chrome.canary",
        "com.microsoft.edgemac",
        "com.microsoft.edgemac.Canary",
        "com.operasoftware.Opera",
        "com.vivaldi.Vivaldi",
        "company.thebrowser.Browser",
        "org.mozilla.firefox",
    ];
    const NATIVE_BUNDLE_IDS: &[&str] = &[
        "us.zoom.xos",
        "com.microsoft.teams",
        "com.microsoft.teams2",
        "com.microsoft.teams.insiders",
    ];

    pub fn send_meeting_disclosure(
        app_ids: Option<Vec<String>>,
        message: String,
    ) -> Result<(), String> {
        if message.trim().is_empty() {
            return Err("disclosure message is empty".to_string());
        }

        if !ax::is_process_trusted() {
            return Err(
                "Accessibility permission is required for automatic disclosure".to_string(),
            );
        }

        let app_ids = resolve_target_app_ids(app_ids)?;
        if app_ids.is_empty() {
            return Err("no supported meeting app is using the microphone".to_string());
        }

        let mut errors = Vec::new();
        for app_id in app_ids {
            let Some(pid) = find_running_pid(&app_id) else {
                continue;
            };

            match post_disclosure_to_pid(pid, &message) {
                Ok(true) => return Ok(()),
                Ok(false) => errors.push(format!(
                    "{app_id}: chat input not found or did not accept text"
                )),
                Err(error) => errors.push(format!("{app_id}: {error}")),
            }
        }

        if errors.is_empty() {
            Err("supported meeting app is not running".to_string())
        } else {
            Err(errors.join("; "))
        }
    }

    fn resolve_target_app_ids(app_ids: Option<Vec<String>>) -> Result<Vec<String>, String> {
        let ids = match app_ids {
            Some(ids) if !ids.is_empty() => ids,
            _ => list_current_mic_app_ids()?,
        };

        let mut seen = HashSet::new();
        let mut ids = ids
            .into_iter()
            .filter(|id| is_supported_meeting_app(id))
            .filter(|id| seen.insert(id.clone()))
            .collect::<Vec<_>>();
        ids.sort_by_key(|id| platform_priority(id));
        Ok(ids)
    }

    #[cfg(feature = "list")]
    fn list_current_mic_app_ids() -> Result<Vec<String>, String> {
        crate::list_mic_using_apps()
            .map(|apps| apps.into_iter().map(|app| app.id).collect())
            .map_err(|error| error.to_string())
    }

    #[cfg(not(feature = "list"))]
    fn list_current_mic_app_ids() -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }

    fn is_supported_meeting_app(app_id: &str) -> bool {
        NATIVE_BUNDLE_IDS.contains(&app_id) || BROWSER_BUNDLE_IDS.contains(&app_id)
    }

    fn platform_priority(app_id: &str) -> u8 {
        if app_id == "us.zoom.xos" {
            0
        } else if app_id.contains("teams") {
            1
        } else {
            2
        }
    }

    fn find_running_pid(bundle_id: &str) -> Option<i32> {
        let bundle_id = ns::String::with_str(bundle_id);
        let apps = ns::RunningApp::with_bundle_id(&bundle_id);
        let app = apps.get(0).ok()?;
        Some(app.pid())
    }

    fn post_disclosure_to_pid(pid: i32, message: &str) -> Result<bool, String> {
        activate_running_app(pid);

        let app = ax::UiElement::with_app_pid(pid);
        let _ = app.set_messaging_timeout(Duration::from_millis(400));

        if try_post_disclosure(&app, message)? {
            return Ok(true);
        }

        if !press_chat_opener(&app) {
            return Ok(false);
        }

        std::thread::sleep(CHAT_OPEN_WAIT);
        try_post_disclosure(&app, message)
    }

    fn activate_running_app(pid: i32) {
        let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) else {
            return;
        };

        let _ = app.unhide();
        #[allow(deprecated)]
        let options = NSApplicationActivationOptions::ActivateAllWindows
            | NSApplicationActivationOptions::ActivateIgnoringOtherApps;
        let _ = app.activateWithOptions(options);
        std::thread::sleep(APP_ACTIVATE_WAIT);
    }

    fn try_post_disclosure(root: &ax::UiElement, message: &str) -> Result<bool, String> {
        let candidates = message_input_candidates(root);
        let mut last_error = None;

        for mut candidate in candidates {
            match type_disclosure_into_input(&mut candidate.element, root, message) {
                Ok(()) => return Ok(true),
                Err(error) => last_error = Some(error),
            }
        }

        if let Some(error) = last_error {
            tracing::debug!(%error, "meeting_disclosure_input_rejected");
        }
        Ok(false)
    }

    struct ElementCandidate {
        element: arc::R<ax::UiElement>,
        score: i32,
        order: usize,
    }

    fn message_input_candidates(root: &ax::UiElement) -> Vec<ElementCandidate> {
        let mut candidates = collect_elements(root)
            .into_iter()
            .enumerate()
            .filter_map(|(order, element)| {
                let score = score_message_input(&element_summary(&element), role_name(&element));
                (score > 0).then_some(ElementCandidate {
                    element,
                    score,
                    order,
                })
            })
            .collect::<Vec<_>>();

        candidates.sort_by(|a, b| b.score.cmp(&a.score).then(a.order.cmp(&b.order)));
        candidates
    }

    fn press_chat_opener(root: &ax::UiElement) -> bool {
        let mut candidates = collect_elements(root)
            .into_iter()
            .enumerate()
            .filter_map(|(order, element)| {
                let role = role_name(&element);
                if role.as_deref() != Some("AXButton")
                    && role.as_deref() != Some("AXMenuItem")
                    && role.as_deref() != Some("AXMenuButton")
                {
                    return None;
                }

                let score = score_chat_opener(&element_summary(&element));
                (score > 0).then_some(ElementCandidate {
                    element,
                    score,
                    order,
                })
            })
            .collect::<Vec<_>>();

        candidates.sort_by(|a, b| b.score.cmp(&a.score).then(a.order.cmp(&b.order)));

        candidates.into_iter().any(|candidate| {
            candidate
                .element
                .perform_action(ax::action::press())
                .is_ok()
        })
    }

    fn type_disclosure_into_input(
        input: &mut ax::UiElement,
        root: &ax::UiElement,
        message: &str,
    ) -> Result<(), String> {
        focus_message_input(input)?;

        paste_disclosure_message(message)?;
        std::thread::sleep(PASTE_WAIT);

        if !input_contains_message(input, message) {
            return Err("chat input did not receive pasted disclosure text".to_string());
        }

        if input.perform_action(ax::action::confirm()).is_ok() {
            return Ok(());
        }

        if press_send_button(root) {
            return Ok(());
        }

        Ok(())
    }

    fn focus_message_input(input: &mut ax::UiElement) -> Result<(), String> {
        let focused: &'static cf::Type = true.into();
        let ax_focused = input.set_attr(ax::attr::focused(), focused).is_ok();
        let clicked = input_center(input)
            .map(post_mouse_click)
            .transpose()?
            .is_some();

        std::thread::sleep(INPUT_FOCUS_WAIT);

        if ax_focused || clicked {
            Ok(())
        } else {
            Err("failed to focus chat input".to_string())
        }
    }

    fn input_center(input: &ax::UiElement) -> Option<cg::Point> {
        let position = input.pos().ok()?.cg_point()?;
        let size = input.size().ok()?.cg_size()?;
        if size.width <= 0.0 || size.height <= 0.0 {
            return None;
        }

        Some(cg::Point::new(
            position.x + (size.width / 2.0),
            position.y + (size.height / 2.0),
        ))
    }

    fn paste_disclosure_message(message: &str) -> Result<(), String> {
        let pasteboard = NSPasteboard::generalPasteboard();
        let _ = pasteboard.clearContents();

        let value = NSString::from_str(message);
        let text_type = unsafe { NSPasteboardTypeString };
        if !pasteboard.setString_forType(&value, text_type) {
            return Err("failed to write disclosure to pasteboard".to_string());
        }

        post_key_combination(ANSI_V_KEY_CODE, cg::EventFlags::CMD)
    }

    fn post_mouse_click(point: cg::Point) -> Result<(), String> {
        let mouse_down = cg::Event::mouse(
            None,
            cg::EventType::LEFT_MOUSE_DOWN,
            point,
            cg::MouseButton::Left,
        )
        .ok_or_else(|| "failed to create input click-down event".to_string())?;
        mouse_down.post(cg::EventTapLocation::Hid);

        let mouse_up = cg::Event::mouse(
            None,
            cg::EventType::LEFT_MOUSE_UP,
            point,
            cg::MouseButton::Left,
        )
        .ok_or_else(|| "failed to create input click-up event".to_string())?;
        mouse_up.post(cg::EventTapLocation::Hid);

        Ok(())
    }

    fn post_key_combination(key_code: cg::KeyCode, flags: cg::EventFlags) -> Result<(), String> {
        let mut key_down = cg::Event::keyboard(None, key_code, true)
            .ok_or_else(|| "failed to create paste key-down event".to_string())?;
        key_down.set_flags(flags);
        key_down.post(cg::EventTapLocation::Hid);

        let mut key_up = cg::Event::keyboard(None, key_code, false)
            .ok_or_else(|| "failed to create paste key-up event".to_string())?;
        key_up.set_flags(flags);
        key_up.post(cg::EventTapLocation::Hid);

        Ok(())
    }

    fn input_contains_message(input: &ax::UiElement, message: &str) -> bool {
        [ax::attr::value(), ax::attr::selected_text()]
            .into_iter()
            .filter_map(|attr| attr_string(input, attr))
            .any(|value| contains_disclosure_text(&value, message))
    }

    fn press_send_button(root: &ax::UiElement) -> bool {
        let mut candidates = collect_elements(root)
            .into_iter()
            .enumerate()
            .filter_map(|(order, element)| {
                if role_name(&element).as_deref() != Some("AXButton") {
                    return None;
                }

                let score = score_send_button(&element_summary(&element));
                (score > 0).then_some(ElementCandidate {
                    element,
                    score,
                    order,
                })
            })
            .collect::<Vec<_>>();

        candidates.sort_by(|a, b| b.score.cmp(&a.score).then(a.order.cmp(&b.order)));

        candidates.into_iter().any(|candidate| {
            candidate
                .element
                .perform_action(ax::action::press())
                .is_ok()
        })
    }

    fn collect_elements(root: &ax::UiElement) -> Vec<arc::R<ax::UiElement>> {
        let mut elements = Vec::new();
        let mut stack = vec![root.retained()];

        while let Some(element) = stack.pop() {
            if elements.len() >= MAX_AX_ELEMENTS {
                break;
            }

            for child in child_elements(&element).into_iter().rev() {
                stack.push(child);
            }

            elements.push(element);
        }

        elements
    }

    fn child_elements(element: &ax::UiElement) -> Vec<arc::R<ax::UiElement>> {
        let mut children = Vec::new();

        if role_name(element).as_deref() == Some("AXApplication")
            && let Ok(focused_element) = element.focused_ui_element()
        {
            children.push(focused_element);
        }
        if let Ok(nav_children) = element.children_in_nav_order() {
            children.extend(nav_children.iter().map(|child| child.retained()));
        } else if let Ok(raw_children) = element.children() {
            children.extend(raw_children.iter().map(|child| child.retained()));
        }

        children
    }

    fn element_summary(element: &ax::UiElement) -> String {
        [
            ax::attr::title(),
            ax::attr::desc(),
            ax::attr::help(),
            ax::attr::placeholder_value(),
            ax::attr::value(),
            ax::attr::label_value(),
            ax::attr::id(),
            ax::attr::role_desc(),
        ]
        .into_iter()
        .filter_map(|attr| attr_string(element, attr))
        .map(|value| normalize_platform_text(&value))
        .collect::<Vec<_>>()
        .join(" ")
    }

    fn attr_string(element: &ax::UiElement, attr: &ax::Attr) -> Option<String> {
        element
            .attr_value(attr)
            .ok()?
            .try_as_string()
            .map(|value| value.to_string())
    }

    fn role_name(element: &ax::UiElement) -> Option<String> {
        element.role().ok().map(|role| role.to_string())
    }

    fn normalize_platform_text(value: &str) -> String {
        value
            .to_ascii_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn normalize_disclosure_text(value: &str) -> String {
        value.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn contains_disclosure_text(value: &str, message: &str) -> bool {
        normalize_disclosure_text(value).contains(&normalize_disclosure_text(message))
    }

    fn contains_any(value: &str, needles: &[&str]) -> bool {
        needles.iter().any(|needle| value.contains(needle))
    }

    fn score_message_input(summary: &str, role: Option<String>) -> i32 {
        let is_text_input = matches!(role.as_deref(), Some("AXTextArea" | "AXTextField"));
        if !is_text_input {
            return 0;
        }

        if contains_any(
            summary,
            &[
                "address", "search", "find", "email", "password", "phone", "name",
            ],
        ) {
            return 0;
        }

        let mut score = 10;
        if contains_any(summary, &["message", "chat", "conversation", "reply"]) {
            score += 30;
        }
        if contains_any(summary, &["type", "send", "everyone", "meeting"]) {
            score += 15;
        }

        score
    }

    fn score_chat_opener(summary: &str) -> i32 {
        if contains_any(summary, &["hide chat", "close chat", "dismiss chat"]) {
            return 0;
        }

        let mut score = 0;
        if summary.contains("chat") {
            score += 25;
        }
        if summary.contains("conversation") {
            score += 20;
        }
        if contains_any(summary, &["open", "show", "meeting", "everyone"]) {
            score += 10;
        }

        score
    }

    fn score_send_button(summary: &str) -> i32 {
        if contains_any(summary, &["feedback", "to phone", "invite"]) {
            return 0;
        }

        if summary == "send" || summary.starts_with("send ") {
            return 30;
        }
        if summary.contains("send message") || summary.contains("send chat") {
            return 25;
        }
        0
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn supports_major_meeting_platforms() {
            assert!(is_supported_meeting_app("us.zoom.xos"));
            assert!(is_supported_meeting_app("com.google.Chrome"));
            assert!(is_supported_meeting_app("com.microsoft.edgemac"));
            assert!(is_supported_meeting_app("com.microsoft.teams"));
            assert!(is_supported_meeting_app("com.microsoft.teams2"));
            assert!(!is_supported_meeting_app("com.tinyspeck.slackmacgap"));
        }

        #[test]
        fn scores_chat_message_inputs_above_generic_text_inputs() {
            let generic = score_message_input("", Some("AXTextArea".to_string()));
            let chat =
                score_message_input("type a message to everyone", Some("AXTextArea".to_string()));

            assert!(generic > 0);
            assert!(chat > generic);
            assert_eq!(
                score_message_input("address and search bar", Some("AXTextField".to_string())),
                0
            );
        }

        #[test]
        fn avoids_closing_an_open_chat_panel() {
            assert!(score_chat_opener("open chat") > 0);
            assert!(score_chat_opener("chat with everyone") > 0);
            assert_eq!(score_chat_opener("hide chat"), 0);
            assert_eq!(score_chat_opener("close chat"), 0);
        }

        #[test]
        fn recognizes_send_buttons_without_feedback_controls() {
            assert!(score_send_button("send") > 0);
            assert!(score_send_button("send message") > 0);
            assert_eq!(score_send_button("send feedback"), 0);
        }

        #[test]
        fn verifies_disclosure_text_with_normalized_spacing() {
            assert!(contains_disclosure_text(
                "This meeting may be recorded and transcribed for notes.",
                "This meeting may be recorded and transcribed for notes.",
            ));
            assert!(contains_disclosure_text(
                "This meeting may be recorded\nand transcribed for notes.",
                "This meeting may be recorded and transcribed for notes.",
            ));
            assert!(!contains_disclosure_text(
                "Type a message to everyone",
                "This meeting may be recorded and transcribed for notes.",
            ));
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    pub fn send_meeting_disclosure(
        _app_ids: Option<Vec<String>>,
        _message: String,
    ) -> Result<(), String> {
        Err("automatic disclosure is only supported on macOS".to_string())
    }
}

pub fn send_meeting_disclosure(
    app_ids: Option<Vec<String>>,
    message: String,
) -> Result<(), String> {
    platform::send_meeting_disclosure(app_ids, message)
}
