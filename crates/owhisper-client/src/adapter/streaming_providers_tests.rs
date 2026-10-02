use super::*;
use anlg_ws_client::client::Message;
use owhisper_interface::{ListenParams, stream::StreamResponse};

fn transcript(response: &StreamResponse) -> (&str, bool, f64, f64) {
    match response {
        StreamResponse::TranscriptResponse {
            channel,
            is_final,
            start,
            duration,
            ..
        } => (
            &channel.alternatives[0].transcript,
            *is_final,
            *start,
            *duration,
        ),
        _ => panic!("expected transcript"),
    }
}

#[test]
fn inworld_replaces_partials_and_keeps_each_turn() {
    let adapter = InworldAdapter::default();
    adapter.audio_to_message(vec![0; 32000].into());
    let partial = adapter
        .parse_response(r#"{"result":{"transcription":{"transcript":"hello","isFinal":false}}}"#);
    let final_result = adapter.parse_response(
        r#"{"result":{"transcription":{"transcript":"hello world","isFinal":true}}}"#,
    );
    assert_eq!(transcript(&partial[0]), ("hello", false, 0.0, 1.0));
    assert_eq!(
        transcript(&final_result[0]),
        ("hello world", true, 0.0, 1.0)
    );
    adapter.audio_to_message(vec![0; 32000].into());
    let next = adapter.parse_response(
        r#"{"result":{"transcription":{"transcript":"next turn","isFinal":true}}}"#,
    );
    assert_eq!(transcript(&next[0]), ("next turn", true, 1.0, 1.0));
    let mic = adapter.fork_session();
    mic.audio_to_message(vec![0; 16000].into());
    let isolated =
        mic.parse_response(r#"{"result":{"transcription":{"transcript":"mic","isFinal":true}}}"#);
    assert_eq!(transcript(&isolated[0]), ("mic", true, 0.0, 0.5));
}

#[test]
fn gradium_waits_for_segment_end_before_committing_text() {
    let adapter = GradiumAdapter::default();
    adapter.audio_to_message(vec![0; 96000].into());
    adapter.parse_response(r#"{"type":"text","text":"Hello","start_s":0.5,"stream_id":0}"#);
    adapter.parse_response(r#"{"type":"text","text":"world","start_s":1.5,"stream_id":0}"#);
    let final_result = adapter.parse_response(r#"{"type":"end_text","stop_s":2.5,"stream_id":0}"#);
    assert_eq!(
        transcript(&final_result[0]),
        ("Hello world", true, 0.5, 2.0)
    );
    let done = adapter.parse_response(r#"{"type":"end_of_stream"}"#);
    assert!(matches!(
        done[0],
        StreamResponse::TranscriptResponse {
            from_finalize: true,
            ..
        }
    ));
}

#[tokio::test]
async fn modulate_sends_pcm_and_retains_segment_speaker_and_timing() {
    let adapter = ModulateAdapter::default();
    let url = adapter
        .build_ws_url_with_api_key(
            "https://platform.modulate.ai",
            &ListenParams::default(),
            1,
            Some("test+key"),
        )
        .await
        .unwrap();
    assert!(
        url.query_pairs()
            .any(|(key, value)| key == "api_key" && value == "test+key")
    );
    assert!(
        url.query_pairs()
            .any(|(key, value)| key == "endpointing" && value == "true")
    );
    assert!(matches!(adapter.finalize_message(), Message::Text(value) if value.is_empty()));
    let result = adapter.parse_response(r#"{"type":"utterance","utterance":{"text":"Bonjour","start_ms":1500,"duration_ms":700,"speaker":2}}"#);
    assert_eq!(
        transcript(&result[0]),
        ("Bonjour", true, 1.5, 0.7000000000000002)
    );
    match &result[0] {
        StreamResponse::TranscriptResponse { channel, .. } => {
            assert_eq!(channel.alternatives[0].words[0].speaker, Some(2))
        }
        _ => unreachable!(),
    }
}

#[test]
fn alebex_authenticates_with_subprotocol_and_commits_final() {
    let adapter = AlebexAdapter::default();
    assert_eq!(
        adapter.build_auth_header(Some("wt_test")),
        Some(("Sec-WebSocket-Protocol", "alebex.token.wt_test".into()))
    );
    adapter.audio_to_message(vec![0; 32000].into());
    let result = adapter.parse_response(r#"{"type":"final","text":"Finished"}"#);
    assert_eq!(transcript(&result[0]), ("Finished", true, 0.0, 1.0));
    assert!(matches!(
        adapter.parse_response(r#"{"type":"error","message":"Invalid token"}"#)[0],
        StreamResponse::ErrorResponse { .. }
    ));
}

#[test]
fn nvidia_preserves_word_alignment_and_final_drain_marker() {
    let adapter = NvidiaAdapter::default();
    let result = adapter.parse_response(r#"{"type":"conversation.item.input_audio_transcription.completed","transcript":"Hello","words_info":{"words":[{"word":"Hello","start_time":1.0,"end_time":1.5,"speaker_tag":3,"confidence":0.9}]},"is_last_result":true}"#);
    assert_eq!(transcript(&result[0]), ("Hello", true, 1.0, 0.5));
    assert!(matches!(
        result[1],
        StreamResponse::TranscriptResponse {
            from_finalize: true,
            ..
        }
    ));
    match &result[0] {
        StreamResponse::TranscriptResponse { channel, .. } => {
            assert_eq!(channel.alternatives[0].words[0].speaker, Some(3))
        }
        _ => unreachable!(),
    }
}

#[test]
fn provider_selection_survives_custom_and_loopback_endpoints() {
    for (provider, expected) in [
        ("inworld", AdapterKind::Inworld),
        ("gradium", AdapterKind::Gradium),
        ("modulate", AdapterKind::Modulate),
        ("alebex", AdapterKind::Alebex),
        ("nvidia", AdapterKind::Nvidia),
        ("amazon_bedrock", AdapterKind::AmazonBedrock),
    ] {
        assert_eq!(
            AdapterKind::from_url_and_languages(
                &format!("http://localhost:9000?provider={provider}"),
                &[],
                None
            ),
            expected
        );
    }
    assert_eq!(
        AdapterKind::from_url_and_languages(
            "https://api.anarlog.so/stt?provider=inworld",
            &[],
            None
        ),
        AdapterKind::Anarlog
    );
    let url = NvidiaAdapter::default().build_ws_url(
        "http://localhost:9000?provider=nvidia",
        &ListenParams::default(),
        1,
    );
    assert_eq!(
        url.as_str(),
        "ws://localhost:9000/v1/realtime?intent=transcription"
    );
}

#[tokio::test]
async fn nvidia_mints_a_fresh_handshake_token_for_each_audio_channel() {
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/realtime/transcription_sessions"))
        .and(header("authorization", "Bearer mint-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "client_secret":{"value":"ephemeral"}
        })))
        .mount(&server)
        .await;
    let adapter = NvidiaAdapter::default();
    let url = adapter
        .build_ws_url_with_api_key(&server.uri(), &ListenParams::default(), 1, Some("mint-key"))
        .await
        .unwrap();
    assert!(!url.as_str().contains("ephemeral"));
    assert_eq!(
        adapter.build_auth_header(Some("mint-key")),
        Some((
            "Sec-WebSocket-Protocol",
            "realtime, realtime-token.ephemeral".into()
        ))
    );
    Mock::given(method("POST"))
        .and(path("/v1/realtime/transcription_sessions"))
        .and(header("authorization", "Bearer mic-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "client_secret":{"value":"mic-ephemeral"}
        })))
        .mount(&server)
        .await;
    let mic = adapter.fork_session();
    assert!(mic.build_auth_header(None).is_none());
    mic.build_ws_url_with_api_key(&server.uri(), &ListenParams::default(), 1, Some("mic-key"))
        .await
        .unwrap();
    assert_eq!(
        mic.build_auth_header(Some("mic-key")),
        Some((
            "Sec-WebSocket-Protocol",
            "realtime, realtime-token.mic-ephemeral".into()
        ))
    );
}
