use std::collections::HashSet;

use super::*;

#[test]
fn test_meeting_chat_message_validation() {
    assert!(validate_meeting_chat_message("disclosure message").is_ok());
    assert!(validate_meeting_chat_message(" \n\t ").is_err());
    assert!(validate_meeting_chat_message(&"x".repeat(2_000)).is_ok());
    assert!(validate_meeting_chat_message(&"x".repeat(2_001)).is_err());
}

#[test]
fn test_chat_mutation_is_enabled_only_for_macos_slack_huddles() {
    for bundle_id in ["com.tinyspeck.slackmacgap", "com.slack.Slack", "slack"] {
        assert_eq!(
            supports_meeting_chat_mutation(bundle_id),
            cfg!(target_os = "macos"),
            "{bundle_id} should be eligible for AX chat mutation only on macOS"
        );
    }
    for bundle_id in [
        "us.zoom.xos",
        "com.microsoft.teams2",
        "Cisco-Systems.Spark",
        "com.google.Chrome",
        "com.hnc.Discord",
        "zoom",
        "google-chrome",
        "com.anarlog.dev",
    ] {
        assert!(
            !supports_meeting_chat_mutation(bundle_id),
            "{bundle_id} should remain ineligible for AX chat mutation"
        );
    }
}

#[test]
fn test_linux_and_windows_process_aliases_map_to_meeting_apps() {
    for (alias, platform, browser) in [
        ("slack", MeetingPlatform::Slack, false),
        ("/usr/bin/slack", MeetingPlatform::Slack, false),
        ("Slack.exe", MeetingPlatform::Slack, false),
        ("zoom", MeetingPlatform::Zoom, false),
        ("teams-for-linux", MeetingPlatform::MicrosoftTeams, false),
        ("google-chrome", MeetingPlatform::Unknown, true),
        ("chrome", MeetingPlatform::Unknown, true),
        ("firefox", MeetingPlatform::Unknown, true),
        ("firefox-bin", MeetingPlatform::Unknown, true),
        ("opera-stable", MeetingPlatform::Unknown, true),
        ("opera-beta", MeetingPlatform::Unknown, true),
        ("opera-developer", MeetingPlatform::Unknown, true),
        ("browseros", MeetingPlatform::Unknown, true),
        ("helium", MeetingPlatform::Unknown, true),
        ("zen", MeetingPlatform::Unknown, true),
        ("zen-bin", MeetingPlatform::Unknown, true),
    ] {
        assert!(
            is_meeting_app_bundle(alias),
            "{alias} should be a recognized meeting app alias"
        );
        assert_eq!(classify_bundle(alias), platform);
        assert_eq!(is_browser_bundle(alias), browser);
    }

    assert_eq!(
        unique_recognized_meeting_bundle(&["slack".to_string()]).unwrap(),
        "slack"
    );
    assert_eq!(
        select_active_bundle_ids(
            MEETING_APP_BUNDLES.iter().map(|bundle| bundle.id),
            &["slack".to_string(), "google-chrome".to_string()],
        ),
        vec!["com.tinyspeck.slackmacgap", "com.google.Chrome"]
    );
}

#[test]
fn test_established_native_bundle_aliases_are_classified() {
    for (bundle_id, platform) in [
        ("com.slack.Slack", MeetingPlatform::Slack),
        ("com.cisco.webex", MeetingPlatform::Webex),
        ("com.cisco.webexmeetingsapp", MeetingPlatform::Webex),
        ("com.discordapp.Discord", MeetingPlatform::Discord),
    ] {
        assert!(is_meeting_app_bundle(bundle_id));
        assert_eq!(classify_bundle(bundle_id), platform);
        assert_eq!(
            classify_surface(bundle_id, &platform),
            MeetingSurface::Native
        );
    }
}

