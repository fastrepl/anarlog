use std::collections::HashMap;

use super::{
    AxNode, MIN_VIDEO_AREA, MeetingCapturedChatMessage, MeetingChatDirection, MeetingChatTarget,
    MeetingParticipantStream, MeetingPlatform, MeetingSurface, node_labels, path_is_ancestor,
    validated_chat_scope,
};

pub(super) fn find_participant_streams(
    platform: &MeetingPlatform,
    surface: &MeetingSurface,
    nodes: &[AxNode],
) -> Vec<MeetingParticipantStream> {
    if *platform == MeetingPlatform::Unknown {
        return Vec::new();
    }

    let mut streams = nodes
        .iter()
        .filter_map(|node| candidate_stream(platform, surface, node).map(|stream| (stream, node)))
        .collect::<Vec<_>>();

    streams.sort_by(|a, b| {
        b.0.is_active_speaker
            .cmp(&a.0.is_active_speaker)
            .then_with(|| b.0.confidence.total_cmp(&a.0.confidence))
    });
    let mut retained_nodes: Vec<(String, &AxNode)> = Vec::new();
    streams.retain(|(stream, node)| {
        let duplicate = stream.participant_name.as_deref().is_some_and(|name| {
            retained_nodes.iter().any(|(retained_name, retained)| {
                retained_name.eq_ignore_ascii_case(name)
                    && same_participant_ax_identity(retained, node)
            })
        });
        if !duplicate && let Some(name) = stream.participant_name.clone() {
            retained_nodes.push((name, node));
        }
        !duplicate
    });
    let retained_limit = streams
        .iter()
        .filter(|(stream, _)| stream.is_active_speaker)
        .count()
        .max(24);
    streams.truncate(retained_limit);
    streams.into_iter().map(|(stream, _)| stream).collect()
}

fn same_participant_ax_identity(left: &AxNode, right: &AxNode) -> bool {
    left.element_hash
        .zip(right.element_hash)
        .is_some_and(|(left, right)| left == right)
        || (!left.tree_path.is_empty()
            && !right.tree_path.is_empty()
            && (left.tree_path.starts_with(&right.tree_path)
                || right.tree_path.starts_with(&left.tree_path)))
}

pub(super) fn is_zoom_meeting_evidence(node: &AxNode) -> bool {
    zoom_meeting_evidence_label(node).is_some()
}

fn zoom_meeting_evidence_label(node: &AxNode) -> Option<&str> {
    let role = node.role.as_deref()?;
    let labels = node_labels(node).collect::<Vec<_>>();
    let has_audio_state = labels.iter().any(|label| {
        let label = label.to_ascii_lowercase();
        label.contains("computer audio") || label.contains("no audio connected")
    });

    if matches!(role, "AXGroup" | "AXCell")
        && has_audio_state
        && let Some(label) = labels.iter().copied().find(|label| {
            let label = label.trim();
            let lower = label.to_ascii_lowercase();
            let is_video_render = lower
                .strip_prefix("video render ")
                .and_then(|rest| rest.split_once(','))
                .is_some_and(|(name, state)| {
                    !name.trim().is_empty()
                        && (state.contains("computer audio")
                            || state.contains("no audio connected"))
                });
            is_video_render || lower == "video tile"
        })
    {
        return Some(label);
    }

    if matches!(role, "AXStaticText" | "AXCell" | "AXRow" | "AXGroup") {
        return labels.into_iter().find(|label| {
            let lower = label.to_ascii_lowercase();
            lower.contains("participant id:")
                && (lower.contains("computer audio")
                    || lower.contains("no audio connected")
                    || lower.contains("(host")
                    || lower.contains("(me"))
        });
    }

    None
}

fn zoom_participant_evidence_label(node: &AxNode) -> Option<&str> {
    let role = node.role.as_deref()?;
    if matches!(role, "AXGroup" | "AXCell" | "AXRow")
        && let Some(label) = node_labels(node).find(|label| has_explicit_speaker_state(label))
    {
        return Some(label);
    }

    zoom_meeting_evidence_label(node)
}

fn slack_participant_evidence_label(node: &AxNode) -> Option<&str> {
    if node.role.as_deref() != Some("AXCell") {
        return None;
    }

    node_labels(node).find(|label| {
        let label = label.trim();
        let lower = label.to_ascii_lowercase();
        lower.starts_with("view ")
            && lower.ends_with("'s profile")
            && !label[5..label.len() - "'s profile".len()].trim().is_empty()
    })
}

