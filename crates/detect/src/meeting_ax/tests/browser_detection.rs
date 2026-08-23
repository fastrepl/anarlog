use super::*;

#[test]
fn test_aside_meet_code_title_classifies_without_meeting_url() {
    let web_area = node(16, "AXWebArea", "", None);
    assert_eq!(
        classify_browser_context(
            Some("about:blank"),
            Some("Meet - jyz-nspz-tzk"),
            Some(&web_area),
            &[],
        ),
        MeetingPlatform::GoogleMeet
    );
    assert!(browser_window_has_provider_signal(
        Some("about:blank"),
        Some("Meet - jyz-nspz-tzk - Aside"),
    ));
    assert!(!browser_title_platform_signals("Meet - notes").contains(&MeetingPlatform::GoogleMeet));
}

#[test]
fn test_browser_title_classifies_meet_web() {
    let web_area = node(16, "AXWebArea", "Team sync - Google Meet", None);
    assert_eq!(
        classify_browser_context(
            Some("https://meet.google.com/abc-defg-hij"),
            Some("Team sync - Google Meet - Google Chrome"),
            Some(&web_area),
            &[],
        ),
        MeetingPlatform::GoogleMeet
    );
    assert_eq!(
        classify_surface("com.google.Chrome", &MeetingPlatform::GoogleMeet),
        MeetingSurface::Web
    );
}

#[test]
fn test_zoom_title_and_bounded_leave_classify_without_an_exposed_url() {
    let web_area = node(16, "AXWebArea", "John Jeong's Zoom Meeting", None);
    let leave = node(
        17,
        "AXButton",
        "Leave",
        Some(AxRect {
            x: 10.0,
            y: 10.0,
            width: 120.0,
            height: 40.0,
        }),
    );

    assert_eq!(
        classify_browser_context(
            None,
            Some("John Jeong's Zoom Meeting - Google Chrome"),
            Some(&web_area),
            &[leave],
        ),
        MeetingPlatform::Zoom
    );
}

#[test]
fn test_teams_title_and_bounded_leave_classify_without_an_exposed_url() {
    let web_area = node(
        16,
        "AXWebArea",
        "Microsoft Teams meeting | Microsoft Teams",
        None,
    );
    let leave = node(
        17,
        "AXButton",
        "Leave",
        Some(AxRect {
            x: 10.0,
            y: 10.0,
            width: 120.0,
            height: 40.0,
        }),
    );

    assert_eq!(
        classify_browser_context(
            None,
            Some("Microsoft Teams meeting | Microsoft Teams - Microsoft Edge"),
            Some(&web_area),
            &[leave],
        ),
        MeetingPlatform::MicrosoftTeams
    );
}

#[test]
fn test_browser_background_tab_nodes_cannot_classify_active_window() {
    let background_meet_node = node(
        17,
        "AXButton",
        "Team sync - Google Meet background tab",
        None,
    );

    assert_eq!(
        classify_platform(
            "com.google.Chrome",
            Some("Inbox - Google Chrome"),
            &[background_meet_node],
            MeetingPlatform::Unknown,
        ),
        MeetingPlatform::Unknown
    );
}

#[test]
fn test_browser_active_web_area_can_validate_one_platform_but_not_conflicts() {
    let meet_web_area = node(18, "AXWebArea", "Team sync - Google Meet", None);
    let generic_web_area = node(19, "AXWebArea", "Document", None);
    assert_eq!(
        classify_browser_context(
            Some("https://meet.google.com/abc-defg-hij"),
            Some("Google Chrome"),
            Some(&meet_web_area),
            &[],
        ),
        MeetingPlatform::GoogleMeet
    );

    assert_eq!(
        classify_browser_context(
            Some("https://meet.google.com/abc-defg-hij"),
            Some("Zoom Meeting - Google Chrome"),
            Some(&meet_web_area),
            &[],
        ),
        MeetingPlatform::Unknown
    );

    assert_eq!(
        classify_browser_context(
            Some("https://meet.google.com/abc-defg-hij"),
            Some("Google Chrome"),
            Some(&generic_web_area),
            &[],
        ),
        MeetingPlatform::Unknown
    );
    assert_eq!(
        classify_browser_context(
            Some("https://meet.google.com/abc-defg-hij"),
            Some("Google Chrome"),
            Some(&generic_web_area),
            &[node(20, "AXButton", "Leave call", None)],
        ),
        MeetingPlatform::GoogleMeet
    );

    assert_eq!(
        classify_browser_context(
            Some("https://www.google.com/search?q=Google+Meet"),
            Some("Google Meet - Google Search"),
            Some(&meet_web_area),
            &[],
        ),
        MeetingPlatform::Unknown
    );
}