#[test]
fn test_established_browser_variants_are_recognized_as_web_surfaces() {
    for bundle_id in [
        "com.apple.SafariTechnologyPreview",
        "com.google.Chrome.canary",
        "com.microsoft.edgemac.Beta",
        "com.microsoft.edgemac.Canary",
        "com.microsoft.edgemac.Dev",
        "org.mozilla.firefoxdeveloperedition",
        "org.mozilla.nightly",
        "com.brave.Browser.beta",
        "com.brave.Browser.nightly",
        "org.chromium.Chromium",
        "com.operasoftware.OperaDeveloper",
        "com.operasoftware.OperaGX",
        "com.operasoftware.OperaNext",
        "net.imput.helium",
    ] {
        assert!(is_meeting_app_bundle(bundle_id));
        assert!(is_browser_bundle(bundle_id));
        assert_eq!(
            classify_surface(bundle_id, &MeetingPlatform::Unknown),
            MeetingSurface::Web
        );
    }
}

#[test]
fn test_meeting_app_registry_drives_bundle_kind() {
    let mut seen = HashSet::new();

    for bundle in MEETING_APP_BUNDLES {
        assert!(seen.insert(bundle.id), "duplicate bundle id: {}", bundle.id);
        assert!(is_meeting_app_bundle(bundle.id));
        assert_eq!(
            is_browser_bundle(bundle.id),
            bundle.kind == MeetingAppBundleKind::Browser,
            "unexpected browser classification for {}",
            bundle.id
        );
    }
}

#[test]
fn test_chat_mutation_scope_deduplicates_one_recognized_meeting_app() {
    let bundle_ids = vec![
        "com.tinyspeck.slackmacgap".to_string(),
        "com.tinyspeck.slackmacgap".to_string(),
        "com.hyprnote.dev".to_string(),
    ];

    assert_eq!(
        unique_recognized_meeting_bundle(&bundle_ids),
        Ok("com.tinyspeck.slackmacgap")
    );
}

#[test]
fn test_chat_mutation_scope_rejects_zero_or_multiple_meeting_apps() {
    assert!(unique_recognized_meeting_bundle(&[]).is_err());
    assert!(
        unique_recognized_meeting_bundle(&[
            "us.zoom.xos".to_string(),
            "com.tinyspeck.slackmacgap".to_string(),
        ])
        .is_err()
    );
}

#[test]
fn test_zoom_scope_does_not_fall_back_to_an_unrelated_slack_huddle() {
    let bundle_ids = ["us.zoom.xos".to_string()];
    let scoped_bundle = unique_recognized_meeting_bundle(&bundle_ids).unwrap();

    assert_eq!(scoped_bundle, "us.zoom.xos");
    assert!(!supports_meeting_chat_mutation(scoped_bundle));
}

#[test]
fn test_chat_input_candidate_requires_chat_signal() {
    let mut chat = node(3, "AXTextArea", "Send a message", None);
    chat.settable_value = true;
    chat.text = node_text(
        &chat.role,
        &chat.title,
        &chat.value,
        &chat.description,
        &chat.placeholder,
    );

    let target = candidate_chat_target(&chat).unwrap();

    assert_eq!(target.kind, "input");
    assert!(target.settable);
    assert!(target.confidence > 0.7);
}

#[test]
fn test_visible_empty_chat_surface_can_establish_a_capture_baseline() {
    let mut zoom_chat_list = node(4, "AXTable", "Chat list", None);
    zoom_chat_list.within_zoom_meeting_scope = true;
    zoom_chat_list.within_zoom_chat_scope = true;
    assert!(meeting_chat_surface_is_visible(
        &MeetingPlatform::Zoom,
        &[zoom_chat_list],
    ));

    let mut zoom_rename_input = node(5, "AXTextField", "Display name", None);
    zoom_rename_input.settable_value = true;
    zoom_rename_input.within_zoom_meeting_scope = true;
    zoom_rename_input.text = node_text(
        &zoom_rename_input.role,
        &zoom_rename_input.title,
        &zoom_rename_input.value,
        &zoom_rename_input.description,
        &zoom_rename_input.placeholder,
    );
    assert!(!meeting_chat_surface_is_visible(
        &MeetingPlatform::Zoom,
        &[zoom_rename_input],
    ));

    let mut slack_input = node(6, "AXTextArea", "Message to test", None);
    slack_input.settable_value = true;
    slack_input.within_slack_huddle_scope = true;
    slack_input.text = node_text(
        &slack_input.role,
        &slack_input.title,
        &slack_input.value,
        &slack_input.description,
        &slack_input.placeholder,
    );
    assert!(meeting_chat_surface_is_visible(
        &MeetingPlatform::Slack,
        &[slack_input],
    ));

    assert!(!meeting_chat_surface_is_visible(
        &MeetingPlatform::Slack,
        &[node(7, "AXTextArea", "Search", None)],
    ));
}

