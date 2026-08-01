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

    let mut participant = node(
        7,
        "AXImage",
        "",
        Some(AxRect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 100.0,
        }),
    );
    participant.title = None;
    participant.settable_value = true;
    participant.value = Some("private participant value".to_string());
    participant.text = searchable_node_text(
        &participant.role,
        &participant.title,
        &participant.value,
        &participant.description,
        &participant.placeholder,
        participant.settable_value,
    );
    assert!(
        candidate_stream(
            &MeetingPlatform::Slack,
            &MeetingSurface::Native,
            &participant,
        )
        .is_none()
    );

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
fn test_private_video_text_and_large_images_are_not_participant_streams() {
    let private_nodes = [
        node(20, "AXStaticText", "Private video notes", None),
        node(
            21,
            "AXImage",
            "Private camera preview",
            Some(AxRect {
                x: 0.0,
                y: 0.0,
                width: 640.0,
                height: 480.0,
            }),
        ),
    ];

    for platform in [
        MeetingPlatform::Zoom,
        MeetingPlatform::GoogleMeet,
        MeetingPlatform::MicrosoftTeams,
        MeetingPlatform::Slack,
        MeetingPlatform::Discord,
        MeetingPlatform::Webex,
    ] {
        assert!(
            find_participant_streams(&platform, &MeetingSurface::Native, &private_nodes).is_empty(),
            "unexpected participant for {platform:?}"
        );
    }
}

#[test]
fn test_zoom_participant_anchor_is_platform_specific_across_surfaces() {
    let participant = node(
        22,
        "AXGroup",
        "Video render Ada Lovelace, Computer audio unmuted",
        None,
    );

    assert!(
        candidate_stream(
            &MeetingPlatform::Zoom,
            &MeetingSurface::Native,
            &participant,
        )
        .is_some()
    );
    assert!(
        candidate_stream(&MeetingPlatform::Zoom, &MeetingSurface::Web, &participant,).is_some()
    );
    for platform in [
        MeetingPlatform::MicrosoftTeams,
        MeetingPlatform::Discord,
        MeetingPlatform::Webex,
    ] {
        assert!(candidate_stream(&platform, &MeetingSurface::Native, &participant).is_none());
    }
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
    assert!(native_meeting_window_is_validated(
        &MeetingPlatform::Webex,
        &[fixture_node(6, "AXButton", "Leave meeting", &[0])],
    ));
}

#[test]
fn test_unknown_browser_surface_does_not_emit_participant_streams() {
    let streams = find_participant_streams(
        &MeetingPlatform::Unknown,
        &MeetingSurface::Unknown,
        &[node(
            8,
            "AXGroup",
            "Video render Private Browser Content, active speaker",
            Some(AxRect {
                x: 0.0,
                y: 0.0,
                width: 320.0,
                height: 180.0,
            }),
        )],
    );

    assert!(streams.is_empty());
}
