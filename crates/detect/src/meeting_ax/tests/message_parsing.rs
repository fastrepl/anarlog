use super::*;

#[test]
fn test_chat_button_is_open_chat_control_not_input() {
    let target = candidate_chat_target(&node(4, "AXButton", "Chat", None)).unwrap();

    assert_eq!(target.kind, "openChatControl");
    assert!(!target.settable);
    assert!(target.signals.contains(&"open-chat-control".to_string()));
}

#[test]
fn test_zoom_chat_message_parser_preserves_sender_time_text_and_links() {
    let parsed = parse_chat_message(
        &MeetingPlatform::Zoom,
        "From Ada Lovelace to Everyone\n10:42 AM\nHere is the doc https://example.com/spec.",
    )
    .unwrap();

    assert_eq!(parsed.sender, Some("Ada Lovelace".to_string()));
    assert_eq!(parsed.timestamp, Some("10:42 AM".to_string()));
    assert_eq!(parsed.text, "Here is the doc https://example.com/spec.");
    assert_eq!(
        extract_links(&parsed.text),
        vec!["https://example.com/spec"]
    );
}

#[test]
fn test_zoom_chat_message_parser_handles_current_native_row_description() {
    let parsed = parse_chat_message(
        &MeetingPlatform::Zoom,
        "You, ANLG-76 AX integration 5844 https://example.com/ax-test, 4:16\u{202f}PM",
    )
    .unwrap();

    assert_eq!(parsed.sender, Some("You".to_string()));
    assert_eq!(parsed.timestamp, Some("4:16 PM".to_string()));
    assert_eq!(
        parsed.text,
        "ANLG-76 AX integration 5844 https://example.com/ax-test"
    );
    assert_eq!(
        extract_links(&parsed.text),
        vec!["https://example.com/ax-test"]
    );
}

#[test]
fn test_zoom_chat_message_parser_handles_current_web_row_description() {
    let parsed = parse_chat_message(
        &MeetingPlatform::Zoom,
        "You to Everyone, 02:20 PM, ANLG-297 Chrome Zoom web QA https://anarlog.so/chrome-zoom",
    )
    .unwrap();

    assert_eq!(parsed.sender, Some("You".to_string()));
    assert_eq!(parsed.timestamp, Some("02:20 PM".to_string()));
    assert_eq!(
        parsed.text,
        "ANLG-297 Chrome Zoom web QA https://anarlog.so/chrome-zoom"
    );
    assert_eq!(
        meeting_chat_direction(&MeetingPlatform::Zoom, parsed.sender.as_deref()),
        Some(MeetingChatDirection::Outgoing)
    );
    assert_eq!(
        extract_links(&parsed.text),
        vec!["https://anarlog.so/chrome-zoom"]
    );
}

#[test]
fn test_zoom_chat_direction_uses_native_self_sender_label() {
    assert_eq!(
        meeting_chat_direction(&MeetingPlatform::Zoom, Some("You")),
        Some(MeetingChatDirection::Outgoing)
    );
    assert_eq!(
        meeting_chat_direction(&MeetingPlatform::Zoom, Some("Ada")),
        Some(MeetingChatDirection::Incoming)
    );
    assert_eq!(
        meeting_chat_direction(&MeetingPlatform::Slack, Some("You")),
        None
    );
}