#[test]
fn test_browser_chat_scope_requires_live_exit_visible_composer_and_platform_container() {
    let meet_nodes = vec![
        fixture_node(0, "AXWebArea", "Team sync - Google Meet", &[]),
        fixture_node(1, "AXButton", "Leave call", &[0]),
        fixture_node(2, "AXGroup", "In-call messages", &[1]),
        fixture_composer(3, "Send a message", &[1, 0]),
    ];

    assert_eq!(
        validated_chat_scope(&MeetingPlatform::GoogleMeet, &meet_nodes),
        Some((vec![1], vec![1, 0]))
    );

    let mut disabled_composer_nodes = meet_nodes.clone();
    disabled_composer_nodes[3].enabled = Some(false);
    assert!(validated_chat_scope(&MeetingPlatform::GoogleMeet, &disabled_composer_nodes).is_none());
    assert_eq!(
        validated_chat_capture_scope(&MeetingPlatform::GoogleMeet, &disabled_composer_nodes),
        Some((vec![1], vec![1, 0]))
    );

    let mut prejoin_nodes = meet_nodes.clone();
    prejoin_nodes[1] = fixture_node(1, "AXButton", "Turn off microphone", &[0]);
    assert!(validated_chat_scope(&MeetingPlatform::GoogleMeet, &prejoin_nodes).is_none());

    let mut hidden_composer_nodes = meet_nodes.clone();
    hidden_composer_nodes[3].bounds = None;
    assert!(validated_chat_scope(&MeetingPlatform::GoogleMeet, &hidden_composer_nodes).is_none());

    let mut duplicate_composer_nodes = meet_nodes.clone();
    duplicate_composer_nodes.push(fixture_composer(4, "Send a message", &[1, 1]));
    assert!(
        validated_chat_scope(&MeetingPlatform::GoogleMeet, &duplicate_composer_nodes).is_none()
    );

    let mut support_widget_nodes = meet_nodes.clone();
    support_widget_nodes[2] = fixture_node(2, "AXGroup", "Support chat", &[1]);
    assert!(validated_chat_scope(&MeetingPlatform::GoogleMeet, &support_widget_nodes).is_none());
}

#[test]
fn test_browser_chat_scope_accepts_explicit_heading_beside_the_composer() {
    let meet_nodes = vec![
        fixture_node(0, "AXWebArea", "Team sync - Google Meet", &[]),
        fixture_node(1, "AXButton", "Leave call", &[0]),
        fixture_node(2, "AXGroup", "", &[1]),
        fixture_node(3, "AXHeading", "In-call messages", &[1, 0]),
        fixture_composer(4, "Send a message", &[1, 1, 0]),
    ];

    assert_eq!(
        validated_chat_scope(&MeetingPlatform::GoogleMeet, &meet_nodes),
        Some((vec![1], vec![1, 1, 0]))
    );
}