fn explicit_web_speaker_evidence_label(node: &AxNode) -> Option<&str> {
    if !matches!(
        node.role.as_deref(),
        Some("AXGroup") | Some("AXCell") | Some("AXRow")
    ) {
        return None;
    }

    node_labels(node).find(|label| has_explicit_speaker_state(label))
}

fn participant_evidence_label<'a>(
    platform: &MeetingPlatform,
    surface: &MeetingSurface,
    node: &'a AxNode,
) -> Option<&'a str> {
    match (platform, surface) {
        (MeetingPlatform::Zoom, MeetingSurface::Native) => zoom_participant_evidence_label(node),
        (MeetingPlatform::Zoom, MeetingSurface::Web) => zoom_participant_evidence_label(node)
            .or_else(|| explicit_web_speaker_evidence_label(node)),
        (MeetingPlatform::Slack, MeetingSurface::Native) => slack_participant_evidence_label(node),
        (
            MeetingPlatform::GoogleMeet
            | MeetingPlatform::MicrosoftTeams
            | MeetingPlatform::Slack
            | MeetingPlatform::Webex,
            MeetingSurface::Web,
        ) => explicit_web_speaker_evidence_label(node),
        _ => None,
    }
}

pub(super) fn candidate_stream(
    platform: &MeetingPlatform,
    surface: &MeetingSurface,
    node: &AxNode,
) -> Option<MeetingParticipantStream> {
    let role = node.role.as_deref().unwrap_or_default();
    let text = node.text.as_str();
    let evidence_label = participant_evidence_label(platform, surface, node)?;
    let area = node
        .bounds
        .as_ref()
        .map(|r| r.width * r.height)
        .unwrap_or(0.0);
    let mut signals = Vec::new();
    let mut confidence = 0.55;

    if role == "AXGroup" && area >= MIN_VIDEO_AREA {
        confidence += 0.15;
        signals.push("large-group".to_string());
    }
    if evidence_label
        .to_ascii_lowercase()
        .starts_with("video render ")
        || evidence_label.trim().eq_ignore_ascii_case("video tile")
    {
        signals.push("video-label".to_string());
    } else {
        signals.push("participant-row-label".to_string());
    }
    if has_explicit_speaker_state(evidence_label) {
        confidence += 0.25;
        signals.push("speaker-state-label".to_string());
    }
    if text.contains("computer audio") || text.contains("no audio connected") {
        confidence += 0.15;
        signals.push("audio-state-label".to_string());
    }
    if area >= MIN_VIDEO_AREA {
        confidence += 0.15;
        signals.push("video-sized-bounds".to_string());
    }

    let label = Some(evidence_label.to_string());
    let participant_name = participant_name_from_evidence(platform, evidence_label);
    let is_active_speaker = signals.iter().any(|signal| signal == "speaker-state-label");

    Some(MeetingParticipantStream {
        id: node.element_hash.map_or_else(
            || format!("ax-node-{}", node.index),
            |hash| format!("ax-element-{hash:x}"),
        ),
        platform: platform.clone(),
        surface: surface.clone(),
        participant_name,
        label,
        bounds: node.bounds.clone(),
        confidence,
        is_active_speaker,
        signals,
    })
}

pub(super) fn participant_name_from_evidence(
    platform: &MeetingPlatform,
    evidence_label: &str,
) -> Option<String> {
    let label = evidence_label.trim();
    let lower = label.to_ascii_lowercase();
    match platform {
        MeetingPlatform::Zoom => {
            if lower == "video tile" {
                return None;
            }

            if let Some(name) = participant_name_from_speaker_label(label) {
                return Some(name);
            }

            if lower.starts_with("video render ") {
                return label["Video render ".len()..]
                    .split(',')
                    .next()
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(str::to_string);
            }

            let end = label
                .find('(')
                .or_else(|| lower.find("participant id:"))
                .unwrap_or(label.len());
            let name = label[..end].trim().trim_end_matches(',').trim();
            (!name.is_empty()).then(|| name.to_string())
        }
        MeetingPlatform::Slack if lower.starts_with("view ") && lower.ends_with("'s profile") => {
            Some(
                label["View ".len()..label.len() - "'s profile".len()]
                    .trim()
                    .to_string(),
            )
        }
        MeetingPlatform::GoogleMeet
        | MeetingPlatform::MicrosoftTeams
        | MeetingPlatform::Slack
        | MeetingPlatform::Webex => participant_name_from_speaker_label(label),
        MeetingPlatform::Discord | MeetingPlatform::Unknown => None,
    }
}

