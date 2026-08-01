use super::*;

#[test]
fn test_slack_huddle_requires_huddle_label_and_enabled_leave_control() {
    let mut composer = node(2, "AXTextArea", "Message to test", None);
    composer.settable_value = true;
    let mut disabled_leave = node(1, "AXButton", "Leave Huddle", None);
    disabled_leave.enabled = Some(false);

    assert_eq!(slack_huddle_context(&[composer.clone()]), None);
    assert_eq!(
        slack_huddle_context(&[node(0, "AXWindow", "Huddle in test", None), disabled_leave,]),
        None
    );
    assert_eq!(
        slack_huddle_context(&[
            node(0, "AXWindow", "Huddle in  test", None),
            node(1, "AXButton", "Leave Huddle", None),
            composer,
        ]),
        Some(("Huddle in  test".to_string(), "test".to_string()))
    );
}

#[test]
fn test_slack_live_huddle_controls_fit_tree_depth_budget() {
    assert!(MAX_TREE_DEPTH >= 14);
}

#[test]
fn test_ordinary_slack_composer_is_not_a_huddle_composer() {
    let mut composer = node(2, "AXTextArea", "Message #general", None);
    composer.settable_value = true;

    assert!(!is_slack_huddle_composer(&composer, "test"));
    assert!(candidate_chat_target(&composer).is_none());
    assert_eq!(slack_huddle_context(&[composer]), None);
}

#[test]
fn test_slack_hidden_thread_control_is_recognized() {
    let control = node(3, "AXButton", "Show/hide Thread", None);

    assert!(is_slack_thread_control(&control));
    assert_eq!(
        candidate_chat_target(&control).unwrap().kind,
        "openChatControl"
    );
}

#[test]
fn test_slack_composer_and_send_button_must_share_live_thread_container() {
    let mut composer = node(4, "AXTextArea", "Message to test", None);
    composer.settable_value = true;
    let thread = [
        ancestor("Thread in test (private channel)"),
        ancestor("composer"),
    ];
    let other_thread = [ancestor("Thread in random (private channel)")];
    let duplicate_label_other_path = [ancestor_at("Thread in test (private channel)", &[9, 4])];

    assert!(is_slack_huddle_composer_in_thread(
        &composer, &thread, "test"
    ));
    assert!(!is_slack_huddle_composer_in_thread(&composer, &[], "test"));

    let mut send = node(5, "AXButton", "Send now", None);
    send.enabled = Some(false);
    assert!(!is_slack_send_now_in_thread(&send, &thread, "test", &[0]));

    send.enabled = Some(true);
    assert!(is_slack_send_now_in_thread(&send, &thread, "test", &[0]));
    assert!(!is_slack_send_now_in_thread(
        &send,
        &other_thread,
        "test",
        &[0]
    ));
    assert!(!is_slack_send_now_in_thread(
        &send,
        &duplicate_label_other_path,
        "test",
        &[0]
    ));
}

#[test]
fn test_slack_composer_selection_fails_on_ambiguity_and_drafts() {
    let mut first = node(4, "AXTextArea", "Message to test", None);
    first.settable_value = true;
    let mut second = first.clone();
    second.index = 5;

    assert_eq!(
        unique_matching_index([&first, &second].into_iter().enumerate(), |node| {
            is_slack_huddle_composer(node, "test")
        },),
        UniqueMatch::Ambiguous
    );

    first.value = Some("existing draft".to_string());
    assert!(has_nonempty_draft(&first));
    first.value = Some(" \n ".to_string());
    assert!(!has_nonempty_draft(&first));
    assert!(chat_input_is_owned("disclosure", "disclosure"));
    assert!(!chat_input_is_owned(
        "disclosure plus user text",
        "disclosure"
    ));
}
