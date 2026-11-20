use super::*;

const OPERATION: &str = "op-1";
const NOW: u64 = 1_700_000_000_000;

fn request(text: &str) -> RefineRequest {
    RefineRequest {
        operation_id: OPERATION.into(),
        now_ms: NOW,
        text: text.into(),
        selection: None,
        policy: Some(TextPolicy {
            cleanup: true,
            voice_edit: true,
        }),
        model_ready: true,
        terms: vec![],
        replacements: vec![],
        instructions: None,
        before: None,
        context: None,
    }
}

fn ask(request: RefineRequest) -> (ModelCall, PendingRefinement) {
    match prepare_refinement(request) {
        RefinePlan::Ask { call, pending } => (call, pending),
        RefinePlan::Done { outcome } => panic!("expected a model call, got {outcome:?}"),
    }
}

fn done(request: RefineRequest) -> RefineOutcome {
    match prepare_refinement(request) {
        RefinePlan::Done { outcome } => outcome,
        RefinePlan::Ask { .. } => panic!("expected no model call"),
    }
}

/// The outcome of a cleanup whose model replied `reply`.
fn cleaned(request: RefineRequest, reply: &str) -> RefineOutcome {
    let (_, pending) = ask(request);
    resolve_refinement(pending, ModelReply::Text { text: reply.into() })
}

fn kept(text: &str) -> RefineOutcome {
    RefineOutcome::Cleaned { text: text.into() }
}

fn raw(text: &str, reason: RefineReason) -> RefineOutcome {
    RefineOutcome::Raw {
        text: text.into(),
        reason,
    }
}

fn snapshot(operation_id: &str, captured_at_ms: u64) -> ContextSnapshot {
    ContextSnapshot {
        operation_id: operation_id.into(),
        captured_at_ms,
        before: Some("We can ship it tomorrow.".into()),
        screen: Some("Slack — #launch\n\nPriya: Is Supabase still down?".into()),
        terms: vec!["Supabase".into()],
    }
}

#[test]
fn makes_no_model_call_unless_the_person_chose_it_and_never_types_an_instruction_over_a_selection()
{
    let replacements = vec![Replacement {
        from: "work mail".into(),
        to: "john@fastrepl.com".into(),
    }];
    let off = RefineRequest {
        policy: Some(TextPolicy::default()),
        replacements: replacements.clone(),
        ..request("send it to work mail")
    };
    assert_eq!(
        done(off.clone()),
        raw("send it to john@fastrepl.com", RefineReason::CleanupOff)
    );
    assert_eq!(
        done(RefineRequest {
            policy: None,
            ..off.clone()
        }),
        raw(
            "send it to john@fastrepl.com",
            RefineReason::PolicyUnreadable
        )
    );
    assert_eq!(
        done(RefineRequest {
            model_ready: false,
            policy: request("").policy,
            ..off.clone()
        }),
        raw("send it to john@fastrepl.com", RefineReason::NoModel)
    );
    let edit = RefineRequest {
        selection: Some("A long line.".into()),
        ..request("make it shorter")
    };
    for (changed, reason) in [
        (
            RefineRequest {
                policy: None,
                ..edit.clone()
            },
            RefineReason::PolicyUnreadable,
        ),
        (
            RefineRequest {
                policy: Some(TextPolicy {
                    cleanup: true,
                    voice_edit: false,
                }),
                ..edit.clone()
            },
            RefineReason::VoiceEditOff,
        ),
        (
            RefineRequest {
                model_ready: false,
                ..edit.clone()
            },
            RefineReason::NoModel,
        ),
        (
            RefineRequest {
                selection: Some("x".repeat(MAX_SELECTION_LENGTH + 1)),
                ..edit.clone()
            },
            RefineReason::TooLong,
        ),
    ] {
        assert_eq!(done(changed), RefineOutcome::EditFailed { reason });
    }
    assert_eq!(done(request("   ")), RefineOutcome::Empty);
}