#[test]
fn test_teams_capture_accepts_deep_light_meeting_composer_below_explicit_heading() {
    let composer_path = vec![1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let nodes = vec![
        fixture_node(0, "AXWebArea", "Microsoft Teams meeting", &[]),
        fixture_node(1, "AXButton", "Leave", &[0]),
        fixture_node(2, "AXGroup", "", &[1]),
        fixture_node(3, "AXHeading", "Meeting chat", &[1, 0, 0, 0]),
        fixture_composer(4, "Type a message", &composer_path),
    ];

    assert_eq!(
        validated_chat_capture_scope(&MeetingPlatform::MicrosoftTeams, &nodes),
        Some((vec![1], composer_path))
    );
    assert!(validated_chat_scope(&MeetingPlatform::MicrosoftTeams, &nodes).is_none());
}

#[test]
fn test_platform_chat_adapters_validate_the_requested_provider_matrix() {
    for (platform, exit_label, scope_label, composer_label) in [
        (
            MeetingPlatform::GoogleMeet,
            "Leave call",
            "In-call messages",
            "Send a message",
        ),
        (
            MeetingPlatform::MicrosoftTeams,
            "Hang up",
            "Meeting chat",
            "Type a message",
        ),
        (
            MeetingPlatform::Zoom,
            "Leave meeting",
            "Chat",
            "Message everyone",
        ),
        (
            MeetingPlatform::Webex,
            "Leave meeting",
            "Chat with everyone",
            "Type a message",
        ),
    ] {
        let nodes = vec![
            fixture_node(0, "AXWebArea", "Meeting", &[]),
            fixture_node(1, "AXButton", exit_label, &[0]),
            fixture_node(2, "AXGroup", scope_label, &[1]),
            fixture_composer(3, composer_label, &[1, 0]),
        ];

        assert_eq!(
            validated_chat_scope(&platform, &nodes),
            Some((vec![1], vec![1, 0])),
            "adapter did not validate {platform:?}"
        );

        let mut send = fixture_node(4, "AXButton", "Send", &[1, 1]);
        assert!(
            is_platform_send_button(&platform, &send, &[1]),
            "send button was not accepted for {platform:?}"
        );
        send.tree_path = vec![2, 0];
        assert!(
            !is_platform_send_button(&platform, &send, &[1]),
            "out-of-scope send button was accepted for {platform:?}"
        );
    }

    let slack_nodes = vec![
        fixture_node(0, "AXWebArea", "Huddle in test", &[]),
        fixture_node(1, "AXButton", "Leave huddle", &[0]),
        fixture_node(2, "AXGroup", "Thread in test", &[1]),
        fixture_composer(3, "Message to test", &[1, 0]),
    ];
    assert_eq!(
        validated_chat_scope(&MeetingPlatform::Slack, &slack_nodes),
        Some((vec![1], vec![1, 0]))
    );

    let mut ordinary_channel_nodes = slack_nodes;
    ordinary_channel_nodes[2] = fixture_node(2, "AXGroup", "Channel test", &[1]);
    assert!(validated_chat_scope(&MeetingPlatform::Slack, &ordinary_channel_nodes).is_none());
}

#[test]
fn test_zoom_web_chat_scope_accepts_live_message_list_and_composer_labels() {
    let nodes = vec![
        fixture_node(0, "AXWebArea", "John Jeong's Zoom Meeting", &[]),
        fixture_node(1, "AXButton", "Leave", &[0]),
        fixture_node(2, "AXGroup", "Chat Message List", &[4, 0]),
        fixture_composer(3, "Type message here ...", &[4, 1, 0]),
    ];

    assert_eq!(
        validated_chat_capture_scope(&MeetingPlatform::Zoom, &nodes),
        Some((vec![4], vec![4, 1, 0]))
    );
}

#[test]
fn test_zoom_web_chat_scope_ignores_help_text_when_matching_exact_label() {
    let mut scope = fixture_node(2, "AXGroup", "Chat", &[4, 0]);
    scope.description = Some("View messages shared during this meeting".to_string());
    let nodes = vec![
        fixture_node(0, "AXWebArea", "John Jeong's Zoom Meeting", &[]),
        fixture_node(1, "AXButton", "Leave", &[0]),
        scope,
        fixture_composer(3, "Type message here ...", &[4, 1, 0]),
    ];

    assert_eq!(
        validated_chat_capture_scope(&MeetingPlatform::Zoom, &nodes),
        Some((vec![4], vec![4, 1, 0]))
    );
}

#[test]
fn test_native_webex_chat_scope_accepts_live_history_and_composer_labels() {
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
    ];

    assert_eq!(
        validated_chat_capture_scope(&MeetingPlatform::Webex, &nodes),
        Some((vec![1], vec![1, 1]))
    );
}

