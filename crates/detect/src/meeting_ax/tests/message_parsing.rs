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
fn test_teams_current_native_description_preserves_metadata_and_direction() {
    let raw = "anon cannon Sent ANLG-297 Teams native chat capture test Link https://example.com/teams Today at 5:08\u{202f}AM.";
    let parsed = parse_chat_message(&MeetingPlatform::MicrosoftTeams, raw).unwrap();

    assert_eq!(parsed.sender.as_deref(), Some("anon cannon"));
    assert_eq!(parsed.timestamp.as_deref(), Some("5:08 AM"));
    assert_eq!(parsed.direction, Some(MeetingChatDirection::Outgoing));
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
    assert_eq!(messages[0].direction, Some(MeetingChatDirection::Outgoing));
    assert_eq!(messages[0].sender.as_deref(), Some("anon cannon"));
    assert_eq!(messages[0].timestamp.as_deref(), Some("5:08 AM"));
    assert_eq!(messages[0].links, ["https://example.com/teams"]);
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
    assert!(!is_zoom_meeting_scope_node(&node(
        2,
        "AXWindow",
        "Zoom Workplace",
        None,
    )));

    let mut chat_row = node(3, "AXGroup", "You, meeting chat message, 4:16 PM", None);
    chat_row.identifier = Some("ZMTextMessageCellView".to_string());
    assert!(is_zoom_chat_scope_node(&chat_row));

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

    let team_chat_message = node(3, "AXStaticText", "You, private team chat, 4:16 PM", None);
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
fn test_web_speaker_mapping_requires_an_explicit_accessibility_state() {
    for platform in [
        MeetingPlatform::Zoom,
        MeetingPlatform::GoogleMeet,
        MeetingPlatform::MicrosoftTeams,
        MeetingPlatform::Slack,
        MeetingPlatform::Webex,
    ] {
        let speaker = fixture_node(40, "AXGroup", "Ada Lovelace, active speaker", &[4, 0]);
        let stream = candidate_stream(&platform, &MeetingSurface::Web, &speaker)
            .unwrap_or_else(|| panic!("failed to map {platform:?} speaker"));
        assert_eq!(stream.participant_name.as_deref(), Some("Ada Lovelace"));
        assert!(stream.is_active_speaker);

        let chat_text = fixture_node(
            41,
            "AXStaticText",
            "We should discuss active speaker behavior",
            &[4, 1],
        );
        assert!(candidate_stream(&platform, &MeetingSurface::Web, &chat_text).is_none());
    }

    for label in [
        "Ada Lovelace, speaking French",
        "Ada Lovelace, speaking=false",
        "Ada Lovelace, active speaker=false",
        "Ada Lovelace, active speaker (false)",
        "Ada Lovelace, active speaker, false",
        "Ada Lovelace, speaking, French",
        "Active speaker: false",
        "who is speaking",
        "We should discuss who is speaking",
        "someone is speaking",
    ] {
        let false_state = fixture_node(42, "AXGroup", label, &[4, 2]);
        assert!(
            candidate_stream(
                &MeetingPlatform::GoogleMeet,
                &MeetingSurface::Web,
                &false_state,
            )
            .is_none()
        );
    }

    let false_zoom_state = fixture_node(
        43,
        "AXGroup",
        "Video render Ada Lovelace, speaking=false",
        &[4, 3],
    );
    assert!(
        candidate_stream(
            &MeetingPlatform::Zoom,
            &MeetingSurface::Native,
            &false_zoom_state,
        )
        .is_none()
    );
}

#[test]
fn test_zoom_participant_names_cover_web_speaker_and_native_roster_labels() {
    assert_eq!(
        participant_name_from_evidence(
            &MeetingPlatform::Zoom,
            "Video render Grace Hopper is speaking",
        )
        .as_deref(),
        Some("Grace Hopper")
    );
    assert_eq!(
        participant_name_from_evidence(
            &MeetingPlatform::Zoom,
            "Video render Grace Hopper, Computer audio unmuted, active speaker",
        )
        .as_deref(),
        Some("Grace Hopper")
    );
    assert_eq!(
        participant_name_from_evidence(&MeetingPlatform::Zoom, "Ada Lovelace, active speaker",)
            .as_deref(),
        Some("Ada Lovelace")
    );
    assert_eq!(
        participant_name_from_evidence(
            &MeetingPlatform::Zoom,
            "Ada Lovelace (Host, me, Participant ID:417329) No audio connected",
        )
        .as_deref(),
        Some("Ada Lovelace")
    );
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
