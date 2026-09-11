use super::*;
use anlg_language::ISO639;

fn final_event(adapter: &NariAdapter, id: &str, text: &str) -> Vec<StreamResponse> {
    adapter.parse_response(&serde_json::json!({"type":"transcript.completed", "item_id":id,
        "transcript":text, "language":"en", "commit_reason":"vad", "usage":{"input_audio_seconds":1.5}}).to_string())
}
fn finalized(responses: &[StreamResponse]) -> bool {
    responses.iter().any(|response| {
        matches!(
            response,
            StreamResponse::TranscriptResponse {
                from_finalize: true,
                ..
            }
        )
    })
}
fn transcript(response: &StreamResponse) -> &str {
    match response {
        StreamResponse::TranscriptResponse { channel, .. } => &channel.alternatives[0].transcript,
        _ => panic!("not transcript"),
    }
}

#[test]
fn config_uses_documented_models_languages_and_vad() {
    let adapter = NariAdapter::default();
    let params = ListenParams {
        model: Some("qwen3-asr".into()),
        languages: vec![ISO639::Ko.into()],
        keywords: vec!["Anarlog".into()],
        ..Default::default()
    };
    let Message::Text(config) = adapter.initial_message(None, &params, 1).unwrap() else {
        panic!()
    };
    let config: serde_json::Value = serde_json::from_str(&config).unwrap();
    assert_eq!(config["session"]["model"], "qwen3-asr");
    assert_eq!(config["session"]["language"], "ko");
    assert_eq!(config["session"]["prompt"], "Anarlog");
    assert_eq!(config["session"]["turn_detection"]["type"], "server_vad");
    assert!(
        NariAdapter::language_support_live(&[ISO639::Ko.into()], Some(DEFAULT_MODEL))
            .is_supported()
    );
    assert!(!NariAdapter::language_support_live(&[ISO639::Sw.into()], None).is_supported());
    assert!(!NariAdapter::language_support_live(&[], Some("unknown")).is_supported());
    assert!(
        !super::super::AdapterKind::Nari
            .language_support_batch(&[], None)
            .is_supported()
    );
    assert_eq!(
        adapter
            .build_ws_url("https://api.narilabs.com", &params, 1)
            .as_str(),
        "wss://api.narilabs.com/v1/realtime?intent=transcription"
    );
    assert_eq!(
        Provider::from_url("https://api.narilabs.com"),
        Some(Provider::Nari)
    );
    assert_eq!(Provider::from_url("https://evilnarilabs.com"), None);
    let Message::Text(audio) = adapter.audio_to_message(bytes::Bytes::from_static(&[0, 1, 2, 3]))
    else {
        panic!()
    };
    let audio: serde_json::Value = serde_json::from_str(&audio).unwrap();
    assert_eq!(audio["type"], "input_audio_buffer.append");
    assert_eq!(audio["audio"], "AAECAw==");
}

#[test]
fn partial_revisions_replace_text_and_final_uses_utterance_timing() {
    let adapter = NariAdapter::default();
    adapter.parse_response(
        r#"{"type":"input_audio_buffer.speech_started","item_id":"a","audio_start_ms":2000}"#,
    );
    for text in ["I scream", "Ice cream"] {
        let responses = adapter.parse_response(
            &serde_json::json!({"type":"transcript.partial","item_id":"a","transcript":text})
                .to_string(),
        );
        assert_eq!(transcript(&responses[0]), text);
    }
    adapter.parse_response(
        r#"{"type":"input_audio_buffer.speech_stopped","item_id":"a","audio_end_ms":3500}"#,
    );
    let responses = final_event(&adapter, "a", "Ice cream is delicious.");
    assert!(!finalized(&responses));
    match &responses[0] {
        StreamResponse::TranscriptResponse {
            start,
            duration,
            channel,
            ..
        } => {
            assert_eq!((*start, *duration), (2.0, 1.5));
            assert_eq!(channel.alternatives[0].words.len(), 1);
            assert_eq!(channel.alternatives[0].words[0].speaker, None);
        }
        _ => panic!(),
    }
    assert!(final_event(&adapter, "a", "duplicate").is_empty());
    assert!(
        adapter
            .parse_response(r#"{"type":"transcript.partial","item_id":"a","transcript":"stale"}"#)
            .is_empty()
    );
}

#[test]
fn empty_final_commit_waits_for_pending_results_in_order() {
    let adapter = NariAdapter::default();
    for id in ["a", "b"] {
        adapter.parse_response(
            &serde_json::json!({"type":"input_audio_buffer.committed","item_id":id}).to_string(),
        );
    }
    assert!(final_event(&adapter, "b", "second").is_empty());
    assert!(!finalized(&adapter.parse_response(&serde_json::json!({"type":"input_audio_buffer.commit_empty","client_event_id":FINALIZE_ID}).to_string())));
    let responses = final_event(&adapter, "a", "first");
    assert_eq!(responses.len(), 3);
    assert_eq!(transcript(&responses[0]), "first");
    assert_eq!(transcript(&responses[1]), "second");
    assert!(finalized(&responses));
}

#[test]
fn stop_ack_does_not_finish_before_its_transcript() {
    let adapter = NariAdapter::default();
    let ack = serde_json::json!({"type":"input_audio_buffer.committed","item_id":"a","client_event_id":FINALIZE_ID});
    assert!(!finalized(&adapter.parse_response(&ack.to_string())));
    assert!(finalized(&final_event(&adapter, "a", "last words")));
    let empty = NariAdapter::default();
    assert!(finalized(&empty.parse_response(&serde_json::json!({"type":"input_audio_buffer.commit_empty","client_event_id":FINALIZE_ID}).to_string())));
}

#[test]
fn forked_channels_do_not_share_pending_utterances() {
    let adapter = NariAdapter::default();
    adapter.parse_response(r#"{"type":"input_audio_buffer.committed","item_id":"mic"}"#);
    let speaker = adapter.fork_session();
    assert_eq!(
        transcript(&final_event(&speaker, "speaker", "system audio")[0]),
        "system audio"
    );
    assert_eq!(adapter.state.lock().unwrap().items.len(), 1);
}

#[test]
fn credit_errors_are_reported_and_pending_state_is_bounded() {
    let adapter = NariAdapter::default();
    let errors = adapter.parse_response(
        r#"{"type":"error","error":{"code":"INSUFFICIENT_CREDITS","message":"Add credits"}}"#,
    );
    assert!(
        matches!(&errors[0],StreamResponse::ErrorResponse {error_code:Some(402),provider,..} if provider=="nari")
    );
    for id in 0..64 {
        adapter.parse_response(
            &serde_json::json!({"type":"input_audio_buffer.committed","item_id":id.to_string()})
                .to_string(),
        );
    }
    let errors =
        adapter.parse_response(r#"{"type":"input_audio_buffer.committed","item_id":"overflow"}"#);
    assert!(matches!(&errors[0], StreamResponse::ErrorResponse { .. }));
    assert_eq!(adapter.state.lock().unwrap().items.len(), 64);
}