#[test]
fn test_native_linux_webex_capture_accepts_read_only_atspi_composer() {
    let nodes = vec![
        fixture_node(0, "AXWindow", "John's meeting", &[]),
        fixture_node(1, "AXButton", "Leave meeting", &[1, 0]),
        fixture_node(
            2,
            "AXGroup",
            "Chat Tab list, Everyone tab",
            &[1, 1, 0, 1, 0],
        ),
        fixture_node(
            3,
            "AXStaticText",
            "Write a message to everyone. Press Shift + Enter for new line.",
            &[1, 1, 0, 2, 0, 2, 0, 5, 4, 0],
        ),
    ];

    assert_eq!(
        validated_chat_capture_scope(&MeetingPlatform::Webex, &nodes),
        Some((vec![1, 1, 0], vec![1, 1, 0, 2, 0, 2, 0, 5, 4, 0]))
    );
    assert!(validated_chat_scope(&MeetingPlatform::Webex, &nodes).is_none());
}

#[test]
fn test_webex_web_chat_scope_accepts_live_popup_leave_and_named_composer() {
    let nodes = vec![
        fixture_node(0, "AXWebArea", "In meeting · Meeting · Webex", &[]),
        fixture_node(1, "AXPopUpButton", "Leave meeting", &[0]),
        fixture_node(2, "AXGroup", "Chat with Everyone", &[1]),
        fixture_composer(3, "Write a message to John Jeong's meeting", &[1, 0]),
    ];

    assert_eq!(
        validated_chat_capture_scope(&MeetingPlatform::Webex, &nodes),
        Some((vec![1], vec![1, 0]))
    );
}

#[test]
fn test_firefox_webex_chat_scope_accepts_combobox_composer() {
    let mut composer = fixture_node(
        3,
        "AXComboBox",
        "Write a message to John Jeong's meeting",
        &[1, 0],
    );
    composer.settable_value = true;
    let nodes = vec![
        fixture_node(0, "AXWebArea", "In meeting · Meeting · Webex", &[]),
        fixture_node(1, "AXButton", "Leave meeting", &[0]),
        fixture_node(2, "AXGroup", "Chat with Everyone", &[1]),
        composer,
    ];

    assert_eq!(
        validated_chat_capture_scope(&MeetingPlatform::Webex, &nodes),
        Some((vec![1], vec![1, 0]))
    );
}

#[test]
fn test_safari_webex_chat_scope_accepts_repeated_accessible_label() {
    let mut scope = fixture_node(2, "AXGroup", "Chat with Everyone", &[1]);
    scope.description = Some("Chat with Everyone".to_string());
    let mut composer = fixture_node(
        3,
        "AXComboBox",
        "Write a message to John Jeong's meeting",
        &[1, 0],
    );
    composer.settable_value = true;
    let nodes = vec![
        fixture_node(0, "AXWebArea", "In meeting · Meeting · Webex", &[]),
        fixture_node(1, "AXPopUpButton", "Leave meeting", &[0]),
        scope,
        composer,
    ];

    assert_eq!(
        validated_chat_capture_scope(&MeetingPlatform::Webex, &nodes),
        Some((vec![1], vec![1, 0]))
    );
}

#[test]
fn test_teams_and_webex_reject_generic_chat_containers() {
    for (platform, exit_label, composer_label) in [
        (MeetingPlatform::MicrosoftTeams, "Hang up", "Type a message"),
        (MeetingPlatform::Webex, "Leave meeting", "Send a message"),
    ] {
        let nodes = vec![
            fixture_node(0, "AXWebArea", "Meeting", &[]),
            fixture_node(1, "AXButton", exit_label, &[0]),
            fixture_node(2, "AXGroup", "Chat", &[1]),
            fixture_composer(3, composer_label, &[1, 0]),
        ];

        assert!(
            validated_chat_scope(&platform, &nodes).is_none(),
            "generic chat unexpectedly validated for {platform:?}"
        );
    }
}

