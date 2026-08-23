use super::*;

#[test]
fn test_chat_inspection_does_not_use_writable_value_as_label() {
    let mut input = node(6, "AXTextArea", "", None);
    input.title = None;
    input.settable_value = true;
    input.value = Some("private draft".to_string());
    input.text = searchable_node_text(
        &input.role,
        &input.title,
        &input.value,
        &input.description,
        &input.placeholder,
        input.settable_value,
    );

    assert!(!input.text.contains("private draft"));
    assert!(candidate_chat_target(&input).is_none());
    assert_eq!(inspection_label(&input), None);

    let mut read_only_input = input.clone();
    read_only_input.settable_value = false;
    read_only_input.value = Some("private read-only text".to_string());
    read_only_input.text = searchable_node_text(
        &read_only_input.role,
        &read_only_input.title,
        &read_only_input.value,
        &read_only_input.description,
        &read_only_input.placeholder,
        read_only_input.settable_value,
    );
    assert!(!read_only_input.text.contains("private read-only text"));
    assert_eq!(node_labels(&read_only_input).count(), 0);

    let mut secure_input = read_only_input.clone();
    secure_input.role = Some("AXSecureTextField".to_string());
    secure_input.value = Some("private password".to_string());
    secure_input.text = searchable_node_text(
        &secure_input.role,
        &secure_input.title,
        &secure_input.value,
        &secure_input.description,
        &secure_input.placeholder,
        secure_input.settable_value,
    );
    assert!(!secure_input.text.contains("private password"));
    assert_eq!(node_labels(&secure_input).count(), 0);
}

#[test]
fn test_native_meeting_window_validation_is_evidence_backed() {
    let settings = [
        node(0, "AXWindow", "Zoom Workplace Settings", None),
        node(1, "AXStaticText", "Video", None),
        node(2, "AXButton", "Camera preview", None),
    ];
    let zoom_meeting = [node(
        3,
        "AXGroup",
        "Video render Ada Lovelace, Computer audio unmuted",
        None,
    )];
    let discord_voice = [node(4, "AXStaticText", "Voice connected", None)];

    assert!(!native_meeting_window_is_validated(
        &MeetingPlatform::Zoom,
        &settings,
    ));
    assert!(native_meeting_window_is_validated(
        &MeetingPlatform::Zoom,
        &zoom_meeting,
    ));
    let current_linux_zoom = [
        node(4, "AXWindow", "Meeting", None),
        node(
            5,
            "AXButton",
            "Leave",
            Some(AxRect {
                x: 1224.0,
                y: 792.0,
                width: 78.0,
                height: 56.0,
            }),
        ),
    ];
    assert!(native_meeting_window_is_validated(
        &MeetingPlatform::Zoom,
        &current_linux_zoom,
    ));
    assert!(!native_meeting_window_is_validated(
        &MeetingPlatform::Zoom,
        &[node(6, "AXWindow", "Meeting", None)],
    ));
    assert!(!native_meeting_window_is_validated(
        &MeetingPlatform::Zoom,
        &[
            node(7, "AXWindow", "Leave meeting", None),
            node(
                8,
                "AXButton",
                "Leave",
                Some(AxRect {
                    x: 898.0,
                    y: 588.0,
                    width: 70.0,
                    height: 32.0,
                }),
            ),
        ],
    ));
    assert!(native_meeting_window_is_validated(
        &MeetingPlatform::Zoom,
        &[node(
            9,
            "AXTabGroup",
            "John Jeong, Computer audio muted",
            None,
        )],
    ));
    assert!(native_meeting_window_is_validated(
        &MeetingPlatform::Discord,
        &discord_voice,
    ));
    for platform in [MeetingPlatform::MicrosoftTeams, MeetingPlatform::Webex] {
        assert!(!native_meeting_window_is_validated(
            &platform,
            &zoom_meeting,
        ));
    }
    assert!(native_meeting_window_is_validated(
        &MeetingPlatform::MicrosoftTeams,
        &[fixture_node(5, "AXButton", "Hang up", &[0])],
    ));
    assert!(!native_meeting_window_is_validated(
        &MeetingPlatform::MicrosoftTeams,
        &[fixture_node(6, "AXButton", "Leave", &[0])],
    ));
    assert!(native_meeting_window_is_validated(
        &MeetingPlatform::MicrosoftTeams,
        &[
            fixture_node(6, "AXButton", "Leave", &[0]),
            fixture_node(7, "AXToolbar", "Meeting controls", &[0]),
        ],
    ));
    assert!(native_meeting_window_is_validated(
        &MeetingPlatform::Webex,
        &[fixture_node(8, "AXButton", "Leave meeting", &[0])],
    ));
}