#[test]
fn test_teams_current_native_description_preserves_metadata_without_inferring_direction() {
    let raw = "anon cannon Sent ANLG-297 Teams native chat capture test Link https://example.com/teams Today at 5:08\u{202f}AM.";
    let parsed = parse_chat_message(&MeetingPlatform::MicrosoftTeams, raw).unwrap();

    assert_eq!(parsed.sender.as_deref(), Some("anon cannon"));
    assert_eq!(parsed.timestamp.as_deref(), Some("5:08 AM"));
    assert_eq!(parsed.direction, None);
    assert_eq!(
        parsed.text,
        "ANLG-297 Teams native chat capture test https://example.com/teams"
    );
    assert_eq!(extract_links(&parsed.text), ["https://example.com/teams"]);

    let messages = extract_chat_messages(
        &MeetingPlatform::MicrosoftTeams,
        &MeetingSurface::Native,
        &[
            fixture_node(0, "AXButton", "Leave", &[1]),
            fixture_node(1, "AXHeading", "Meeting chat", &[4, 0]),
            fixture_composer(2, "Type a message", &[4, 9, 0]),
            fixture_node(3, "AXGroup", raw, &[4, 1]),
        ],
    );
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].direction, None);
    assert_eq!(messages[0].sender.as_deref(), Some("anon cannon"));
    assert_eq!(messages[0].timestamp.as_deref(), Some("5:08 AM"));
    assert_eq!(messages[0].links, ["https://example.com/teams"]);
}

#[test]
fn test_teams_trailing_time_fallback_survives_today_at_in_message_text() {
    let raw = "anon cannon Sent We discussed Today at lunch 5:09 PM";
    let parsed = parse_chat_message(&MeetingPlatform::MicrosoftTeams, raw).unwrap();

    assert_eq!(parsed.sender.as_deref(), Some("anon cannon"));
    assert_eq!(parsed.timestamp.as_deref(), Some("5:09 PM"));
    assert_eq!(parsed.text, "We discussed Today at lunch");
    assert_eq!(parsed.direction, None);
}

#[test]
fn test_teams_current_firefox_description_preserves_metadata_without_inferring_direction() {
    let raw = "anon cannon Sent ANLG-297 Firefox Teams web QA Link https://anarlog.so/firefox-teams 3:03 PM";
    let parsed = parse_chat_message(&MeetingPlatform::MicrosoftTeams, raw).unwrap();

    assert_eq!(parsed.sender.as_deref(), Some("anon cannon"));
    assert_eq!(parsed.timestamp.as_deref(), Some("3:03 PM"));
    assert_eq!(parsed.direction, None);
    assert_eq!(
        parsed.text,
        "ANLG-297 Firefox Teams web QA https://anarlog.so/firefox-teams"
    );
    assert_eq!(
        extract_links(&parsed.text),
        ["https://anarlog.so/firefox-teams"]
    );

    let messages = extract_chat_messages(
        &MeetingPlatform::MicrosoftTeams,
        &MeetingSurface::Web,
        &[
            fixture_node(0, "AXButton", "Leave", &[1]),
            fixture_node(1, "AXHeading", "Meeting chat", &[4, 0]),
            fixture_composer(2, "Type a message", &[4, 9, 0]),
            fixture_node(3, "AXGroup", raw, &[4, 1]),
        ],
    );
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].direction, None);
    assert_eq!(messages[0].sender.as_deref(), Some("anon cannon"));
    assert_eq!(messages[0].timestamp.as_deref(), Some("3:03 PM"));
    assert_eq!(messages[0].links, ["https://anarlog.so/firefox-teams"]);
}

#[test]
fn test_native_webex_structured_message_preserves_metadata_and_direction() {
    let nodes = vec![
        fixture_node(0, "AXWindow", "John's meeting", &[]),
        fixture_node(
            1,
            "AXButton",
            "Leave meeting or end meeting for everyone",
            &[0],
        ),
        fixture_node(
            2,
            "AXScrollArea",
            "thread conversation history, list",
            &[1, 0],
        ),
        fixture_composer(
            3,
            "Write a message to everyone, Shift + Enter for a new line",
            &[1, 1],
        ),
        fixture_node(
            4,
            "AXCell",
            "ANLG-297 native Webex macOS QA https://anarlog.so/native-webex, You, 3:33\u{202f}PM, has hyperlinks, Press Enter key to navigate to the action buttons",
            &[1, 0, 0],
        ),
        fixture_node(
            5,
            "AXTextArea",
            "ANLG-297 native Webex macOS QA https://anarlog.so/native-webex",
            &[1, 0, 0, 0],
        ),
    ];

    let messages = extract_chat_messages(&MeetingPlatform::Webex, &MeetingSurface::Native, &nodes);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].sender.as_deref(), Some("You"));
    assert_eq!(messages[0].timestamp.as_deref(), Some("3:33 PM"));
    assert_eq!(messages[0].direction, Some(MeetingChatDirection::Outgoing));
    assert_eq!(
        messages[0].text,
        "ANLG-297 native Webex macOS QA https://anarlog.so/native-webex"
    );
    assert_eq!(messages[0].links, ["https://anarlog.so/native-webex"]);
}

