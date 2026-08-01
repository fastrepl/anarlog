use super::*;

#[test]
fn test_active_bundle_selection_is_scoped_and_deduplicated() {
    let active_bundle_ids = vec![
        "com.tinyspeck.slackmacgap".to_string(),
        "com.google.Chrome".to_string(),
        "com.tinyspeck.slackmacgap".to_string(),
        "com.example.unrelated".to_string(),
    ];

    assert_eq!(
        select_active_bundle_ids(
            MEETING_APP_BUNDLES.iter().map(|bundle| bundle.id),
            &active_bundle_ids,
        ),
        vec!["com.tinyspeck.slackmacgap", "com.google.Chrome"]
    );
    assert!(
        select_active_bundle_ids(MEETING_APP_BUNDLES.iter().map(|bundle| bundle.id), &[],)
            .is_empty()
    );
}

#[test]
fn test_slack_capture_requires_active_huddle_and_huddle_specific_scope() {
    let active_control = node(0, "AXButton", "Leave huddle", None);
    let channel_message = node(
        1,
        "AXStaticText",
        "Grace Hopper 9:03 PM\nChannel-only message",
        None,
    );

    assert!(
        extract_chat_messages(
            &MeetingPlatform::Slack,
            &MeetingSurface::Native,
            &[active_control.clone(), channel_message.clone()],
        )
        .is_empty()
    );

    let mut huddle_message = channel_message;
    huddle_message.within_slack_huddle_scope = true;
    assert!(
        extract_chat_messages(
            &MeetingPlatform::Slack,
            &MeetingSurface::Native,
            &[huddle_message.clone()],
        )
        .is_empty()
    );

    let messages = extract_chat_messages(
        &MeetingPlatform::Slack,
        &MeetingSurface::Native,
        &[active_control, huddle_message],
    );
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].text, "Channel-only message");
}

#[test]
fn test_slack_huddle_scope_rejects_general_channel_containers() {
    assert!(is_slack_huddle_scope_node(&node(
        0,
        "AXGroup",
        "Huddle chat",
        None,
    )));
    assert!(!is_slack_huddle_scope_node(&node(
        1,
        "AXGroup",
        "#general conversation",
        None,
    )));
    assert!(!slack_huddle_is_active(&[node(
        2,
        "AXStaticText",
        "Someone mentioned leave huddle in a channel message",
        None,
    )]));
}

#[test]
fn test_slack_capture_context_identity_tracks_validated_surface() {
    let context = slack_capture_context_id("test", "Huddle in test", 0x101, 0x202);

    assert_eq!(
        context,
        slack_capture_context_id("test", "Huddle in test", 0x101, 0x202)
    );
    assert_ne!(
        context,
        slack_capture_context_id("another", "Huddle in another", 0x101, 0x202)
    );
    assert_ne!(
        context,
        slack_capture_context_id("test", "Huddle in test", 0x303, 0x202)
    );
    assert_ne!(
        context,
        slack_capture_context_id("test", "Huddle in test", 0x101, 0x404)
    );
}

#[test]
fn test_zoom_capture_context_stays_stable_across_participant_changes() {
    let root = |chat_hash, participant_names: &[&str]| {
        let mut window = node(0, "AXWindow", "Zoom Meeting", None);
        window.element_hash = Some(0x101);
        window.within_zoom_meeting_scope = true;

        let mut chat = node(1, "AXTable", "Chat list", None);
        chat.element_hash = Some(chat_hash);
        chat.within_zoom_meeting_scope = true;
        chat.within_zoom_chat_scope = true;

        let participants = participant_names.iter().enumerate().map(|(index, name)| {
            let mut participant = node(
                index + 2,
                "AXGroup",
                &format!("Video render {name}, Computer audio unmuted"),
                None,
            );
            participant.element_hash = Some(0x300 + index);
            participant.within_zoom_meeting_scope = true;
            participant
        });

        NativeMeetingRoot {
            window_title: Some("Zoom Meeting".to_string()),
            nodes: std::iter::once(window)
                .chain(std::iter::once(chat))
                .chain(participants)
                .collect(),
        }
    };

    let first = zoom_capture_context_id(&root(0x202, &["Ada", "Grace"])).unwrap();
    let reordered = zoom_capture_context_id(&root(0x202, &["Grace", "Ada"])).unwrap();
    let switched = zoom_capture_context_id(&root(0x202, &["Ada", "Linus"])).unwrap();
    let new_chat_surface = zoom_capture_context_id(&root(0x404, &["Ada", "Grace"])).unwrap();

    assert_eq!(first, reordered);
    assert_eq!(first, switched);
    assert_ne!(first, new_chat_surface);
}

#[test]
fn test_extract_chat_messages_keeps_repeated_source_rows_distinct() {
    let message = "From Ada Lovelace to Everyone\n10:42 AM\nDecision: keep the launch date";
    let nodes = vec![
        zoom_message_node(12, message),
        zoom_message_node(13, message),
    ];

    let messages = extract_chat_messages(&MeetingPlatform::Zoom, &MeetingSurface::Native, &nodes);

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].sender, Some("Ada Lovelace".to_string()));
    assert_eq!(messages[0].text, "Decision: keep the launch date");
    assert_ne!(messages[0].id, messages[1].id);
    assert!(messages[0].id.ends_with("occurrence=1"));
    assert!(messages[1].id.ends_with("occurrence=2"));
}

#[test]
fn test_element_hash_stabilizes_identical_rows_across_snapshot_shifts() {
    let message = "From Ada Lovelace to Everyone\n10:42 AM\nDecision: keep the launch date";
    let hashed_node = |index, element_hash| {
        let mut node = zoom_message_node(index, message);
        node.element_hash = Some(element_hash);
        node
    };

    let first = extract_chat_messages(
        &MeetingPlatform::Zoom,
        &MeetingSurface::Native,
        &[hashed_node(0, 0x101), hashed_node(1, 0x202)],
    );
    let shifted = extract_chat_messages(
        &MeetingPlatform::Zoom,
        &MeetingSurface::Native,
        &[
            hashed_node(0, 0x303),
            hashed_node(1, 0x101),
            hashed_node(2, 0x202),
        ],
    );

    assert_eq!(first[0].id, shifted[1].id);
    assert_eq!(first[1].id, shifted[2].id);
    assert!(first[0].id.contains("cfhash=101"));
    assert!(first[0].id.contains("Decision: keep the launch date"));
}

#[test]
fn test_extract_chat_messages_retains_newest_eighty_rows() {
    let nodes = (0..85)
        .map(|index| zoom_message_node(index, &format!("From Ada to Everyone\nmessage {index}")))
        .collect::<Vec<_>>();

    let messages = extract_chat_messages(&MeetingPlatform::Zoom, &MeetingSurface::Native, &nodes);

    assert_eq!(messages.len(), 80);
    assert_eq!(messages.first().unwrap().text, "message 5");
    assert_eq!(messages.last().unwrap().text, "message 84");
}
