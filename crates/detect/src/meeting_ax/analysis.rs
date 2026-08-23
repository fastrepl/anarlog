use std::collections::HashMap;

use super::{
    AxNode, MeetingCapturedChatMessage, MeetingChatDirection, MeetingChatTarget, MeetingPlatform,
    MeetingSurface, node_labels, path_is_ancestor, validated_chat_capture_scope,
};

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

    if matches!(role, "AXGroup" | "AXCell" | "AXTabGroup")
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
            is_video_render || lower == "video tile" || zoom_audio_state_label_has_name(label)
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
fn zoom_audio_state_label_has_name(label: &str) -> bool {
    let lower = label.to_ascii_lowercase();
    [", computer audio", ", no audio connected"]
        .into_iter()
        .find_map(|marker| lower.find(marker).map(|index| label[..index].trim()))
        .is_some_and(is_plausible_zoom_scope_name)
}

fn is_plausible_zoom_scope_name(name: &str) -> bool {
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

pub(super) fn is_zoom_meeting_scope_node(node: &AxNode) -> bool {
    if node.role.as_deref() != Some("AXWindow") {
        return false;
    }

    let title = node.title.as_deref().unwrap_or_default().to_lowercase();
    title.contains("zoom meeting") || title.trim() == "meeting"
}

pub(super) fn is_zoom_chat_scope_node(node: &AxNode) -> bool {
    if node.identifier.as_deref() == Some("ZMTextMessageCellView") {
        return true;
    }

    matches!(node.role.as_deref(), Some("AXTable") | Some("AXList"))
        && node_labels(node).any(|label| {
            matches!(
                label.trim().to_ascii_lowercase().as_str(),
                "chat list" | "chat history"
            )
        })
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
    .map(str::trim)
    .filter(|value| !value.is_empty())
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
        || label.contains("type message here")
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
        let Some((scope_path, _)) = validated_chat_capture_scope(platform, nodes) else {
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

    if *platform == MeetingPlatform::GoogleMeet
        && let Some(scope_path) = generic_scope_path.as_deref()
    {
        parsed_nodes.extend(extract_google_meet_structured_messages(nodes, scope_path));
        parsed_nodes.extend(extract_google_meet_timestamp_sibling_messages(
            nodes, scope_path,
        ));
    }
    if *platform == MeetingPlatform::Webex
        && let Some(scope_path) = generic_scope_path.as_deref()
    {
        parsed_nodes.extend(extract_webex_structured_messages(nodes, scope_path));
    }
    if *platform == MeetingPlatform::Zoom && *surface == MeetingSurface::Native {
        parsed_nodes.extend(extract_zoom_title_description_messages(nodes));
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
            direction: parsed
                .direction
                .clone()
                .or_else(|| meeting_chat_direction(platform, parsed.sender.as_deref())),
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

fn extract_zoom_title_description_messages(nodes: &[AxNode]) -> Vec<(&AxNode, ParsedChatMessage)> {
    let chat_history_paths = nodes
        .iter()
        .filter(|node| node.within_zoom_meeting_scope && is_zoom_chat_scope_node(node))
        .map(|node| node.tree_path.clone())
        .collect::<Vec<_>>();
    let [chat_history_path] = chat_history_paths.as_slice() else {
        return Vec::new();
    };

    let mut messages = nodes
        .iter()
        .filter(|node| {
            node.within_zoom_meeting_scope
                && path_is_ancestor(chat_history_path, &node.tree_path)
                && matches!(
                    node.role.as_deref(),
                    Some("AXGroup") | Some("AXCell") | Some("AXRow")
                )
        })
        .filter_map(|node| {
            let (sender, timestamp) = split_sender_time(node.title.as_deref()?)?;
            let text = normalize_chat_text(node.description.as_deref()?);
            (looks_like_chat_sender(sender) && !text.is_empty() && !is_chat_chrome_text(&text))
                .then(|| {
                    (
                        node,
                        ParsedChatMessage {
                            sender: Some(sender.to_string()),
                            timestamp: Some(timestamp.to_string()),
                            direction: None,
                            text,
                        },
                    )
                })
        })
        .collect::<Vec<_>>();
    let message_paths = messages
        .iter()
        .map(|(node, _)| node.tree_path.clone())
        .collect::<Vec<_>>();
    messages.retain(|(node, _)| {
        !message_paths
            .iter()
            .any(|path| path_is_ancestor(&node.tree_path, path))
    });
    messages
}

fn extract_google_meet_structured_messages<'a>(
    nodes: &'a [AxNode],
    scope_path: &[usize],
) -> Vec<(&'a AxNode, ParsedChatMessage)> {
    let mut branches = Vec::<Vec<usize>>::new();
    for node in nodes.iter().filter(|node| {
        node.tree_path.starts_with(scope_path) && node.tree_path.len() > scope_path.len()
    }) {
        let branch = node.tree_path[..scope_path.len() + 1].to_vec();
        if !branches.contains(&branch) {
            branches.push(branch);
        }
    }
    branches.sort();

    let mut messages = Vec::new();
    for branch in branches {
        let branch_nodes = nodes
            .iter()
            .filter(|node| node.tree_path.starts_with(&branch))
            .collect::<Vec<_>>();
        let mut senders = Vec::<(&AxNode, String, Option<String>)>::new();
        for node in branch_nodes.iter().filter(|node| {
            node.tree_path.len() <= branch.len() + 3
                && matches!(node.role.as_deref(), Some("AXStaticText") | Some("AXText"))
        }) {
            let Some(label) = chat_message_text(node) else {
                continue;
            };
            let (sender, timestamp) = split_sender_time(&label)
                .map(|(sender, time)| (sender, Some(time.to_string())))
                .unwrap_or((label.as_str(), None));
            if !looks_like_chat_sender(sender)
                || senders.iter().any(|(_, existing_sender, existing_time)| {
                    existing_sender == sender && existing_time == &timestamp
                })
            {
                continue;
            }
            senders.push((node, sender.to_string(), timestamp));
        }
        let [(sender_node, sender, timestamp)] = senders.as_slice() else {
            continue;
        };

        let mut fragment_groups = Vec::<(Vec<usize>, Vec<(&AxNode, String)>)>::new();
        for node in branch_nodes.into_iter().filter(|node| {
            node.index > sender_node.index
                && node.tree_path.len() > branch.len()
                && matches!(
                    node.role.as_deref(),
                    Some("AXStaticText") | Some("AXText") | Some("AXLink")
                )
        }) {
            let Some(fragment) = google_meet_chat_fragment(node) else {
                continue;
            };
            if fragment == *sender || looks_like_time(&fragment) || is_chat_chrome_text(&fragment) {
                continue;
            }
            let group_path = node.tree_path[..branch.len() + 1].to_vec();
            if let Some((_, fragments)) = fragment_groups
                .iter_mut()
                .find(|(path, _)| path == &group_path)
            {
                if !fragments.iter().any(|(_, existing)| existing == &fragment) {
                    fragments.push((node, fragment));
                }
            } else {
                fragment_groups.push((group_path, vec![(node, fragment)]));
            }
        }

        for (_, fragments) in fragment_groups {
            let Some(source) = fragments.first().map(|(node, _)| *node) else {
                continue;
            };
            let text = fragments
                .into_iter()
                .map(|(_, fragment)| fragment)
                .collect::<Vec<_>>()
                .join(" ");
            if !text.is_empty() {
                messages.push((
                    source,
                    ParsedChatMessage {
                        sender: Some(sender.clone()),
                        timestamp: timestamp.clone(),
                        direction: None,
                        text,
                    },
                ));
            }
        }
    }
    messages
}

fn google_meet_chat_fragment(node: &AxNode) -> Option<String> {
    if node.role.as_deref() == Some("AXLink") {
        return [
            node.title.as_deref(),
            node.description.as_deref(),
            node.value.as_deref(),
        ]
        .into_iter()
        .flatten()
        .map(normalize_chat_text)
        .find(|value| value.starts_with("http://") || value.starts_with("https://"));
    }

    let fragment = chat_message_text(node)?;
    (!matches!(
        fragment.to_ascii_lowercase().as_str(),
        "hover over a message to pin it" | "pin message"
    ))
    .then_some(fragment)
}

fn extract_google_meet_timestamp_sibling_messages<'a>(
    nodes: &'a [AxNode],
    scope_path: &[usize],
) -> Vec<(&'a AxNode, ParsedChatMessage)> {
    nodes
        .iter()
        .filter_map(|timestamp_node| {
            if !timestamp_node.tree_path.starts_with(scope_path)
                || timestamp_node.tree_path.len() < scope_path.len() + 3
                || !matches!(
                    timestamp_node.role.as_deref(),
                    Some("AXStaticText") | Some("AXText")
                )
            {
                return None;
            }

            let timestamp = chat_message_text(timestamp_node)?;
            if !looks_like_time(&timestamp) {
                return None;
            }

            let leaf_index = timestamp_node.tree_path.len() - 1;
            if timestamp_node.tree_path[leaf_index] != 0 {
                return None;
            }
            let slot_index = leaf_index - 1;
            let slot = timestamp_node.tree_path[slot_index];
            let parent_path = &timestamp_node.tree_path[..slot_index];

            let mut content_path = parent_path.to_vec();
            content_path.push(slot.checked_add(1)?);
            let fragments = nodes
                .iter()
                .filter(|node| {
                    node.tree_path.starts_with(&content_path)
                        && matches!(
                            node.role.as_deref(),
                            Some("AXStaticText") | Some("AXText") | Some("AXLink")
                        )
                })
                .filter_map(|node| google_meet_chat_fragment(node).map(|fragment| (node, fragment)))
                .fold(
                    Vec::<(&AxNode, String)>::new(),
                    |mut fragments, fragment| {
                        if !fragments
                            .iter()
                            .any(|(_, existing)| existing == &fragment.1)
                        {
                            fragments.push(fragment);
                        }
                        fragments
                    },
                );
            let source = fragments.first()?.0;
            let text = fragments
                .into_iter()
                .map(|(_, fragment)| fragment)
                .collect::<Vec<_>>()
                .join(" ");
            if text.is_empty() {
                return None;
            }

            let sender = slot.checked_sub(1).and_then(|sender_slot| {
                let sender_is_previous_message_content =
                    sender_slot.checked_sub(1).is_some_and(|timestamp_slot| {
                        let mut timestamp_path = parent_path.to_vec();
                        timestamp_path.push(timestamp_slot);
                        nodes.iter().any(|node| {
                            node.tree_path.starts_with(&timestamp_path)
                                && matches!(
                                    node.role.as_deref(),
                                    Some("AXStaticText") | Some("AXText")
                                )
                                && chat_message_text(node)
                                    .is_some_and(|text| looks_like_time(&text))
                        })
                    });
                if sender_is_previous_message_content {
                    return None;
                }

                let mut sender_path = parent_path.to_vec();
                sender_path.push(sender_slot);
                let mut labels = nodes
                    .iter()
                    .filter(|node| {
                        node.tree_path.starts_with(&sender_path)
                            && matches!(node.role.as_deref(), Some("AXStaticText") | Some("AXText"))
                    })
                    .filter_map(chat_message_text)
                    .filter(|label| looks_like_chat_sender(label));
                let sender = labels.next()?;
                labels.next().is_none().then_some(sender)
            });

            Some((
                source,
                ParsedChatMessage {
                    sender,
                    timestamp: Some(timestamp),
                    direction: None,
                    text,
                },
            ))
        })
        .collect()
}

fn extract_webex_structured_messages<'a>(
    nodes: &'a [AxNode],
    scope_path: &[usize],
) -> Vec<(&'a AxNode, ParsedChatMessage)> {
    nodes
        .iter()
        .filter(|node| {
            (matches!(node.role.as_deref(), Some("AXRow") | Some("AXCell"))
                || (node.role.as_deref() == Some("AXGroup")
                    && node_labels(node).any(|label| {
                        let label = label.to_ascii_lowercase();
                        label.contains(", sent by ")
                            && label.contains("press enter key to enter the group")
                    })))
                && node.tree_path.starts_with(scope_path)
        })
        .filter_map(|row| {
            let descendants = nodes
                .iter()
                .filter(|node| path_is_ancestor(&row.tree_path, &node.tree_path))
                .collect::<Vec<_>>();
            let mut fragments = descendants
                .iter()
                .filter(|node| {
                    let is_message_text = node
                        .title
                        .as_deref()
                        .is_some_and(|title| title.trim().eq_ignore_ascii_case("message text"));
                    (node.role.as_deref() == Some("AXTextArea")
                        && (!node.settable_value || is_message_text))
                        || (node.role.as_deref() == Some("AXStaticText") && is_message_text)
                })
                .filter_map(|node| {
                    let text = [
                        node.value.as_deref(),
                        node.title.as_deref(),
                        node.description.as_deref(),
                    ]
                    .into_iter()
                    .flatten()
                    .map(normalize_chat_text)
                    .find(|text| !text.is_empty() && !is_chat_chrome_text(text))?;
                    Some((*node, text))
                });
            let (source, first) = fragments.next()?;
            let text = std::iter::once(first)
                .chain(fragments.map(|(_, fragment)| fragment))
                .collect::<Vec<_>>()
                .join("\n");
            let descendant_metadata = descendants
                .iter()
                .flat_map(|node| node_labels(node))
                .filter_map(|label| {
                    let normalized = normalize_chat_text(label);
                    let (sender, timestamp) = split_sender_time(&normalized)?;
                    looks_like_chat_sender(sender)
                        .then_some((sender.to_string(), timestamp.to_string()))
                })
                .collect::<Vec<_>>();
            let (sender, timestamp) = if let [(sender, timestamp)] = descendant_metadata.as_slice()
            {
                (sender.clone(), timestamp.clone())
            } else {
                node_labels(row).find_map(|label| {
                    let normalized = normalize_chat_text(label);
                    let metadata = normalized.strip_prefix(&text)?.trim_start_matches(", ");
                    let parts = metadata.split(", ").map(str::trim).collect::<Vec<_>>();
                    parts.windows(2).find_map(|parts| {
                        let sender = parts[0].strip_prefix("sent by ").unwrap_or(parts[0]);
                        (looks_like_chat_sender(sender) && looks_like_time(parts[1]))
                            .then(|| (sender.to_string(), parts[1].to_string()))
                    })
                })?
            };

            Some((
                source,
                ParsedChatMessage {
                    sender: Some(sender.clone()),
                    timestamp: Some(timestamp),
                    direction: meeting_chat_direction(
                        &MeetingPlatform::Webex,
                        Some(sender.as_str()),
                    ),
                    text,
                },
            ))
        })
        .collect()
}

pub(super) fn meeting_chat_direction(
    platform: &MeetingPlatform,
    sender: Option<&str>,
) -> Option<MeetingChatDirection> {
    if !matches!(platform, MeetingPlatform::Zoom | MeetingPlatform::Webex) {
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
    pub(super) direction: Option<MeetingChatDirection>,
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

    let value = [
        node.value.as_deref(),
        node.title.as_deref(),
        node.description.as_deref(),
    ]
    .into_iter()
    .flatten()
    .find(|value| !value.trim().is_empty())?;
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
        MeetingPlatform::MicrosoftTeams => parse_teams_accessibility_description(raw_text)
            .or_else(|| parse_web_chat_message(raw_text)),
        MeetingPlatform::GoogleMeet => parse_web_chat_message(raw_text),
        MeetingPlatform::Webex => {
            parse_webex_browser_message(raw_text).or_else(|| parse_web_chat_message(raw_text))
        }
        MeetingPlatform::Discord | MeetingPlatform::Unknown => None,
    }
}

fn parse_webex_browser_message(raw_text: &str) -> Option<ParsedChatMessage> {
    let normalized = normalize_chat_text(raw_text);
    let rest = normalized.strip_prefix("Message from ")?;
    let parts = rest.split(", ").map(str::trim).collect::<Vec<_>>();
    let time_index = parts.iter().position(|part| looks_like_time(part))?;
    let sender = parts.first().copied()?;
    let text = parts.get(time_index + 1..)?.join(", ").trim().to_string();

    (looks_like_chat_sender(sender) && !text.is_empty() && !is_chat_chrome_text(&text)).then(|| {
        ParsedChatMessage {
            sender: Some(sender.to_string()),
            timestamp: Some(parts[time_index].to_string()),
            direction: meeting_chat_direction(&MeetingPlatform::Webex, Some(sender)),
            text,
        }
    })
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
                direction: None,
                text: text.to_string(),
            });
    }

    if let Some((sender, timestamp)) = split_sender_time(first) {
        let text = lines[1..].join("\n").trim().to_string();
        return (looks_like_chat_sender(sender) && !text.is_empty() && !is_chat_chrome_text(&text))
            .then(|| ParsedChatMessage {
                sender: Some(sender.to_string()),
                timestamp: Some(timestamp.to_string()),
                direction: None,
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
                direction: None,
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
                direction: None,
                text,
            });
    }

    None
}