#[test]
fn test_webex_browser_message_preserves_live_metadata_and_direction() {
    let parsed = parse_chat_message(
        &MeetingPlatform::Webex,
        "Message from You, Unverified, 3:43 PM, ANLG-297 Chrome Webex web QA https://anarlog.so/chrome-webex",
    )
    .unwrap();

    assert_eq!(parsed.sender.as_deref(), Some("You"));
    assert_eq!(parsed.timestamp.as_deref(), Some("3:43 PM"));
    assert_eq!(parsed.direction, Some(MeetingChatDirection::Outgoing));
    assert_eq!(
        parsed.text,
        "ANLG-297 Chrome Webex web QA https://anarlog.so/chrome-webex"
    );
    assert_eq!(
        extract_links(&parsed.text),
        ["https://anarlog.so/chrome-webex"]
    );

    let incoming = parse_chat_message(
        &MeetingPlatform::Webex,
        "Message from John Jeong, fastrepl.com, 3:33 PM, ANLG-297 native Webex macOS QA https://anarlog.so/native-webex",
    )
    .unwrap();
    assert_eq!(incoming.sender.as_deref(), Some("John Jeong"));
    assert_eq!(incoming.direction, Some(MeetingChatDirection::Incoming));
}

#[test]
fn test_native_linux_webex_structured_message_preserves_live_metadata() {
    let mut composer = fixture_node(
        3,
        "AXStaticText",
        "Write a message to everyone. Press Shift + Enter for new line.",
        &[1, 1, 0, 2, 0, 2, 0, 5, 4, 0],
    );
    composer.settable_value = false;
    let mut message_text = fixture_node(
        5,
        "AXTextArea",
        "Message text",
        &[1, 1, 0, 2, 0, 2, 0, 6, 0, 15, 0, 0, 6],
    );
    message_text.settable_value = true;
    message_text.value = Some(
        "ANLG-297 Linux native Webex AT-SPI QA https://anarlog.so/linux-native-webex-atspi"
            .to_string(),
    );
    let nodes = vec![
        fixture_node(0, "AXWindow", "John's meeting", &[]),
        fixture_node(1, "AXButton", "Leave meeting", &[1, 0]),
        fixture_node(
            2,
            "AXGroup",
            "Chat Tab list, Everyone tab",
            &[1, 1, 0, 1, 0],
        ),
        composer,
        fixture_node(
            4,
            "AXGroup",
            "ANLG-297 Linux native Webex AT-SPI QA https://anarlog.so/linux-native-webex-atspi, sent by You, 09:39:52, has hyperlinks, press Enter key to enter the group, then Tab key to navigate to message actions.",
            &[1, 1, 0, 2, 0, 2, 0, 6, 0, 15, 0, 0],
        ),
        message_text,
    ];

    let messages = extract_chat_messages(&MeetingPlatform::Webex, &MeetingSurface::Native, &nodes);
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].sender.as_deref(), Some("You"));
    assert_eq!(messages[0].timestamp.as_deref(), Some("09:39:52"));
    assert_eq!(messages[0].direction, Some(MeetingChatDirection::Outgoing));
    assert_eq!(
        messages[0].text,
        "ANLG-297 Linux native Webex AT-SPI QA https://anarlog.so/linux-native-webex-atspi"
    );
    assert_eq!(
        messages[0].links,
        ["https://anarlog.so/linux-native-webex-atspi"]
    );
}