#[test]
fn test_web_capture_extracts_only_the_validated_chat_subtree() {
    let nodes = vec![
        fixture_node(0, "AXWebArea", "Team sync - Google Meet", &[]),
        fixture_node(1, "AXButton", "Leave call", &[0]),
        fixture_node(2, "AXGroup", "In-call messages", &[1]),
        fixture_composer(3, "Send a message", &[1, 0]),
        fixture_node(
            4,
            "AXGroup",
            "Ada Lovelace\n10:42 AM\nDiscuss the rollout https://example.com/plan",
            &[1, 1],
        ),
        fixture_node(
            5,
            "AXGroup",
            "Mallory\n10:43 AM\nUnrelated browser content",
            &[2, 0],
        ),
    ];

    let messages =
        extract_chat_messages(&MeetingPlatform::GoogleMeet, &MeetingSurface::Web, &nodes);

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].sender.as_deref(), Some("Ada Lovelace"));
    assert_eq!(messages[0].timestamp.as_deref(), Some("10:42 AM"));
    assert_eq!(
        messages[0].text,
        "Discuss the rollout https://example.com/plan"
    );
    assert_eq!(messages[0].links, vec!["https://example.com/plan"]);
}

#[test]
fn test_web_capture_ignores_aggregate_containers() {
    let mut aggregate_list = fixture_node(4, "AXList", "Message list", &[1, 1]);
    aggregate_list.value = Some("Mallory\n10:41 AM\nAggregated list value".to_string());
    let nodes = vec![
        fixture_node(0, "AXWebArea", "Team sync - Google Meet", &[]),
        fixture_node(1, "AXButton", "Leave call", &[0]),
        fixture_node(2, "AXGroup", "In-call messages", &[1]),
        fixture_composer(3, "Send a message", &[1, 0]),
        aggregate_list,
        fixture_node(
            5,
            "AXGroup",
            "Ada Lovelace\n10:42 AM\nFirst message\nGrace Hopper\n10:43 AM\nSecond message",
            &[1, 2],
        ),
        fixture_node(
            6,
            "AXGroup",
            "Ada Lovelace\n10:42 AM\nFirst message",
            &[1, 2, 0],
        ),
        fixture_node(
            7,
            "AXGroup",
            "Grace Hopper\n10:43 AM\nSecond message",
            &[1, 2, 1],
        ),
    ];

    let messages =
        extract_chat_messages(&MeetingPlatform::GoogleMeet, &MeetingSurface::Web, &nodes);

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].text, "First message");
    assert_eq!(messages[1].text, "Second message");
}

#[test]
fn test_aside_blank_web_area_uses_meet_code_identity() {
    let nodes = vec![
        fixture_node(0, "AXWebArea", "", &[]),
        fixture_node(1, "AXButton", "Leave call", &[0]),
        fixture_node(2, "AXGroup", "In-call messages", &[1]),
        fixture_composer(3, "Send a message", &[1, 0]),
    ];
    let root = |title: &str| BrowserMeetingRoot {
        platform: MeetingPlatform::GoogleMeet,
        window_title: Some(title.to_string()),
        web_area_url: Some("about:blank".into()),
        nodes: nodes.clone(),
    };

    let first = browser_capture_context_id(&root("Meet - jyz-nspz-tzk")).unwrap();
    let same = browser_capture_context_id(&root("Meet - jyz-nspz-tzk - Aside")).unwrap();
    let other = browser_capture_context_id(&root("Meet - abc-defg-hij")).unwrap();

    assert_eq!(first, same);
    assert_ne!(first, other);
}

#[test]
fn test_linux_browser_context_falls_back_to_titled_active_call_identity() {
    let nodes = vec![
        fixture_node(0, "AXWebArea", "John Jeong's Zoom Meeting", &[]),
        fixture_node(1, "AXButton", "Leave", &[0]),
        fixture_node(2, "AXGroup", "Chat Message List", &[1]),
        fixture_composer(3, "Type message here ...", &[1, 0]),
    ];
    let root = |title: &str| BrowserMeetingRoot {
        platform: MeetingPlatform::Zoom,
        window_title: Some(title.to_string()),
        web_area_url: None,
        nodes: nodes.clone(),
    };

    let first =
        browser_capture_context_id(&root("John Jeong's Zoom Meeting - Google Chrome")).unwrap();
    let same =
        browser_capture_context_id(&root("John Jeong's Zoom Meeting - Google Chrome")).unwrap();
    let next =
        browser_capture_context_id(&root("Grace Hopper's Zoom Meeting - Google Chrome")).unwrap();

    assert_eq!(first, same);
    assert_ne!(first, next);
}