#[test]
fn types_the_raw_words_when_cleanup_times_out_fails_or_misbehaves_but_nothing_when_cancelled() {
    let replacements = vec![Replacement {
        from: "my website".into(),
        to: "https://johnjeong.com".into(),
    }];
    let (call, pending) = ask(RefineRequest {
        replacements,
        ..request("um check my website")
    });
    assert_eq!(call.deadline_ms, CLEANUP_DEADLINE_MS);
    let resolve = |reply| resolve_refinement(pending.clone(), reply);
    let fallback = |reason| raw("um check https://johnjeong.com", reason);
    assert_eq!(
        resolve(ModelReply::TimedOut),
        fallback(RefineReason::TimedOut)
    );
    assert_eq!(resolve(ModelReply::Failed), fallback(RefineReason::Failed));
    assert_eq!(resolve(ModelReply::Cancelled), RefineOutcome::Cancelled);
    assert_eq!(
        resolve(ModelReply::Text {
            text: "Check my website.".into()
        }),
        kept("Check https://johnjeong.com.")
    );
    for (reply, reason) in [
        ("   ", RefineReason::EmptyReply),
        ("<think>the user wants", RefineReason::Malformed),
        (
            "<instruction>check my website</instruction>",
            RefineReason::Malformed,
        ),
    ] {
        assert_eq!(
            resolve(ModelReply::Text { text: reply.into() }),
            fallback(reason)
        );
    }
    assert_eq!(
        resolve(ModelReply::Text {
            text: "<think>clean it</think>Check my website.".into()
        }),
        kept("Check https://johnjeong.com.")
    );
}

#[test]
fn keeps_an_edit_only_from_a_result_and_never_falls_back_to_the_spoken_words() {
    let (call, pending) = ask(RefineRequest {
        selection: Some("  Long sentence here. ".into()),
        ..request("shorten it")
    });
    assert_eq!(call.deadline_ms, EDIT_DEADLINE_MS);
    assert!(
        call.prompt
            .starts_with("<selection>\n  Long sentence here. \n</selection>")
    );
    let resolve =
        |text: &str| resolve_refinement(pending.clone(), ModelReply::Text { text: text.into() });
    assert_eq!(
        resolve("<result>Short.</result>"),
        RefineOutcome::Edited {
            text: "  Short. ".into()
        }
    );
    assert_eq!(
        resolve("Short."),
        RefineOutcome::EditFailed {
            reason: RefineReason::Malformed
        }
    );
    assert_eq!(
        resolve("<result> </result>"),
        RefineOutcome::EditFailed {
            reason: RefineReason::EmptyReply
        }
    );
    assert_eq!(
        resolve_refinement(pending.clone(), ModelReply::TimedOut),
        RefineOutcome::EditFailed {
            reason: RefineReason::TimedOut
        }
    );
    assert_eq!(
        resolve_refinement(pending, ModelReply::Cancelled),
        RefineOutcome::Cancelled
    );
}

#[test]
fn rejects_replies_that_answer_summarize_loop_or_repeat_the_text_before_the_caret() {
    let ramble = "um so like the thing is, the the build is, like, really slow, it's slow because, um, the cache, the cache is cold every single time, you know";
    let fluent = "The build is really slow because the cache is cold every single time.";
    assert_eq!(cleaned(request(ramble), fluent), kept(fluent));
    assert_eq!(
        cleaned(request(ramble), "The build is slow."),
        raw(ramble, RefineReason::Dropped)
    );

    let question = "what time does the pharmacy close";
    let answer = "Most pharmacies close around 9 PM on weekdays, but hours vary by location, so check the store's page before you go.";
    assert_eq!(
        cleaned(request(question), answer),
        raw(question, RefineReason::Answered)
    );

    let said = "we should send the report to Mina before the meeting tomorrow morning okay";
    assert_eq!(
        cleaned(
            request(said),
            "Send the report, send the report, send the report."
        ),
        raw(said, RefineReason::Repetition)
    );
    // Repetition the speaker said is theirs to keep.
    let cheer = "go team go team go team let's win this one tonight";
    assert_eq!(
        cleaned(
            request(cheer),
            "Go team, go team, go team! Let's win this one tonight."
        ),
        kept("Go team, go team, go team! Let's win this one tonight.")
    );

    let before = Some("Thanks for sending the quarterly numbers over.".to_owned());
    let continued = RefineRequest {
        before: before.clone(),
        ..request("i'll review them tonight")
    };
    assert_eq!(
        cleaned(
            continued.clone(),
            "Thanks for sending the quarterly numbers over. I'll review them tonight."
        ),
        raw("i'll review them tonight", RefineReason::RepeatsBefore)
    );
    assert_eq!(
        cleaned(continued, "I'll review them tonight."),
        kept("I'll review them tonight.")
    );
}

#[test]
fn allows_shorter_replies_for_marked_corrections_written_forms_and_spellings() {
    let raw_text = "let's meet on Tuesday, actually no, Wednesday works better for me";
    assert_eq!(
        cleaned(request(raw_text), "<corrected/>Let's meet on Wednesday."),
        kept("Let's meet on Wednesday.")
    );
    assert_eq!(
        cleaned(request(raw_text), "Let's meet on Wednesday."),
        raw(raw_text, RefineReason::Dropped)
    );
    assert_eq!(
        cleaned(request(raw_text), "<corrected/>Wednesday."),
        raw(raw_text, RefineReason::Dropped)
    );

    assert_eq!(cleaned(request("forty two dollars"), "$42"), kept("$42"));
    assert_eq!(
        cleaned(request("ask Jon, J O N"), "Ask Jon."),
        kept("Ask Jon.")
    );
    assert_eq!(
        cleaned(
            request("mina at acme dot io slash pricing"),
            "mina@acme.io/pricing"
        ),
        kept("mina@acme.io/pricing")
    );
    assert_eq!(
        cleaned(request("사십이 달러 보내 줘"), "42달러 보내 줘."),
        kept("42달러 보내 줘.")
    );
}