#[test]
fn test_zoom_capture_requires_zoom_meeting_window_scope() {
    assert!(is_zoom_meeting_scope_node(&node(
        0,
        "AXWindow",
        "Zoom Meeting",
        None,
    )));
    assert!(is_zoom_meeting_scope_node(&node(
        1,
        "AXWindow",
        "John Jeong's Zoom Meeting",
        None,
    )));
    assert!(is_zoom_meeting_scope_node(&node(
        2, "AXWindow", "Meeting", None,
    )));
    assert!(!is_zoom_meeting_scope_node(&node(
        3,
        "AXWindow",
        "Zoom Workplace",
        None,
    )));

    let mut chat_row = node(4, "AXGroup", "You, meeting chat message, 4:16 PM", None);
    chat_row.identifier = Some("ZMTextMessageCellView".to_string());
    assert!(is_zoom_chat_scope_node(&chat_row));

    let mut linux_history = node(5, "AXList", "chat history", None);
    linux_history.description = Some("chat history".to_string());
    assert!(is_zoom_chat_scope_node(&linux_history));
    assert!(!is_zoom_chat_scope_node(&node(
        6,
        "AXList",
        "chat history summary",
        None,
    )));

    let mut meeting_caption = node(
        4,
        "AXStaticText",
        "Ada, confidential caption, 4:16 PM",
        None,
    );
    meeting_caption.within_zoom_meeting_scope = true;
    assert!(
        extract_chat_messages(
            &MeetingPlatform::Zoom,
            &MeetingSurface::Native,
            &[meeting_caption],
        )
        .is_empty()
    );

    let team_chat_message = node(7, "AXStaticText", "You, private team chat, 4:16 PM", None);
    assert!(
        extract_chat_messages(
            &MeetingPlatform::Zoom,
            &MeetingSurface::Native,
            &[team_chat_message],
        )
        .is_empty()
    );
}

#[test]
fn test_native_linux_zoom_structured_message_preserves_live_metadata() {
    let message = "ANLG-297 Linux native Zoom AT-SPI QA https://anarlog.so/linux-native-zoom-atspi";
    let mut window = fixture_node(0, "AXWindow", "Meeting", &[]);
    window.within_zoom_meeting_scope = true;
    let mut history = fixture_node(1, "AXList", "chat history", &[11, 59, 0, 25]);
    history.within_zoom_meeting_scope = true;
    let mut outer = fixture_node(2, "AXGroup", "You 10:07", &[11, 59, 0, 25, 0]);
    outer.description = Some(message.to_string());
    outer.within_zoom_meeting_scope = true;
    let mut inner = fixture_node(3, "AXGroup", "You 10:07", &[11, 59, 0, 25, 0, 0]);
    inner.description = Some(message.to_string());
    inner.within_zoom_meeting_scope = true;

    let messages = extract_chat_messages(
        &MeetingPlatform::Zoom,
        &MeetingSurface::Native,
        &[window, history, outer, inner],
    );

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].sender.as_deref(), Some("You"));
    assert_eq!(messages[0].timestamp.as_deref(), Some("10:07"));
    assert_eq!(messages[0].direction, Some(MeetingChatDirection::Outgoing));
    assert_eq!(messages[0].text, message);
    assert_eq!(
        messages[0].links,
        ["https://anarlog.so/linux-native-zoom-atspi"]
    );
}

#[test]
fn test_slack_chat_message_parser_handles_sender_time_prefix() {
    let parsed = parse_chat_message(
        &MeetingPlatform::Slack,
        "Grace Hopper 9:03 PM\nShip it after the final check",
    )
    .unwrap();

    assert_eq!(parsed.sender, Some("Grace Hopper".to_string()));
    assert_eq!(parsed.timestamp, Some("9:03 PM".to_string()));
    assert_eq!(parsed.text, "Ship it after the final check");
}