fn parse_teams_accessibility_description(raw_text: &str) -> Option<ParsedChatMessage> {
    let line = normalize_chat_text(raw_text);
    if line.contains('\n') {
        return None;
    }

    let line = line.trim().trim_end_matches('.');
    let (sender_and_text, timestamp) = line
        .rsplit_once(" Today at ")
        .filter(|(_, timestamp)| looks_like_time(timestamp))
        .or_else(|| split_sender_time(line))?;
    if !looks_like_time(timestamp) {
        return None;
    }
    let (sender, text) = sender_and_text.split_once(" Sent ")?;
    let text = text
        .replace(" Link https://", " https://")
        .replace(" Link http://", " http://");

    (looks_like_chat_sender(sender) && !text.is_empty() && !is_chat_chrome_text(&text)).then(|| {
        ParsedChatMessage {
            sender: Some(sender.trim().to_string()),
            timestamp: Some(timestamp.trim().to_string()),
            direction: None,
            text,
        }
    })
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
        if let Some((timestamp, text)) = message_and_time.split_once(", ")
            && looks_like_time(timestamp)
        {
            let sender = sender
                .split_once(" to ")
                .map_or(sender, |(sender, _target)| sender)
                .trim();
            let text = text.trim();
            return (looks_like_chat_sender(sender)
                && !text.is_empty()
                && !is_chat_chrome_text(text))
            .then(|| ParsedChatMessage {
                sender: Some(sender.to_string()),
                timestamp: Some(timestamp.trim().to_string()),
                direction: None,
                text: text.to_string(),
            });
        }
        let (text, timestamp) = message_and_time.rsplit_once(", ")?;
        if looks_like_time(timestamp) {
            let text = text.trim();
            return (!sender.trim().is_empty() && !text.is_empty()).then(|| ParsedChatMessage {
                sender: non_empty_string(sender),
                timestamp: Some(timestamp.trim().to_string()),
                direction: None,
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
        direction: None,
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
        direction: None,
        text,
    })
}

fn parse_slack_accessibility_description(line: &str) -> Option<ParsedChatMessage> {
    let line = line.trim().trim_end_matches('.');
    let (sender, message_and_time) = line.split_once(": ")?;

    for (separator, _) in message_and_time.rmatch_indices(". ") {
        let text = message_and_time[..separator].trim();
        let timestamp = message_and_time[separator + 2..].trim();
        let time = timestamp
            .rsplit_once(" at ")
            .filter(|(date, _)| !date.trim().is_empty())
            .map(|(_, time)| time)
            .or_else(|| {
                let mut parts = timestamp.split(". ");
                let time = parts.next()?.trim();
                (looks_like_time(time) && parts.all(is_slack_accessibility_metadata))
                    .then_some(time)
            });

        if !sender.trim().is_empty()
            && !text.is_empty()
            && time.is_some_and(looks_like_time)
            && !is_chat_chrome_text(text)
        {
            return Some(ParsedChatMessage {
                sender: non_empty_string(sender),
                timestamp: time.map(|time| time.trim().to_string()),
                direction: None,
                text: text.to_string(),
            });
        }
    }

    None
}

fn is_slack_accessibility_metadata(text: &str) -> bool {
    let lower = text.trim().to_ascii_lowercase();
    if lower == "edited" {
        return true;
    }

    let Some((count, kind)) = lower.split_once(' ') else {
        return false;
    };
    count.parse::<usize>().is_ok()
        && matches!(
            kind,
            "link"
                | "links"
                | "reply"
                | "replies"
                | "reaction"
                | "reactions"
                | "file"
                | "files"
                | "attachment"
                | "attachments"
        )
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
            | "type message here ..."
            | "conversation"
            | "message list"
            | "chat message list"
            | "new messages"
    ) || lower.starts_with("type a message")
        || lower.starts_with("type message here")
        || lower.starts_with("message everyone")
        || lower.starts_with("send a message")
        || lower.starts_with("continuous chat is turned off")
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
    let parts = time.split(':').collect::<Vec<_>>();
    if !(2..=3).contains(&parts.len()) {
        return false;
    }

    let Ok(hour) = parts[0].parse::<u8>() else {
        return false;
    };
    let Ok(minute) = parts[1].parse::<u8>() else {
        return false;
    };
    let second_is_valid = parts
        .get(2)
        .is_none_or(|second| second.parse::<u8>().is_ok_and(|second| second < 60));

    minute < 60
        && second_is_valid
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
        || text.contains("type message here")
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