#[test]
fn joins_the_sentence_before_the_caret_only_when_there_is_one() {
    let with_before = RefineRequest {
        before: Some("We'll stay in.".into()),
        ..request("if it rains, no, if it snows")
    };
    assert_eq!(
        cleaned(with_before.clone(), "<corrected/><join/>if it snows."),
        kept(", if it snows.")
    );
    assert_eq!(
        cleaned(with_before, "<join/><corrected/>if it snows."),
        kept(", if it snows.")
    );
    assert_eq!(
        cleaned(
            request("if it rains, no, if it snows"),
            "<join/><corrected/>if it snows."
        ),
        kept("if it snows.")
    );
}

#[test]
fn writes_the_prompt_in_the_spoken_language_with_the_persons_instructions_last() {
    let (call, _) = ask(RefineRequest {
        terms: vec!["Acme".into()],
        instructions: Some("Never use exclamation marks.".into()),
        context: Some(snapshot(OPERATION, NOW - 1_000)),
        ..request("음 내일 회의 몇 시인지 알려줄게")
    });
    let system = &call.system;
    let rule = system
        .find("Reply in Korean: the transcript is in Korean.")
        .unwrap();
    assert!(rule > system.find("<screen>").unwrap());
    assert!(
        rule > system
            .find("<before>\nWe can ship it tomorrow.\n</before>")
            .unwrap()
    );
    assert!(
        rule > system
            .find("Terms to spell exactly: Acme, Supabase")
            .unwrap()
    );
    assert!(rule < system.find("Never use exclamation marks.").unwrap());
    assert!(system.trim_end().ends_with("</instructions>"));
    assert_eq!(call.prompt, "음 내일 회의 몇 시인지 알려줄게");

    let mixed = ask(request("이거 deploy 하기 전에 QA 돌려 주세요"))
        .0
        .system;
    assert!(mixed.contains(
        "Reply in Korean, and keep every word the transcript has in Latin letters in Latin letters"
    ));
    let han = ask(request("明天上午十点开会")).0.system;
    assert!(han.contains("Reply in the transcript's language, Chinese or Japanese: the transcript is written in Chinese characters."));
    assert!(
        ask(request("明日の会議は十時からです"))
            .0
            .system
            .contains("Reply in Japanese")
    );
    assert!(
        !ask(request("can we move it to nine"))
            .0
            .system
            .contains("Reply in")
    );
    let edit = ask(RefineRequest {
        selection: Some("안녕하세요.".into()),
        ..request("영어로 번역해 줘")
    })
    .0;
    assert!(!edit.system.contains("Reply in Korean"));
}

#[test]
fn uses_context_only_from_its_own_fresh_operation_and_bounds_what_it_sends() {
    let used = |context| {
        ask(RefineRequest {
            context: Some(context),
            ..request("see you then")
        })
        .0
        .system
    };
    assert!(used(snapshot(OPERATION, NOW - 1_000)).contains("<screen>"));
    for stale in [
        snapshot("another-op", NOW - 1_000),
        snapshot(OPERATION, NOW - crate::context::CONTEXT_TTL_MS - 1),
        snapshot(OPERATION, NOW + 1),
    ] {
        let system = used(stale);
        assert!(
            !system.contains("<screen>")
                && !system.contains("<before>")
                && !system.contains("Supabase")
        );
    }
    let oversized = ContextSnapshot {
        before: Some(format!("{}END", "x".repeat(5_000))),
        screen: Some("y".repeat(50_000)),
        ..snapshot(OPERATION, NOW)
    };
    let system = used(oversized);
    assert!(system.contains(&format!(
        "{}END\n</before>",
        "x".repeat(MAX_BEFORE_LENGTH - 3)
    )));
    assert!(!system.contains(&"x".repeat(MAX_BEFORE_LENGTH + 1)));
    assert!(system.contains(&"y".repeat(1_500)) && !system.contains(&"y".repeat(1_703)));
    let (_, pending) = ask(RefineRequest {
        before: Some("Thanks.".into()),
        context: Some(snapshot(OPERATION, NOW)),
        ..request("bye")
    });
    assert_eq!(pending.before.as_deref(), Some("Thanks."));
}