#[test]
fn test_browser_meeting_origins_are_exact_and_https_only() {
    for (url, platform) in [
        (
            "https://meet.google.com/abc-defg-hij",
            MeetingPlatform::GoogleMeet,
        ),
        (
            "https://teams.microsoft.com/v2/",
            MeetingPlatform::MicrosoftTeams,
        ),
        (
            "https://teams.live.com/meet/123",
            MeetingPlatform::MicrosoftTeams,
        ),
        ("https://app.zoom.us/wc/123", MeetingPlatform::Zoom),
        (
            "https://fastrepl.webex.com/meet/test",
            MeetingPlatform::Webex,
        ),
        (
            "https://app.slack.com/client/workspace/channel",
            MeetingPlatform::Slack,
        ),
    ] {
        assert_eq!(browser_platform_from_url(Some(url)), Some(platform));
    }

    for url in [
        "http://meet.google.com/abc-defg-hij",
        "https://meet.google.com.evil.example/abc-defg-hij",
        "https://teams.microsoft.com.evil.example/v2/",
        "https://zoom.us.evil.example/wc/123",
        "https://webex.com.evil.example/meet/test",
        "https://slack.com.evil.example/client/workspace/channel",
        "javascript:alert(1)",
    ] {
        assert_eq!(browser_platform_from_url(Some(url)), None, "accepted {url}");
    }
}

#[test]
fn test_meet_chat_scope_accepts_chromium_webkit_and_gecko_role_variants() {
    for (container_role, composer_role) in [
        ("AXGroup", "AXTextArea"),
        ("AXScrollArea", "AXTextField"),
        ("AXList", "AXTextArea"),
    ] {
        let mut composer = fixture_composer(3, "Send a message", &[1, 0]);
        composer.role = Some(composer_role.to_string());
        let nodes = vec![
            fixture_node(0, "AXWebArea", "Team sync - Google Meet", &[]),
            fixture_node(1, "AXButton", "Leave call", &[0]),
            fixture_node(2, container_role, "In-call messages", &[1]),
            composer,
        ];

        assert_eq!(
            validated_chat_scope(&MeetingPlatform::GoogleMeet, &nodes),
            Some((vec![1], vec![1, 0]))
        );
    }
}

#[test]
fn test_browser_meeting_window_scope_must_be_unique() {
    assert_eq!(unique_scope_for_count(0), UniqueMatch::Missing);
    assert_eq!(unique_scope_for_count(1), UniqueMatch::One(0));
    assert_eq!(unique_scope_for_count(2), UniqueMatch::Ambiguous);
    assert_eq!(unique_scope_for_search(1, true), UniqueMatch::One(0));
    assert_eq!(unique_scope_for_search(1, false), UniqueMatch::Ambiguous);
}

#[test]
fn test_webex_native_bundle_classifies_native() {
    assert_eq!(
        classify_bundle("Cisco-Systems.Spark"),
        MeetingPlatform::Webex
    );
    assert_eq!(
        classify_surface("Cisco-Systems.Spark", &MeetingPlatform::Webex),
        MeetingSurface::Native
    );
}

#[test]
fn test_incomplete_native_webex_snapshot_is_read_only() {
    let nodes = vec![fixture_node(
        0,
        "AXButton",
        "Leave meeting or end meeting for everyone",
        &[0],
    )];

    assert!(
        native_meeting_root_from_snapshot(
            &MeetingPlatform::Webex,
            Some("John's meeting".into()),
            nodes.clone(),
            false,
            false,
        )
        .is_some()
    );
    assert!(
        native_meeting_root_from_snapshot(
            &MeetingPlatform::Webex,
            Some("John's meeting".into()),
            nodes,
            false,
            true,
        )
        .is_none()
    );
}

#[test]
fn test_webex_browser_title_classifies_web() {
    let web_area = node(21, "AXWebArea", "Cisco Webex Meetings", None);
    assert_eq!(
        classify_browser_context(
            Some("https://fastrepl.webex.com/meet/team"),
            Some("Cisco Webex Meetings - Brave Browser"),
            Some(&web_area),
            &[],
        ),
        MeetingPlatform::Webex
    );
    assert_eq!(
        classify_surface("com.brave.Browser", &MeetingPlatform::Webex),
        MeetingSurface::Web
    );
}

#[test]
fn test_current_webex_in_meeting_title_classifies_without_an_exposed_url() {
    let web_area = node(22, "AXWebArea", "In meeting · Meeting · Webex", None);
    let leave = node(
        23,
        "AXButton",
        "Leave meeting",
        Some(AxRect {
            x: 10.0,
            y: 10.0,
            width: 40.0,
            height: 40.0,
        }),
    );

    assert_eq!(
        classify_browser_context(
            None,
            Some("In meeting · Meeting · Webex - Google Chrome"),
            Some(&web_area),
            &[leave],
        ),
        MeetingPlatform::Webex
    );
}

#[test]
fn test_current_webex_browser_window_classifies_from_url_and_popup_leave_control() {
    let web_area = node(21, "AXWebArea", "In meeting · Meeting · Webex", None);
    let leave = node(
        22,
        "AXPopUpButton",
        "Leave meeting",
        Some(AxRect {
            x: 10.0,
            y: 10.0,
            width: 120.0,
            height: 40.0,
        }),
    );

    assert_eq!(
        classify_browser_context(
            Some("https://meet1754330889177-4096.webex.com/wbxmjs/joinservice"),
            Some("In meeting · Meeting · Webex - Google Chrome (Incognito)"),
            Some(&web_area),
            &[leave],
        ),
        MeetingPlatform::Webex
    );
}