fn participant_name_from_speaker_label(label: &str) -> Option<String> {
    let label = label.trim();
    let lower = label.to_ascii_lowercase();
    let label = if lower.ends_with(" (you)") {
        &label[..label.len() - " (you)".len()]
    } else {
        label
    };
    let lower = label.to_ascii_lowercase();
    let name = if lower.starts_with("active speaker: ") {
        &label["active speaker: ".len()..]
    } else if lower.ends_with(" is speaking") {
        &label[..label.len() - " is speaking".len()]
    } else if let Some(index) = explicit_speaker_marker_index(&lower, ", active speaker") {
        &label[..index]
    } else if let Some(index) = explicit_speaker_marker_index(&lower, ", speaking") {
        &label[..index]
    } else {
        return None;
    };

    let name = name
        .trim()
        .trim_end_matches(" (You)")
        .trim_end_matches(" (you)")
        .trim();
    let name = if name.to_ascii_lowercase().starts_with("video render ") {
        name["video render ".len()..]
            .split(',')
            .next()
            .unwrap_or_default()
            .trim()
    } else {
        name
    };
    let is_false_state = matches!(
        name.to_ascii_lowercase().as_str(),
        "false" | "none" | "off" | "no"
    );
    (!name.is_empty() && !is_false_state && is_plausible_participant_name(name))
        .then(|| name.to_string())
}

fn is_plausible_participant_name(name: &str) -> bool {
    let name = name.trim();
    if name.is_empty()
        || name.chars().count() > 80
        || name
            .chars()
            .any(|character| matches!(character, '\n' | '\r' | '?' | '!'))
    {
        return false;
    }

    let words = name
        .split_whitespace()
        .map(|word| {
            word.trim_matches(|character: char| !character.is_alphanumeric())
                .to_ascii_lowercase()
        })
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    const GENERIC_SUBJECTS: &[&str] = &[
        "anybody",
        "anyone",
        "everybody",
        "everyone",
        "nobody",
        "participant",
        "participants",
        "person",
        "somebody",
        "someone",
        "speaker",
        "speakers",
        "what",
        "who",
    ];

    !words.is_empty()
        && words.len() <= 6
        && !words
            .iter()
            .any(|word| GENERIC_SUBJECTS.contains(&word.as_str()))
}

fn explicit_speaker_marker_index(label: &str, marker: &str) -> Option<usize> {
    let index = label.find(marker)?;
    let suffix = &label[index + marker.len()..];
    (suffix.is_empty() || suffix == " (you)").then_some(index)
}

fn has_explicit_speaker_state(label: &str) -> bool {
    participant_name_from_speaker_label(label).is_some()
}

pub(super) fn is_zoom_meeting_scope_node(node: &AxNode) -> bool {
    if node.role.as_deref() != Some("AXWindow") {
        return false;
    }

    let title = node.title.as_deref().unwrap_or_default().to_lowercase();
    title.contains("zoom meeting")
}

pub(super) fn is_zoom_chat_scope_node(node: &AxNode) -> bool {
    if node.identifier.as_deref() == Some("ZMTextMessageCellView") {
        return true;
    }

    node.role.as_deref() == Some("AXTable") && chat_scope_label(node).contains("chat list")
}

pub(super) fn slack_huddle_is_active(nodes: &[AxNode]) -> bool {
    nodes.iter().any(|node| {
        let role = node.role.as_deref().unwrap_or_default();
        let label = chat_scope_label(node);

        matches!(role, "AXButton" | "AXMenuItem")
            && (label.starts_with("leave huddle") || label.starts_with("end huddle"))
    })
}

pub(super) fn is_slack_huddle_scope_node(node: &AxNode) -> bool {
    let role = node.role.as_deref().unwrap_or_default();
    let label = chat_scope_label(node);
    let is_huddle_chat_label = label == "huddle"
        || label.contains("huddle chat")
        || label.contains("huddle thread")
        || label.contains("huddle messages")
        || label.contains("huddle conversation");

    match role {
        "AXWindow" => is_huddle_chat_label,
        "AXGroup" | "AXScrollArea" | "AXList" | "AXWebArea" | "AXSheet" => is_huddle_chat_label,
        "AXButton" | "AXMenuItem" => {
            label.contains("open huddle chat") || label.contains("show huddle chat")
        }
        _ => false,
    }
}