#[test]
fn test_slack_chat_message_parser_handles_native_accessibility_description() {
    let parsed = parse_chat_message(
        &MeetingPlatform::Slack,
        "John Jeong: @Artem lorem ipsum. Friday at 5:50\u{202f}PM.",
    )
    .unwrap();

    assert_eq!(parsed.sender, Some("John Jeong".to_string()));
    assert_eq!(parsed.timestamp, Some("5:50 PM".to_string()));
    assert_eq!(parsed.text, "@Artem lorem ipsum");
}

#[test]
fn test_slack_chat_message_parser_handles_live_huddle_description() {
    let parsed = parse_chat_message(
        &MeetingPlatform::Slack,
        "John Jeong: I'm using Anarlog to record and transcribe this meeting. https://anarlog.so. 4:02 AM. 1 link.",
    )
    .unwrap();

    assert_eq!(parsed.sender, Some("John Jeong".to_string()));
    assert_eq!(parsed.timestamp, Some("4:02 AM".to_string()));
    assert_eq!(
        parsed.text,
        "I'm using Anarlog to record and transcribe this meeting. https://anarlog.so"
    );

    let mut active_control = node(0, "AXButton", "", None);
    active_control.value = Some(String::new());
    active_control.description = Some("Leave Huddle".to_string());
    let mut message = node(
        1,
        "AXGroup",
        "John Jeong: I'm using Anarlog to record and transcribe this meeting. https://anarlog.so. 4:02 AM. 1 link.",
        None,
    );
    message.value = Some(String::new());
    message.within_slack_huddle_scope = true;
    let messages = extract_chat_messages(
        &MeetingPlatform::Slack,
        &MeetingSurface::Native,
        &[active_control, message],
    );
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].links, ["https://anarlog.so"]);
}

#[test]
fn test_web_chat_parsers_cover_meet_teams_zoom_slack_and_webex_shapes() {
    for (platform, raw_text, sender, timestamp, text) in [
        (
            MeetingPlatform::GoogleMeet,
            "Ada Lovelace\n10:42 AM\nMeet decision",
            "Ada Lovelace",
            "10:42 AM",
            "Meet decision",
        ),
        (
            MeetingPlatform::MicrosoftTeams,
            "Grace Hopper 10:43 AM\nTeams decision",
            "Grace Hopper",
            "10:43 AM",
            "Teams decision",
        ),
        (
            MeetingPlatform::Zoom,
            "Linus Torvalds, Zoom decision, 10:44 AM",
            "Linus Torvalds",
            "10:44 AM",
            "Zoom decision",
        ),
        (
            MeetingPlatform::Slack,
            "Margaret Hamilton\nSlack decision\n10:45 AM",
            "Margaret Hamilton",
            "10:45 AM",
            "Slack decision",
        ),
        (
            MeetingPlatform::Webex,
            "Katherine Johnson\n10:46 AM\nWebex decision",
            "Katherine Johnson",
            "10:46 AM",
            "Webex decision",
        ),
    ] {
        let parsed = parse_chat_message(&platform, raw_text)
            .unwrap_or_else(|| panic!("failed to parse {platform:?}"));
        assert_eq!(parsed.sender.as_deref(), Some(sender));
        assert_eq!(parsed.timestamp.as_deref(), Some(timestamp));
        assert_eq!(parsed.text, text);
    }
}

