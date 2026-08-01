use super::*;

#[test]
fn test_participant_name_from_zoom_video_render_label() {
    assert_eq!(
        participant_name_from_evidence(
            &MeetingPlatform::Zoom,
            "Video render Ada Lovelace, Computer audio unmuted",
        ),
        Some("Ada Lovelace".to_string())
    );
}

#[test]
fn test_zoom_video_render_becomes_stream_candidate_with_audio_state() {
    let nodes = vec![node(
        7,
        "AXGroup",
        "Video render Ada Lovelace, Computer audio unmuted",
        Some(AxRect {
            x: 0.0,
            y: 0.0,
            width: 320.0,
            height: 180.0,
        }),
    )];

    let streams = find_participant_streams(&MeetingPlatform::Zoom, &MeetingSurface::Native, &nodes);

    assert_eq!(streams.len(), 1);
    assert_eq!(
        streams[0].participant_name,
        Some("Ada Lovelace".to_string())
    );
    assert!(!streams[0].is_active_speaker);
    assert!(streams[0].confidence > 0.6);
    assert!(
        streams[0]
            .signals
            .contains(&"audio-state-label".to_string())
    );
}

#[test]
fn test_explicit_active_speaker_label_marks_stream_active() {
    let nodes = vec![node(
        9,
        "AXGroup",
        "Video render Grace Hopper, active speaker",
        Some(AxRect {
            x: 0.0,
            y: 0.0,
            width: 320.0,
            height: 180.0,
        }),
    )];

    let streams = find_participant_streams(&MeetingPlatform::Zoom, &MeetingSurface::Native, &nodes);

    assert_eq!(streams.len(), 1);
    assert!(streams[0].is_active_speaker);
    assert!(
        streams[0]
            .signals
            .contains(&"speaker-state-label".to_string())
    );
}

#[test]
fn test_self_view_speaker_label_marks_stream_active() {
    let nodes = vec![node(10, "AXRow", "Grace Hopper is speaking (You)", None)];

    let streams = find_participant_streams(&MeetingPlatform::Zoom, &MeetingSurface::Native, &nodes);

    assert_eq!(streams.len(), 1);
    assert_eq!(streams[0].participant_name.as_deref(), Some("Grace Hopper"));
    assert!(streams[0].is_active_speaker);
}

#[test]
fn test_active_speaker_is_retained_past_participant_limit() {
    let mut nodes = (0..24)
        .map(|index| {
            node(
                index,
                "AXGroup",
                &format!("Video render Participant {index}, Computer audio unmuted"),
                Some(AxRect {
                    x: 0.0,
                    y: 0.0,
                    width: 320.0,
                    height: 180.0,
                }),
            )
        })
        .collect::<Vec<_>>();
    nodes.push(node(24, "AXRow", "Ada Lovelace is speaking", None));

    let streams = find_participant_streams(&MeetingPlatform::Zoom, &MeetingSurface::Native, &nodes);

    assert_eq!(streams.len(), 24);
    assert!(streams.iter().any(|stream| {
        stream.is_active_speaker && stream.participant_name.as_deref() == Some("Ada Lovelace")
    }));
}

#[test]
fn test_zoom_prefers_named_speaker_state_over_generic_video_tile() {
    let mut tile = fixture_node(9, "AXGroup", "Video tile", &[0, 1]);
    tile.description = Some("Grace Hopper is speaking".to_string());
    tile.text = node_text(
        &tile.role,
        &tile.title,
        &tile.value,
        &tile.description,
        &tile.placeholder,
    );

    let streams =
        find_participant_streams(&MeetingPlatform::Zoom, &MeetingSurface::Native, &[tile]);

    assert_eq!(streams.len(), 1);
    assert_eq!(
        streams[0].participant_name,
        Some("Grace Hopper".to_string())
    );
    assert_eq!(streams[0].id, "ax-element-1009");
    assert!(streams[0].is_active_speaker);
}

#[test]
fn test_zoom_speaker_flag_uses_the_same_label_as_participant_name() {
    let mut roster = fixture_node(
        15,
        "AXStaticText",
        "Ada Lovelace (Host, me, Participant ID:417329) No audio connected",
        &[0, 1],
    );
    roster.description = Some("Grace Hopper is speaking".to_string());
    roster.text = node_text(
        &roster.role,
        &roster.title,
        &roster.value,
        &roster.description,
        &roster.placeholder,
    );

    let stream = candidate_stream(&MeetingPlatform::Zoom, &MeetingSurface::Native, &roster)
        .expect("expected Zoom roster participant");

    assert_eq!(stream.participant_name.as_deref(), Some("Ada Lovelace"));
    assert!(!stream.is_active_speaker);
    assert!(!stream.signals.contains(&"speaker-state-label".to_string()));
}

#[test]
fn test_participant_streams_deduplicate_repeated_named_ax_nodes() {
    let first = fixture_node(9, "AXGroup", "Grace Hopper is speaking", &[0, 1]);
    let second = fixture_node(10, "AXRow", "Grace Hopper is speaking", &[0, 1, 0]);

    let streams = find_participant_streams(
        &MeetingPlatform::Zoom,
        &MeetingSurface::Native,
        &[first, second],
    );

    assert_eq!(streams.len(), 1);
    assert_eq!(
        streams[0].participant_name,
        Some("Grace Hopper".to_string())
    );
}

#[test]
fn test_participant_streams_keep_distinct_people_with_the_same_name() {
    let first = fixture_node(11, "AXGroup", "Alex Kim is speaking", &[0, 1]);
    let second = fixture_node(12, "AXGroup", "Alex Kim is speaking", &[0, 2]);

    let streams = find_participant_streams(
        &MeetingPlatform::Zoom,
        &MeetingSurface::Native,
        &[first, second],
    );

    assert_eq!(streams.len(), 2);
    assert!(
        streams
            .iter()
            .all(|stream| stream.participant_name.as_deref() == Some("Alex Kim"))
    );
}

#[test]
fn test_slack_profile_row_is_participant_without_claiming_audio_is_speaking() {
    let streams = find_participant_streams(
        &MeetingPlatform::Slack,
        &MeetingSurface::Native,
        &[
            node(12, "AXCell", "View John Jeong's profile", None),
            node(13, "AXStaticText", "Audio", None),
        ],
    );

    assert_eq!(streams.len(), 1);
    assert_eq!(streams[0].participant_name, Some("John Jeong".to_string()));
    assert!(!streams[0].is_active_speaker);
}
