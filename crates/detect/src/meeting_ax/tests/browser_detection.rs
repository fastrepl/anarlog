use super::*;

#[test]
fn test_zoom_participant_roster_row_becomes_stream_candidate() {
    let streams = find_participant_streams(
        &MeetingPlatform::Zoom,
        &MeetingSurface::Native,
        &[node(
            11,
            "AXStaticText",
            "Ada Lovelace (Host, me, Participant ID:417329) No audio connected",
            None,
        )],
    );

    assert_eq!(streams.len(), 1);
    assert!(
        streams[0]
            .signals
            .contains(&"participant-row-label".to_string())
    );
    assert!(
        streams[0]
            .signals
            .contains(&"audio-state-label".to_string())
    );
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
        "ai.perplexity.comet",
        "at.studio.AsideBrowser",
        "company.thebrowser.dia",
        "com.sigmaos.sigmaos.macos",
        "net.imput.helium",
        "com.nousresearch.hermes",
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