#[test]
fn test_linux_meet_context_without_code_falls_back_to_titled_active_call_identity() {
    let nodes = vec![
        fixture_node(0, "AXWebArea", "Team sync - Google Meet", &[]),
        fixture_node(1, "AXButton", "Leave call", &[0]),
        fixture_node(2, "AXGroup", "In-call messages", &[1]),
        fixture_composer(3, "Send a message", &[1, 0]),
    ];
    let root = BrowserMeetingRoot {
        platform: MeetingPlatform::GoogleMeet,
        window_title: Some("Team sync - Google Meet - Google Chrome".to_string()),
        web_area_url: None,
        nodes,
    };

    assert!(browser_capture_context_id(&root).is_some());
}

#[test]
fn test_browser_context_preserves_query_identified_meeting_identity() {
    let teams_nodes = vec![
        fixture_node(0, "AXWebArea", "Microsoft Teams", &[]),
        fixture_node(1, "AXButton", "Hang up", &[0]),
        fixture_node(2, "AXGroup", "Meeting chat", &[1]),
        fixture_composer(3, "Type a message", &[1, 0]),
    ];
    let root = |url: &str| BrowserMeetingRoot {
        platform: MeetingPlatform::MicrosoftTeams,
        window_title: Some("Microsoft Teams meeting".to_string()),
        web_area_url: Some(url.to_string()),
        nodes: teams_nodes.clone(),
    };

    let first = browser_capture_context_id(&root(
        "https://teams.microsoft.com/v2/?meetingId=first#fragment",
    ))
    .unwrap();
    let same_without_fragment =
        browser_capture_context_id(&root("https://teams.microsoft.com/v2/?meetingId=first"))
            .unwrap();
    let second =
        browser_capture_context_id(&root("https://teams.microsoft.com/v2/?meetingId=second"))
            .unwrap();

    assert_eq!(first, same_without_fragment);
    assert_ne!(first, second);
}

#[test]
fn test_capture_context_ignores_volatile_titles_and_tree_paths() {
    let nodes = |scope_path: &[usize], composer_path: &[usize], root_role: &str| {
        vec![
            fixture_node(0, root_role, "Microsoft Teams", &[]),
            fixture_node(1, "AXButton", "Hang up", &[0]),
            fixture_node(2, "AXGroup", "Meeting chat", scope_path),
            fixture_composer(3, "Type a message", composer_path),
        ]
    };
    let browser_root =
        |title: &str, scope_path: &[usize], composer_path: &[usize]| BrowserMeetingRoot {
            platform: MeetingPlatform::MicrosoftTeams,
            window_title: Some(title.to_string()),
            web_area_url: Some("https://teams.microsoft.com/v2/?meetingId=stable".to_string()),
            nodes: nodes(scope_path, composer_path, "AXWebArea"),
        };

    let first = browser_capture_context_id(&browser_root("Microsoft Teams meeting", &[1], &[1, 0]))
        .unwrap();
    let shifted =
        browser_capture_context_id(&browser_root("(1) Microsoft Teams meeting", &[5], &[5, 2]))
            .unwrap();

    assert_eq!(first, shifted);

    let first_native = NativeMeetingRoot {
        window_title: Some("Microsoft Teams meeting".to_string()),
        nodes: nodes(&[1], &[1, 0], "AXWindow"),
    };
    let shifted_native = NativeMeetingRoot {
        window_title: Some("Microsoft Teams meeting · new activity".to_string()),
        nodes: nodes(&[5], &[5, 2], "AXWindow"),
    };

    assert_eq!(
        native_capture_context_id(&MeetingPlatform::MicrosoftTeams, &first_native),
        native_capture_context_id(&MeetingPlatform::MicrosoftTeams, &shifted_native)
    );
}