#[test]
fn test_google_meet_capture_assembles_live_structured_message_leaves() {
    let scope_path = [4];
    let active_control = fixture_node(0, "AXButton", "Leave call", &[1]);
    let heading = fixture_node(1, "AXHeading", "In-call messages", &[4, 0]);
    let composer = fixture_composer(2, "Send a message", &[4, 9, 0]);

    let mut participant = fixture_node(3, "AXStaticText", "John Jeong (JJ)", &[3, 0]);
    participant.value = participant.title.take();
    let mut sender = fixture_node(4, "AXStaticText", "John Jeong (JJ) 4:14 AM", &[4, 1, 2, 0]);
    sender.value = sender.title.take();
    let mut text = fixture_node(
        5,
        "AXStaticText",
        "ANLG-297 Chrome-to-Aside chat capture test",
        &[4, 1, 4, 0, 0, 0],
    );
    text.value = text.title.take();
    let mut link = fixture_node(6, "AXLink", "", &[4, 1, 4, 0, 0, 1]);
    link.value = Some("example.com/chrome".to_string());
    link.description = Some("https://example.com/chrome".to_string());
    let mut second_text = fixture_node(
        7,
        "AXStaticText",
        "Second incoming message",
        &[4, 1, 5, 0, 0, 0],
    );
    second_text.value = second_text.title.take();

    assert_eq!(
        validated_chat_scope(
            &MeetingPlatform::GoogleMeet,
            &[
                active_control.clone(),
                heading.clone(),
                composer.clone(),
                participant.clone(),
                sender.clone(),
                text.clone(),
                link.clone(),
                second_text.clone(),
            ],
        )
        .map(|(scope, _)| scope),
        Some(scope_path.to_vec())
    );

    let messages = extract_chat_messages(
        &MeetingPlatform::GoogleMeet,
        &MeetingSurface::Web,
        &[
            active_control,
            heading,
            composer,
            participant,
            sender,
            text,
            link,
            second_text,
        ],
    );
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].sender.as_deref(), Some("John Jeong (JJ)"));
    assert_eq!(messages[0].timestamp.as_deref(), Some("4:14 AM"));
    assert_eq!(
        messages[0].text,
        "ANLG-297 Chrome-to-Aside chat capture test https://example.com/chrome"
    );
    assert_eq!(messages[0].links, ["https://example.com/chrome"]);
    assert_eq!(messages[1].text, "Second incoming message");
}

#[test]
fn test_google_meet_capture_keeps_firefox_link_only_message_rows() {
    let active_control = fixture_node(0, "AXButton", "Leave call", &[1]);
    let heading = fixture_node(1, "AXHeading", "In-call messages", &[4, 0]);
    let composer = fixture_composer(2, "Send a message", &[4, 2, 0, 1, 2]);
    let banner_link = fixture_node(
        3,
        "AXButton",
        "https://support.google.com/meet/chat",
        &[4, 2, 0, 0, 1],
    );
    let message_link = fixture_node(
        4,
        "AXButton",
        "https://anarlog.so/linux-firefox-meet-atspi",
        &[4, 2, 0, 1, 1, 0],
    );
    let pin = fixture_node(5, "AXButton", "Pin message", &[4, 2, 0, 1, 2, 0]);

    let messages = extract_chat_messages(
        &MeetingPlatform::GoogleMeet,
        &MeetingSurface::Web,
        &[
            active_control,
            heading,
            composer,
            banner_link,
            message_link,
            pin,
        ],
    );

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].sender, None);
    assert_eq!(messages[0].timestamp, None);
    assert_eq!(
        messages[0].text,
        "https://anarlog.so/linux-firefox-meet-atspi"
    );
    assert_eq!(
        messages[0].links,
        ["https://anarlog.so/linux-firefox-meet-atspi"]
    );
}