#[test]
fn test_native_webex_excludes_multitasking_floating_window() {
    let nodes = vec![node(
        1,
        "AXButton",
        "Leave meeting or end meeting for everyone",
        Some(AxRect {
            x: 10.0,
            y: 10.0,
            width: 120.0,
            height: 40.0,
        }),
    )];

    assert!(
        native_meeting_root_from_snapshot(
            &MeetingPlatform::Webex,
            Some("Webex multitasking floating window".into()),
            nodes,
            true,
            false,
        )
        .is_none()
    );
}

#[test]
fn test_only_provider_like_browser_windows_poison_incomplete_capture() {
    assert!(!browser_window_has_provider_signal(
        Some("https://mail.google.com/mail/u/0/#inbox"),
        Some("Inbox - Gmail"),
    ));
    assert!(browser_window_has_provider_signal(
        Some("https://meet.google.com/abc-defg-hij"),
        Some("Weekly planning - Google Meet"),
    ));
    assert!(browser_window_has_provider_signal(
        None,
        Some("Team sync | Microsoft Teams"),
    ));
}

#[test]
fn test_select_child_walk_prefers_visible_subset() {
    assert_eq!(
        select_child_walk(Some(380), Some(42), true),
        Some(ChildWalk::Visible)
    );
    assert_eq!(
        select_child_walk(Some(42), Some(20), true),
        Some(ChildWalk::Children)
    );
    assert_eq!(
        select_child_walk(Some(380), Some(42), false),
        Some(ChildWalk::Children)
    );
    assert_eq!(
        select_child_walk(Some(12), Some(0), true),
        Some(ChildWalk::Children)
    );
    assert_eq!(
        select_child_walk(Some(0), Some(8), true),
        Some(ChildWalk::Visible)
    );
    assert_eq!(
        select_child_walk(None, Some(3), false),
        Some(ChildWalk::Visible)
    );
    assert_eq!(select_child_walk(None, None, true), None);
}

#[test]
fn test_chat_priority_labels_prefer_meet_chat_over_video_tiles() {
    assert!(is_chat_priority_label("In-call messages"));
    assert!(is_chat_priority_label("Send a message"));
    assert!(is_chat_priority_label("Type message here ..."));
    assert!(is_chat_priority_label("Chat Message List"));
    assert!(is_chat_priority_label("Open the chat panel"));
    assert!(is_chat_priority_label("Leave call"));
    assert!(is_chat_priority_label("Chat"));
    assert!(!is_chat_priority_label("Ada Lovelace"));
    assert!(!is_chat_priority_label("Your video is on"));
}

#[test]
fn test_truncated_browser_meet_snapshot_is_accepted_when_uniquely_classified() {
    let web_area = fixture_node(0, "AXWebArea", "Team sync - Google Meet", &[]);
    let nodes = vec![
        web_area.clone(),
        fixture_node(1, "AXButton", "Leave call", &[0]),
        fixture_node(2, "AXGroup", "In-call messages", &[1]),
        fixture_composer(3, "Send a message", &[1, 0]),
    ];

    let BrowserMeetingSnapshot::Accept(root) = browser_meeting_root_from_snapshot(
        nodes,
        false,
        Some("https://meet.google.com/abc-defg-hij".into()),
        Some("Team sync - Google Meet - Aside".into()),
        Some(&web_area),
    ) else {
        panic!("expected a uniquely classified Meet root to survive AX truncation");
    };

    assert_eq!(root.platform, MeetingPlatform::GoogleMeet);
}

#[test]
fn test_truncated_meeting_like_window_stays_unscoped_without_classification() {
    let web_area = fixture_node(0, "AXWebArea", "Document", &[]);

    assert!(matches!(
        browser_meeting_root_from_snapshot(
            vec![web_area.clone()],
            false,
            Some("https://meet.google.com/abc-defg-hij".into()),
            Some("Google Chrome".into()),
            Some(&web_area),
        ),
        BrowserMeetingSnapshot::Unscoped
    ));
}

#[test]
fn test_validated_browser_bundles_are_web_surfaces() {
    for bundle_id in [
        "com.google.Chrome",
        "com.microsoft.edgemac",
        "org.mozilla.firefox",
        "com.apple.Safari",
        "com.brave.Browser",
        "com.vivaldi.Vivaldi",
        "com.operasoftware.Opera",
        "company.thebrowser.Browser",
        "com.browseros.BrowserOS",
        "ai.perplexity.comet",
        "at.studio.AsideBrowser",
        "company.thebrowser.dia",
        "com.sigmaos.sigmaos.macos",
        "net.imput.helium",
        "com.nousresearch.hermes",
        "app.zen-browser.zen",
    ] {
        assert!(
            is_browser_bundle(bundle_id),
            "expected {bundle_id} to be treated as a browser"
        );
        assert_eq!(
            classify_surface(bundle_id, &MeetingPlatform::Zoom),
            MeetingSurface::Web
        );
    }
}