pub(super) fn chat_scope_label(node: &AxNode) -> String {
    [
        node.title.as_deref(),
        node.value.as_deref(),
        node.description.as_deref(),
        node.placeholder.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ")
    .to_lowercase()
}

pub(super) fn meeting_chat_surface_is_visible(
    platform: &MeetingPlatform,
    nodes: &[AxNode],
) -> bool {
    nodes.iter().any(|node| match platform {
        MeetingPlatform::Zoom => {
            node.within_zoom_meeting_scope
                && (node.within_zoom_chat_scope || is_explicit_chat_input(node))
        }
        MeetingPlatform::Slack => node.within_slack_huddle_scope && is_chat_input(node),
        _ => false,
    })
}

fn is_chat_input(node: &AxNode) -> bool {
    candidate_chat_target(node).is_some_and(|target| target.kind == "input")
}

pub(super) fn is_explicit_chat_input(node: &AxNode) -> bool {
    if !is_chat_input(node) {
        return false;
    }

    let label = chat_scope_label(node);
    label.contains("send a message")
        || label.contains("message everyone")
        || label.contains("type a message")
        || label.contains("meeting chat")
}

fn is_generic_chat_message_row_or_leaf(node: &AxNode, scope_path: &[usize]) -> bool {
    node.tree_path != scope_path
        && matches!(
            node.role.as_deref(),
            Some("AXStaticText")
                | Some("AXText")
                | Some("AXCell")
                | Some("AXRow")
                | Some("AXGroup")
        )
}

pub(super) fn extract_chat_messages(
    platform: &MeetingPlatform,
    surface: &MeetingSurface,
    nodes: &[AxNode],
) -> Vec<MeetingCapturedChatMessage> {
    if *platform == MeetingPlatform::Slack && !slack_huddle_is_active(nodes) {
        return Vec::new();
    }

    let requires_generic_scope = *surface == MeetingSurface::Web
        || matches!(
            platform,
            MeetingPlatform::MicrosoftTeams | MeetingPlatform::Webex
        );
    let generic_scope_path = if requires_generic_scope {
        let Some((scope_path, _)) = validated_chat_scope(platform, nodes) else {
            return Vec::new();
        };
        Some(scope_path)
    } else {
        None
    };

    let mut parsed_nodes = Vec::new();

    for node in nodes {
        if *platform == MeetingPlatform::Zoom
            && *surface == MeetingSurface::Native
            && (!node.within_zoom_meeting_scope || !node.within_zoom_chat_scope)
        {
            continue;
        }
        if *platform == MeetingPlatform::Slack
            && *surface == MeetingSurface::Native
            && !node.within_slack_huddle_scope
        {
            continue;
        }
        if generic_scope_path
            .as_ref()
            .is_some_and(|scope_path| !node.tree_path.starts_with(scope_path))
        {
            continue;
        }
        if generic_scope_path
            .as_ref()
            .is_some_and(|scope_path| !is_generic_chat_message_row_or_leaf(node, scope_path))
        {
            continue;
        }

        let Some(raw_text) = chat_message_text(node) else {
            continue;
        };
        let Some(parsed) = parse_chat_message(platform, &raw_text) else {
            continue;
        };
        parsed_nodes.push((node, parsed));
    }

    if generic_scope_path.is_some() {
        let parseable_paths = parsed_nodes
            .iter()
            .map(|(node, _)| node.tree_path.clone())
            .collect::<Vec<_>>();
        parsed_nodes.retain(|(node, _)| {
            !parseable_paths
                .iter()
                .any(|path| path_is_ancestor(&node.tree_path, path))
        });
    }

    let mut signature_counts = HashMap::<String, usize>::new();
    let mut parsed_paths = Vec::<(String, Vec<usize>)>::new();
    let mut messages = Vec::new();

    for (node, parsed) in parsed_nodes {
        let signature = format!(
            "{:?}|{}|{}|{}",
            platform,
            parsed.sender.as_deref().unwrap_or_default(),
            parsed.timestamp.as_deref().unwrap_or_default(),
            parsed.text
        );
        if generic_scope_path.is_some()
            && parsed_paths.iter().any(|(existing_signature, path)| {
                existing_signature == &signature
                    && (path == &node.tree_path
                        || path_is_ancestor(path, &node.tree_path)
                        || path_is_ancestor(&node.tree_path, path))
            })
        {
            continue;
        }
        parsed_paths.push((signature.clone(), node.tree_path.clone()));
        let source_identity = if let Some(element_hash) = node.element_hash {
            format!("cfhash={element_hash:x}")
        } else {
            let occurrence = signature_counts.entry(signature.clone()).or_default();
            *occurrence += 1;
            format!("occurrence={occurrence}")
        };

        messages.push(MeetingCapturedChatMessage {
            id: format!("ax-chat-{signature}|{source_identity}"),
            platform: platform.clone(),
            surface: surface.clone(),
            direction: meeting_chat_direction(platform, parsed.sender.as_deref()),
            sender: parsed.sender,
            timestamp: parsed.timestamp,
            links: extract_links(&parsed.text),
            text: parsed.text,
        });
    }

    if messages.len() > 80 {
        messages.drain(..messages.len() - 80);
    }
    messages
}

pub(super) fn meeting_chat_direction(
    platform: &MeetingPlatform,
    sender: Option<&str>,
) -> Option<MeetingChatDirection> {
    if *platform != MeetingPlatform::Zoom {
        return None;
    }

    sender.map(|sender| {
        let sender = sender.trim().to_lowercase();
        if matches!(sender.as_str(), "you" | "me") || sender.ends_with(" (you)") {
            MeetingChatDirection::Outgoing
        } else {
            MeetingChatDirection::Incoming
        }
    })
}

pub(super) struct ParsedChatMessage {
    pub(super) sender: Option<String>,
    pub(super) timestamp: Option<String>,
    pub(super) text: String,
}

fn chat_message_text(node: &AxNode) -> Option<String> {
    let role = node.role.as_deref().unwrap_or_default();
    if node.settable_value || matches!(role, "AXTextField" | "AXTextArea") {
        return None;
    }
    if candidate_chat_target(node).is_some_and(|target| {
        matches!(
            target.kind.as_str(),
            "input" | "sendButton" | "openChatControl"
        )
    }) {
        return None;
    }

    let value = node
        .value
        .as_deref()
        .or(node.title.as_deref())
        .or(node.description.as_deref())?;
    let text = normalize_chat_text(value);
    if text.len() < 2 || is_chat_chrome_text(&text) {
        return None;
    }

    Some(text)
}

pub(super) fn parse_chat_message(
    platform: &MeetingPlatform,
    raw_text: &str,
) -> Option<ParsedChatMessage> {
    match platform {
        MeetingPlatform::Zoom => {
            parse_zoom_chat_message(raw_text).or_else(|| parse_web_chat_message(raw_text))
        }
        MeetingPlatform::Slack => {
            parse_slack_chat_message(raw_text).or_else(|| parse_web_chat_message(raw_text))
        }
        MeetingPlatform::GoogleMeet | MeetingPlatform::MicrosoftTeams | MeetingPlatform::Webex => {
            parse_web_chat_message(raw_text)
        }
        MeetingPlatform::Discord | MeetingPlatform::Unknown => None,
    }
}

fn parse_web_chat_message(raw_text: &str) -> Option<ParsedChatMessage> {
    let lines = chat_lines(raw_text);
    let first = lines.first()?.as_str();

    if lines.len() == 1 {
        let (sender_and_text, timestamp) = first.rsplit_once(", ")?;
        if !looks_like_time(timestamp) {
            return None;
        }
        let (sender, text) = sender_and_text.split_once(", ")?;
        let sender = sender.trim();
        let text = text.trim();
        return (looks_like_chat_sender(sender) && !text.is_empty() && !is_chat_chrome_text(text))
            .then(|| ParsedChatMessage {
                sender: Some(sender.to_string()),
                timestamp: Some(timestamp.trim().to_string()),
                text: text.to_string(),
            });
    }

    if let Some((sender, timestamp)) = split_sender_time(first) {
        let text = lines[1..].join("\n").trim().to_string();
        return (looks_like_chat_sender(sender) && !text.is_empty() && !is_chat_chrome_text(&text))
            .then(|| ParsedChatMessage {
                sender: Some(sender.to_string()),
                timestamp: Some(timestamp.to_string()),
                text,
            });
    }

    if lines.len() >= 3 && looks_like_time(&lines[1]) {
        let sender = first.trim();
        let text = lines[2..].join("\n").trim().to_string();
        return (looks_like_chat_sender(sender) && !text.is_empty() && !is_chat_chrome_text(&text))
            .then(|| ParsedChatMessage {
                sender: Some(sender.to_string()),
                timestamp: Some(lines[1].clone()),
                text,
            });
    }

    let timestamp = lines.last()?;
    if lines.len() >= 3 && looks_like_time(timestamp) {
        let sender = first.trim();
        let text = lines[1..lines.len() - 1].join("\n").trim().to_string();
        return (looks_like_chat_sender(sender) && !text.is_empty() && !is_chat_chrome_text(&text))
            .then(|| ParsedChatMessage {
                sender: Some(sender.to_string()),
                timestamp: Some(timestamp.clone()),
                text,
            });
    }

    None
}

fn looks_like_chat_sender(sender: &str) -> bool {
    let sender = sender.trim();
    if sender.is_empty()
        || sender.chars().count() > 120
        || sender.contains('\n')
        || sender.contains('\r')
        || looks_like_time(sender)
        || is_chat_chrome_text(sender)
    {
        return false;
    }

    let lower = sender.to_ascii_lowercase();
    !matches!(
        lower.as_str(),
        "google meet" | "microsoft teams" | "zoom" | "slack" | "webex"
    ) && !lower.starts_with("recording ")
        && !lower.starts_with("meeting started")
        && !lower.starts_with("meeting ended")
}

fn parse_zoom_chat_message(raw_text: &str) -> Option<ParsedChatMessage> {
    let lines = chat_lines(raw_text);
    let first = lines.first()?.as_str();

    if lines.len() == 1 {
        let (sender, message_and_time) = first.split_once(", ")?;
        let (text, timestamp) = message_and_time.rsplit_once(", ")?;
        if looks_like_time(timestamp) {
            let text = text.trim();
            return (!sender.trim().is_empty() && !text.is_empty()).then(|| ParsedChatMessage {
                sender: non_empty_string(sender),
                timestamp: Some(timestamp.trim().to_string()),
                text: text.to_string(),
            });
        }
    }

    if !first.starts_with("From ") {
        return None;
    }

    let mut sender = first.trim_start_matches("From ").trim();
    if let Some((name, _target)) = sender.split_once(" to ") {
        sender = name.trim();
    }

    let mut timestamp = None;
    let mut message_start = 1;
    if let Some(line) = lines.get(1) {
        if looks_like_time(line) {
            timestamp = Some(line.clone());
            message_start = 2;
        }
    }

    let text = lines[message_start..].join("\n").trim().to_string();
    (!text.is_empty()).then(|| ParsedChatMessage {
        sender: non_empty_string(sender),
        timestamp,
        text,
    })
}

fn parse_slack_chat_message(raw_text: &str) -> Option<ParsedChatMessage> {
    let lines = chat_lines(raw_text);
    if lines.len() == 1 {
        return parse_slack_accessibility_description(&lines[0]);
    }

    if lines.len() < 2 {
        return None;
    }

    let first_line = lines[0].as_str();
    let (sender, timestamp, message_start) =
        if let Some((name, time)) = split_sender_time(first_line) {
            (name, time.to_string(), 1)
        } else if looks_like_time(&lines[1]) {
            (first_line, lines[1].clone(), 2)
        } else {
            return None;
        };

    let text = lines[message_start..].join("\n").trim().to_string();
    (!text.is_empty() && !is_chat_chrome_text(&text)).then(|| ParsedChatMessage {
        sender: non_empty_string(sender),
        timestamp: Some(timestamp),
        text,
    })
}

fn parse_slack_accessibility_description(line: &str) -> Option<ParsedChatMessage> {
    let line = line.trim().trim_end_matches('.');
    let (sender, message_and_time) = line.split_once(": ")?;

    for (separator, _) in message_and_time.rmatch_indices(". ") {
        let text = message_and_time[..separator].trim();
        let timestamp = message_and_time[separator + 2..].trim();
        let Some((date, time)) = timestamp.rsplit_once(" at ") else {
            continue;
        };

        if !sender.trim().is_empty()
            && !text.is_empty()
            && !date.trim().is_empty()
            && looks_like_time(time)
            && !is_chat_chrome_text(text)
        {
            return Some(ParsedChatMessage {
                sender: non_empty_string(sender),
                timestamp: Some(time.trim().to_string()),
                text: text.to_string(),
            });
        }
    }

    None
}

fn chat_lines(text: &str) -> Vec<String> {
    normalize_chat_text(text)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn normalize_chat_text(text: &str) -> String {
    text.replace(['\u{00a0}', '\u{202f}'], " ")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_chat_chrome_text(text: &str) -> bool {
    let lower = text.to_lowercase();
    matches!(
        lower.as_str(),
        "chat"
            | "meeting chat"
            | "send"
            | "send message"
            | "send a message"
            | "message everyone"
            | "type a message"
            | "conversation"
            | "message list"
            | "new messages"
    ) || lower.starts_with("type a message")
        || lower.starts_with("message everyone")
        || lower.starts_with("send a message")
}

fn split_sender_time(text: &str) -> Option<(&str, &str)> {
    let trimmed = text.trim();
    for suffix in [" AM", " PM", " am", " pm"] {
        if let Some(without_period) = trimmed.strip_suffix(suffix) {
            let (name, clock) = without_period.rsplit_once(' ')?;
            let time_start = trimmed.len() - clock.len() - suffix.len();
            let time = &trimmed[time_start..];
            return looks_like_time(time).then_some((name.trim(), time.trim()));
        }
    }

    let (name, time) = trimmed.rsplit_once(' ')?;
    looks_like_time(time).then_some((name.trim(), time.trim()))
}

pub(super) fn looks_like_time(text: &str) -> bool {
    let compact = text.trim().to_lowercase();
    let meridiem = compact
        .strip_suffix(" am")
        .or_else(|| compact.strip_suffix(" pm"));
    let time = meridiem.unwrap_or(&compact);
    let Some((hour, minute)) = time.split_once(':') else {
        return false;
    };

    let Ok(hour) = hour.parse::<u8>() else {
        return false;
    };
    let Ok(minute) = minute.parse::<u8>() else {
        return false;
    };

    minute < 60
        && if meridiem.is_some() {
            (1..=12).contains(&hour)
        } else {
            hour < 24
        }
}

fn non_empty_string(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

pub(super) fn extract_links(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter_map(|part| {
            let link = part.trim_matches(|c: char| {
                matches!(
                    c,
                    '"' | '\'' | '(' | ')' | '[' | ']' | '<' | '>' | ',' | '.'
                )
            });
            (link.starts_with("http://") || link.starts_with("https://")).then(|| link.to_string())
        })
        .collect()
}

pub(super) fn candidate_chat_target(node: &AxNode) -> Option<MeetingChatTarget> {
    let role = node.role.as_deref().unwrap_or_default();
    let text = node.text.as_str();
    let mut confidence = 0.0;
    let mut signals = Vec::new();
    let mut kind = "unknown";

    let is_button = role == "AXButton" || role == "AXMenuItem";
    let is_send_button = text.contains("send") && is_button;
    let is_text_input = role == "AXTextArea" || role == "AXTextField";
    let has_chat_input_label = text.contains("send a message")
        || text.contains("message everyone")
        || text.contains("message to ")
        || text.contains("type a message")
        || text.contains("chat");
    let is_chat_control = is_button
        && !is_send_button
        && (text == "axbutton chat"
            || text == "axmenuitem chat"
            || text.contains("meeting chat")
            || text.contains("open chat")
            || text.contains("show chat")
            || text.contains("show/hide thread")
            || text.contains(" chat"));

    if is_text_input {
        confidence += 0.25;
        signals.push("text-input-role".to_string());
        kind = "input";
    }
    if has_chat_input_label {
        confidence += 0.4;
        signals.push("chat-label".to_string());
    }
    if is_chat_control {
        confidence += 0.45;
        signals.push("open-chat-control".to_string());
        kind = "openChatControl";
    }
    if is_send_button {
        confidence += 0.35;
        signals.push("send-button".to_string());
        kind = "sendButton";
    }
    if text.contains("conversation") || text.contains("message list") {
        confidence += 0.25;
        signals.push("message-list-label".to_string());
        kind = "messageList";
    }
    if node.settable_value {
        confidence += 0.2;
        signals.push("settable-value".to_string());
        kind = "input";
    }

    if kind == "input" && (!is_text_input || !node.settable_value || !has_chat_input_label) {
        return None;
    }

    if confidence < 0.35 {
        return None;
    }

    Some(MeetingChatTarget {
        kind: kind.to_string(),
        #[cfg(test)]
        settable: node.settable_value,
        confidence,
        #[cfg(test)]
        signals,
    })
}