#[test]
fn test_google_meet_capture_assembles_timestamp_sibling_messages() {
    let active_control = fixture_node(0, "AXButton", "Leave call", &[1]);
    let heading = fixture_node(1, "AXHeading", "In-call messages", &[4, 0]);
    let composer = fixture_composer(2, "Send a message", &[4, 2, 3, 0]);

    let mut first_timestamp = fixture_node(3, "AXStaticText", "1:49\u{202f}PM", &[4, 2, 2, 0, 0]);
    first_timestamp.value = first_timestamp.title.take();
    let mut first_text = fixture_node(
        4,
        "AXStaticText",
        "ANLG-297 Safari macOS chat QA",
        &[4, 2, 2, 1, 0],
    );
    first_text.value = first_text.title.take();
    let mut first_link = fixture_node(5, "AXLink", "", &[4, 2, 2, 1, 1]);
    first_link.value = Some("anarlog.so/safari-qa".to_string());
    first_link.title = Some("https://anarlog.so/safari-qa".to_string());
    let mut pin_help = fixture_node(
        6,
        "AXStaticText",
        "Hover over a message to pin it",
        &[4, 2, 2, 1, 2],
    );
    pin_help.value = pin_help.title.take();

    let mut sender = fixture_node(7, "AXStaticText", "John Jeong (JJ)", &[4, 2, 2, 4, 0]);
    sender.value = sender.title.take();
    let mut second_timestamp = fixture_node(8, "AXStaticText", "1:50\u{202f}PM", &[4, 2, 2, 5, 0]);
    second_timestamp.value = second_timestamp.title.take();
    let mut second_text = fixture_node(
        9,
        "AXStaticText",
        "ANLG-297 Chrome host to Safari",
        &[4, 2, 2, 6, 0],
    );
    second_text.value = second_text.title.take();
    let mut second_link = fixture_node(10, "AXLink", "", &[4, 2, 2, 6, 1]);
    second_link.value = Some("anarlog.so/chrome-host".to_string());
    second_link.title = Some("https://anarlog.so/chrome-host".to_string());
    let mut third_timestamp = fixture_node(11, "AXStaticText", "1:51\u{202f}PM", &[4, 2, 2, 7, 0]);
    third_timestamp.value = third_timestamp.title.take();
    let mut third_text = fixture_node(
        12,
        "AXStaticText",
        "ANLG-297 second self-authored message",
        &[4, 2, 2, 8, 0],
    );
    third_text.value = third_text.title.take();

    let messages = extract_chat_messages(
        &MeetingPlatform::GoogleMeet,
        &MeetingSurface::Web,
        &[
            active_control,
            heading,
            composer,
            first_timestamp,
            first_text,
            first_link,
            pin_help,
            sender,
            second_timestamp,
            second_text,
            second_link,
            third_timestamp,
            third_text,
        ],
    );

    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0].sender, None);
    assert_eq!(messages[0].timestamp.as_deref(), Some("1:49 PM"));
    assert_eq!(
        messages[0].text,
        "ANLG-297 Safari macOS chat QA https://anarlog.so/safari-qa"
    );
    assert_eq!(messages[0].links, ["https://anarlog.so/safari-qa"]);
    assert_eq!(messages[1].sender.as_deref(), Some("John Jeong (JJ)"));
    assert_eq!(messages[1].timestamp.as_deref(), Some("1:50 PM"));
    assert_eq!(
        messages[1].text,
        "ANLG-297 Chrome host to Safari https://anarlog.so/chrome-host"
    );
    assert_eq!(messages[1].links, ["https://anarlog.so/chrome-host"]);
    assert_eq!(messages[2].sender, None);
    assert_eq!(messages[2].timestamp.as_deref(), Some("1:51 PM"));
    assert_eq!(messages[2].text, "ANLG-297 second self-authored message");
}

#[test]
fn test_past_slack_huddle_thread_is_not_captured_without_active_huddle() {
    let mut message = node(
        0,
        "AXGroup",
        "John Jeong: @Artem lorem ipsum. Friday at 5:50 PM.",
        None,
    );
    message.within_slack_huddle_scope = true;

    assert!(
        extract_chat_messages(&MeetingPlatform::Slack, &MeetingSurface::Native, &[message],)
            .is_empty()
    );
}

#[test]
fn test_chat_parsers_reject_unstructured_static_text() {
    assert!(
        parse_chat_message(
            &MeetingPlatform::Zoom,
            "Recording has started for this meeting"
        )
        .is_none()
    );
    assert!(parse_chat_message(&MeetingPlatform::Slack, "Channels\nGeneral").is_none());
    assert!(
        parse_chat_message(
            &MeetingPlatform::GoogleMeet,
            "Continuous chat is turned off, Messages will not be saved when the call ends, 4:10 AM",
        )
        .is_none()
    );
}

#[test]
fn test_chat_parsers_reject_invalid_timestamps() {
    assert!(!looks_like_time("99:99"));
    assert!(!looks_like_time("13:00 PM"));
    assert!(looks_like_time("23:59"));
    assert!(looks_like_time("12:59 PM"));
}
