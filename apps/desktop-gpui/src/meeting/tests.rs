use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anlg_listener_core::LiveTranscriptDelta;
use anlg_transcript::{FinalizedWord, WordState};
use desktop_runtime::{CancellationToken, Profile, RuntimeHandle, ServiceError, SessionId};
use serde_json::{Value, json};

use super::ai::{AiServices, Message, Part, ProviderAdapter, ProviderEvent, Request, Role};
use super::model::{Interval, MAX_TEXT, Retention, Word, recovered_additions, validate_delta};
use super::persistence::{Persistence, coalesce};
use super::retention::Activities;
use super::store::{SpeakerAssignment, SpeakerScope, TranscriptStore};
use futures::{SinkExt, StreamExt, future::BoxFuture, stream::BoxStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct Fixture {
    runtime: RuntimeHandle,
    store: TranscriptStore,
    session: SessionId,
    transcript: Arc<str>,
    directory: PathBuf,
    tempdir: tempfile::TempDir,
}

impl Fixture {
    async fn setting(&self, id: &str, value: Value) {
        self.sql(
            "INSERT OR REPLACE INTO app_settings (id, value_json) VALUES (?, ?)",
            vec![json!(id), json!(value.to_string())],
        )
        .await;
    }

    fn providers(&self, url: &str) -> super::config::ProviderServices {
        let url = url.to_owned();
        super::config::ProviderServices {
            runtime: self.runtime.clone(),
            api_url: url.clone(),
            cloud: Arc::new(|| {
                Box::pin(async {
                    Ok(super::config::CloudAccess {
                        access_token: "fixture-token".into(),
                        user_id: "fixture-owner".into(),
                        is_paid: true,
                    })
                })
            }),
            local: Arc::new(move |_| {
                let url = url.clone();
                Box::pin(async move { Ok(url) })
            }),
            on_device: Arc::new(|| Box::pin(async { Default::default() })),
            secret: Arc::new(|_, _| Box::pin(async { Ok(Some("fixture-key".into())) })),
            secret_write: Arc::new(|_, _, _| Box::pin(async { Ok(()) })),
        }
    }
}

struct NetworkFixture {
    url: String,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for NetworkFixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn provider_fixture() -> NetworkFixture {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut probe = [0; 3];
                stream.peek(&mut probe).await.unwrap();
                if &probe == b"GET" {
                    let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
                    let mut sent = false;
                    while let Some(Ok(message)) = socket.next().await {
                        if message.is_binary() && !sent {
                            sent = true;
                            let response = json!({
                                "type":"Results", "start":0.0, "duration":1.0,
                                "is_final":true, "speech_final":true, "from_finalize":false,
                                "channel_index":[0,2],
                                "channel":{"alternatives":[{"transcript":"durable résumé", "confidence":1.0,
                                    "words":[{"word":"durable", "start":0.0,"end":0.4,"confidence":1.0,"speaker":0},
                                        {"word":"résumé","start":0.4,"end":1.0,"confidence":1.0,"speaker":0}]}]},
                                "metadata":{"request_id":"fixture","model_uuid":"fixture","model_info":{"name":"fixture","version":"1","arch":"test"}}
                            });
                            assert!(
                                !owhisper_client::RealtimeSttAdapter::parse_response(
                                    &owhisper_client::DeepgramAdapter,
                                    &response.to_string()
                                )
                                .is_empty()
                            );
                            socket
                                .send(tokio_tungstenite::tungstenite::Message::Text(
                                    response.to_string().into(),
                                ))
                                .await
                                .unwrap();
                            let mut continuation = response;
                            continuation["start"] = json!(3.0);
                            for word in continuation["channel"]["alternatives"][0]["words"]
                                .as_array_mut()
                                .unwrap()
                            {
                                word["start"] = json!(word["start"].as_f64().unwrap() + 3.0);
                                word["end"] = json!(word["end"].as_f64().unwrap() + 3.0);
                            }
                            socket
                                .send(tokio_tungstenite::tungstenite::Message::Text(
                                    continuation.to_string().into(),
                                ))
                                .await
                                .unwrap();
                        }
                        if message
                            .to_text()
                            .is_ok_and(|text| text.contains("Finalize"))
                        {
                            for channel in 0..2 {
                                let response = json!({"type":"Results","start":4.0,"duration":0.0,"is_final":true,"speech_final":true,"from_finalize":true,"channel_index":[channel,2],"channel":{"alternatives":[{"transcript":"","confidence":1.0,"words":[]}]},"metadata":{"request_id":"fixture","model_uuid":"fixture","model_info":{"name":"fixture","version":"1","arch":"test"}}});
                                socket
                                    .send(tokio_tungstenite::tungstenite::Message::Text(
                                        response.to_string().into(),
                                    ))
                                    .await
                                    .unwrap();
                            }
                        }
                        if message.is_close() {
                            break;
                        }
                    }
                } else {
                    let mut data = Vec::new();
                    let headers = loop {
                        let mut buffer = [0; 8192];
                        let count = stream.read(&mut buffer).await.unwrap();
                        if count == 0 {
                            return;
                        }
                        data.extend_from_slice(&buffer[..count]);
                        if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                            break end + 4;
                        }
                        assert!(data.len() < 65536);
                    };
                    let header = String::from_utf8_lossy(&data[..headers]);
                    let length = header
                        .lines()
                        .find_map(|line| {
                            let (key, value) = line.split_once(':')?;
                            key.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    assert!(length < 32 * 1024 * 1024);
                    let ai = header.contains("/chat/completions");
                    while data.len() < headers + length {
                        let mut buffer = [0; 8192];
                        let count = stream.read(&mut buffer).await.unwrap();
                        if count == 0 {
                            return;
                        }
                        data.extend_from_slice(&buffer[..count]);
                    }
                    let (mime, body) = if ai {
                        let request: Value =
                            serde_json::from_slice(&data[headers..headers + length]).unwrap();
                        assert_eq!(request["stream"], true);
                        ("text/event-stream", "data: {\"choices\":[{\"delta\":{\"content\":\"fixture résumé\"},\"finish_reason\":null}]}\n\ndata: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n".to_owned())
                    } else {
                        assert!(data.len() > headers);
                        ("application/json", json!({"metadata": {}, "results": {"channels": [{"alternatives": [{"transcript":"recovered résumé", "confidence":1.0,"words":[{"word":"recovered","start":0.0,"end":0.1,"confidence":1.0,"speaker":0},{"word":"résumé","start":0.1,"end":0.2,"confidence":1.0,"speaker":0}]}]}]}}).to_string())
                    };
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    stream.write_all(response.as_bytes()).await.unwrap();
                }
            });
        }
    });
    NetworkFixture { url, task }
}

#[tokio::test]
async fn canonical_provider_settings_cloud_admission_and_local_paths() {
    let fixture = Fixture::new().await;
    fixture
        .setting("current_stt_provider", json!("deepgram"))
        .await;
    fixture.setting("current_stt_model", json!("nova-3")).await;
    fixture
        .setting("spoken_languages", json!("[\"en\",\"fr\"]"))
        .await;
    fixture
        .setting(
            "ai_provider:stt:deepgram",
            json!({"base_url":"http://127.0.0.1:4321"}),
        )
        .await;
    fixture.setting("audio_retention", json!("none")).await;
    let mut services = fixture.providers("http://127.0.0.1:4321");
    let capture = services.capture(fixture.session.clone()).await.unwrap();
    assert_eq!(capture.params.api_key, "fixture-key");
    assert_eq!(capture.params.base_url, "http://127.0.0.1:4321");
    assert_eq!(capture.retention, Retention::Never);
    assert_eq!(capture.params.languages.len(), 2);
    fixture
        .setting("current_stt_provider", json!("anarlog"))
        .await;
    fixture.setting("current_stt_model", json!("cloud")).await;
    services.cloud = Arc::new(|| {
        Box::pin(async {
            Ok(super::config::CloudAccess {
                access_token: "fixture".into(),
                user_id: "owner".into(),
                is_paid: false,
            })
        })
    });
    assert!(services.capture(fixture.session.clone()).await.is_err());
    fixture
        .setting("current_stt_provider", json!("local_file"))
        .await;
    fixture
        .setting("current_stt_model", json!("local-file"))
        .await;
    assert!(services.capture(fixture.session.clone()).await.is_err());
    fixture
        .setting("local_stt_model_path", json!("/fixture/model.bin"))
        .await;
    let local = services.capture(fixture.session.clone()).await.unwrap();
    assert!(local.params.api_key.is_empty());
    assert!(matches!(
        local.params.effective_transcription_mode(),
        anlg_listener_core::TranscriptionMode::Batch
    ));
    fixture.close().await;
}

#[tokio::test]
async fn canonical_context_http_stream_and_tool_approval_persist() {
    let fixture = Fixture::new().await;
    let server = provider_fixture().await;
    fixture
        .setting("current_llm_provider", json!("custom"))
        .await;
    fixture.setting("current_llm_model", json!("fixture")).await;
    fixture
        .setting("ai_provider:llm:custom", json!({"base_url":server.url}))
        .await;
    let services = fixture.providers(&server.url);
    let context = super::context::load(&fixture.runtime, fixture.session.clone())
        .await
        .unwrap();
    assert!(context.history[0].role == Role::System);
    let views = services.ai_services(Arc::new(|_| false)).unwrap();
    let chat = views
        .ai
        .chat(fixture.session.clone(), context.group)
        .unwrap();
    let result = chat
        .send(
            Message {
                id: "fixture-http-user".into(),
                role: Role::User,
                parts: vec![Part::Text {
                    text: "Summarize".into(),
                }],
            },
            context.history,
            None,
            Arc::new(|_| {}),
        )
        .await
        .unwrap();
    assert!(matches!(&result.parts[0], Part::Text { text } if text == "fixture résumé"));
    // A chat memo edit stores its proposal, opens its review and waits for the decision.
    let tool = super::ai::MeetingTool::EditMemo {
        session_id: None,
        content: "Approved résumé".into(),
    };
    let opened = Arc::new(std::sync::Mutex::new(Vec::<Arc<str>>::new()));
    let reviews = opened.clone();
    let context = super::ai::ToolContext {
        call_id: "call-memo".into(),
        session: Some(fixture.session.0.to_string()),
        review: Some(Arc::new(move |id| reviews.lock().unwrap().push(id))),
        ..Default::default()
    };
    let execute = super::tools::executor(fixture.runtime.clone(), views.approvals.clone(), None);
    let pending = tokio::spawn(execute(tool, context, CancellationToken::new()));
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !views.approvals.is_pending("call-memo") {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        opened.lock().unwrap().as_slice(),
        [Arc::<str>::from("call-memo")]
    );
    let stored = crate::workspace::proposals::load(&fixture.runtime, "call-memo".into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (stored.status.as_ref(), stored.source.as_ref()),
        ("pending", "chat")
    );
    // The card or tab applies first; the tool's own apply is then a no-op.
    crate::workspace::proposals::apply(&fixture.runtime, "call-memo".into())
        .await
        .unwrap();
    assert!(views.approvals.resolve("call-memo", true));
    assert_eq!(
        pending.await.unwrap().unwrap(),
        json!({"status": "applied"})
    );
    let updated = fixture
        .runtime
        .open_session(fixture.session.clone(), CancellationToken::new())
        .unwrap()
        .receive()
        .await
        .unwrap()
        .note
        .unwrap();
    assert!(updated.body.contains("Approved résumé"));
    let (summary, context) = super::context::load(&fixture.runtime, fixture.session.clone())
        .await
        .unwrap()
        .summary
        .unwrap();
    views
        .ai
        .summarize(summary, context, CancellationToken::new(), Arc::new(|_| {}))
        .await
        .unwrap();
    drop(chat);
    drop(views);
    fixture.close().await;
}

#[tokio::test]
async fn context_preview_preserves_rows_and_summary_preparation_reuses_its_document() {
    let fixture = Fixture::new().await;
    let resolver = super::context::resolver(fixture.runtime.clone());
    let documents = fixture
        .sql("SELECT * FROM session_documents ORDER BY id", vec![])
        .await;
    for _ in 0..2 {
        let context = resolver(
            fixture.session.clone(),
            super::ai_view::ContextPurpose::Preview,
        )
        .await
        .unwrap();
        assert!(context.summary.is_none());
        assert!(context.history[0].role == Role::System);
        assert_eq!(
            fixture
                .sql("SELECT * FROM session_documents ORDER BY id", vec![])
                .await,
            documents
        );
        assert!(
            fixture
                .sql("SELECT * FROM chat_groups", vec![])
                .await
                .is_empty()
        );
    }
    let prepared = resolver(
        fixture.session.clone(),
        super::ai_view::ContextPurpose::Summarize,
    )
    .await
    .unwrap()
    .summary
    .unwrap()
    .0;
    for purpose in [
        super::ai_view::ContextPurpose::Preview,
        super::ai_view::ContextPurpose::Summarize,
    ] {
        let summary = resolver(fixture.session.clone(), purpose)
            .await
            .unwrap()
            .summary
            .unwrap()
            .0;
        assert_eq!(summary.id, prepared.id);
        assert_eq!(summary.body, prepared.body);
        assert_eq!(summary.updated_at, prepared.updated_at);
    }
    assert_eq!(
        fixture
            .sql("SELECT id FROM session_documents", vec![])
            .await
            .len(),
        documents.len() + 1
    );
    assert!(
        fixture
            .sql("SELECT * FROM chat_groups", vec![])
            .await
            .is_empty()
    );
    fixture.close().await;
}

#[tokio::test]
async fn first_chat_send_creates_its_group_without_creating_a_summary() {
    let fixture = Fixture::new().await;
    let provider: Arc<dyn ProviderAdapter> =
        Arc::new(FixtureProvider(Arc::new(AtomicBool::new(false))));
    let ai = Arc::new(
        AiServices::new(
            fixture.runtime.clone(),
            Arc::new(move |_| {
                let provider = provider.clone();
                Box::pin(async move { Ok(provider) })
            }),
            Arc::new(|_, _, _| Box::pin(async { Err(super::model::failure("No fixture tools")) })),
            Arc::new(|_| false),
        )
        .unwrap(),
    );
    let chat = ai
        .chat(fixture.session.clone(), fixture.session.0.clone())
        .unwrap();
    assert!(
        fixture
            .sql("SELECT * FROM chat_groups", vec![])
            .await
            .is_empty()
    );
    let user = Message {
        id: "first-message".into(),
        role: Role::User,
        parts: vec![Part::Text {
            text: "question".into(),
        }],
    };
    chat.send(user.clone(), vec![], None, Arc::new(|_| {}))
        .await
        .unwrap();
    assert_eq!(
        fixture
            .sql("SELECT id FROM chat_groups", vec![])
            .await
            .len(),
        1
    );
    assert_eq!(
        fixture
            .sql("SELECT id FROM chat_messages", vec![])
            .await
            .len(),
        2
    );
    assert!(
        fixture
            .sql(
                "SELECT id FROM session_documents WHERE kind = 'summary'",
                vec![]
            )
            .await
            .is_empty()
    );
    fixture
        .sql("UPDATE chat_groups SET deleted_at = 'deleted'", vec![])
        .await;
    assert!(
        chat.send(user, vec![], None, Arc::new(|_| {}))
            .await
            .is_err()
    );
    assert_eq!(
        fixture
            .sql("SELECT id FROM chat_messages", vec![])
            .await
            .len(),
        2
    );
    assert!(
        fixture
            .sql(
                "SELECT id FROM chat_groups WHERE deleted_at IS NULL",
                vec![]
            )
            .await
            .is_empty()
    );
    chat.flush().await;
    fixture.close().await;
}

#[tokio::test]
async fn provider_admission_failure_exits_preparing_and_allows_retry() {
    let fixture = Fixture::new().await;
    let services = fixture.providers("http://127.0.0.1");
    let capture = super::capture::CaptureService::spawn(
        fixture.runtime.clone(),
        Arc::new(anlg_audio_mock::MockAudio::new(1)),
        Arc::new(FixtureStorage(fixture.directory.clone())),
        services.start_resolver(),
    )
    .unwrap();
    for _ in 0..2 {
        let error = capture
            .start(fixture.session.clone())
            .unwrap()
            .await
            .unwrap()
            .unwrap_err();
        assert!(error.to_string().contains("Choose a provider"));
        let update = capture.take_update(0);
        assert_eq!(update.session, Some(fixture.session.clone()));
        assert_eq!(update.phase, super::capture::Phase::Failed);
        assert_eq!(update.status.as_ref(), "Recording could not start.");
        assert!(update.error.is_some());
    }
    drop(capture);
    fixture.close().await;
}

#[tokio::test]
async fn retrying_capture_prepare_replaces_the_lifecycle_marker() {
    let fixture = Fixture::new().await;
    let config = super::capture::CaptureConfig {
        params: anlg_listener_core::actors::SessionParams {
            session_id: fixture.session.0.to_string(),
            retain_audio: Some(true),
            languages: Vec::new(),
            onboarding: false,
            transcription_mode: anlg_listener_core::TranscriptionMode::Live,
            model: "nova-3".into(),
            base_url: String::new(),
            api_key: String::new(),
            keywords: Vec::new(),
            mic_device: None,
            participant_human_ids: Vec::new(),
            self_human_id: None,
            speaker_assignments: Vec::new(),
            capture_started_at_ms: None,
        },
        provider: "deepgram".into(),
        retention: Retention::Forever,
        memo: String::new(),
        recovery: None,
    };
    let first: Arc<str> = uuid::Uuid::new_v4().to_string().into();
    let second: Arc<str> = uuid::Uuid::new_v4().to_string().into();
    let (first_started, _) = super::capture::prepare(
        &fixture.runtime,
        &fixture.session,
        first.clone(),
        &config,
        0,
    )
    .await
    .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    let directory =
        super::retention::session_directory(&fixture.directory, &fixture.session).unwrap();
    let chunks = directory.join("audio-recovery");
    std::fs::create_dir_all(&chunks).unwrap();
    let pending_chunk = chunks.join(format!("{first_started}-0-1000-0.mp3"));
    silent_chunk(&pending_chunk);

    let (second_started, _) = super::capture::prepare(
        &fixture.runtime,
        &fixture.session,
        second.clone(),
        &config,
        0,
    )
    .await
    .unwrap();

    let rows = fixture
        .sql(
            "SELECT value_json FROM app_settings WHERE id = ?",
            vec![json!(format!(
                "capture_lifecycle_pending:{}",
                fixture.session.0
            ))],
        )
        .await;
    assert_eq!(rows.len(), 1);
    let marker: Value = serde_json::from_str(rows[0]["value_json"].as_str().unwrap()).unwrap();
    let (created_at, owner) = (marker["createdAt"].clone(), marker["ownerUserId"].clone());
    assert_eq!(marker["transcriptId"], json!(second.to_string()));
    assert_eq!(
        marker["inheritedCaptures"],
        json!([{
            "transcriptId": first.to_string(),
            "startedAt": first_started,
            "createdAt": created_at,
            "ownerUserId": owner,
            "memo": "",
            "retainAudio": true,
            "provider": "deepgram",
            "model": "nova-3",
        }])
    );

    let current_chunk = chunks.join(format!("{second_started}-0-1000-0.mp3"));
    silent_chunk(&current_chunk);
    let primary = super::recovery::CaptureTarget {
        transcript: second.clone(),
        started: second_started as i64,
        provider: String::new(),
        model: String::new(),
        memo: String::new(),
        created_at: None,
        owner_user_id: String::new(),
        retained: true,
    };
    let inherited = marker["inheritedCaptures"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(super::recovery::parse_target)
        .collect::<Vec<_>>();
    super::capture::release_attempt_chunks(&directory, &primary, &inherited, i64::MAX, true)
        .await
        .unwrap();
    assert!(pending_chunk.exists());
    assert!(!current_chunk.exists());
    fixture.close().await;
}

#[tokio::test]
async fn retrying_after_a_backward_clock_keeps_the_earlier_pending_target() {
    let fixture = Fixture::new().await;
    let config = super::capture::CaptureConfig {
        params: anlg_listener_core::actors::SessionParams {
            session_id: fixture.session.0.to_string(),
            retain_audio: Some(true),
            languages: Vec::new(),
            onboarding: false,
            transcription_mode: anlg_listener_core::TranscriptionMode::Live,
            model: "nova-3".into(),
            base_url: String::new(),
            api_key: String::new(),
            keywords: Vec::new(),
            mic_device: None,
            participant_human_ids: Vec::new(),
            self_human_id: None,
            speaker_assignments: Vec::new(),
            capture_started_at_ms: None,
        },
        provider: "deepgram".into(),
        retention: Retention::Forever,
        memo: String::new(),
        recovery: None,
    };
    let first: Arc<str> = uuid::Uuid::new_v4().to_string().into();
    let second: Arc<str> = uuid::Uuid::new_v4().to_string().into();

    let directory =
        super::retention::session_directory(&fixture.directory, &fixture.session).unwrap();
    let chunks = directory.join("audio-recovery");
    std::fs::create_dir_all(&chunks).unwrap();

    // Simulate a backward wall-clock adjustment: the earlier attempt's marker and chunk carry a
    // start time in the future relative to the retry's `now`. Its target must still be carried
    // into inheritedCaptures, and its chunk must survive the retry's release.
    let future_started: i64 = 4_102_444_800_000; // 2100-01-01, far ahead of any real `now`.
    let pending_chunk = chunks.join(format!("{future_started}-0-1000-0.mp3"));
    silent_chunk(&pending_chunk);
    let key = format!("capture_lifecycle_pending:{}", fixture.session.0);
    fixture
        .setting(
            &key,
            json!({
                "version": 1,
                "sessionId": fixture.session,
                "transcriptId": first,
                "retainAudio": true,
                "startedAt": future_started,
                "createdAt": "2026-11-26T00:00:00Z",
                "ownerUserId": "",
                "memo": "",
                "provider": "deepgram",
                "model": "nova-3",
            }),
        )
        .await;

    let (second_started, _) = super::capture::prepare(
        &fixture.runtime,
        &fixture.session,
        second.clone(),
        &config,
        0,
    )
    .await
    .unwrap();

    let rows = fixture
        .sql(
            "SELECT value_json FROM app_settings WHERE id = ?",
            vec![json!(key)],
        )
        .await;
    let marker: Value = serde_json::from_str(rows[0]["value_json"].as_str().unwrap()).unwrap();
    assert_eq!(marker["transcriptId"], json!(second.to_string()));
    let inherited = marker["inheritedCaptures"].as_array().unwrap();
    assert_eq!(inherited.len(), 1);
    assert_eq!(inherited[0]["transcriptId"], json!(first.to_string()));

    // Releasing the retry's chunks must not delete the earlier attempt's pending chunk.
    let primary = super::recovery::CaptureTarget {
        transcript: second.clone(),
        started: second_started as i64,
        provider: String::new(),
        model: String::new(),
        memo: String::new(),
        created_at: None,
        owner_user_id: String::new(),
        retained: true,
    };
    let targets = marker["inheritedCaptures"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(super::recovery::parse_target)
        .collect::<Vec<_>>();
    super::capture::release_attempt_chunks(&directory, &primary, &targets, i64::MAX, true)
        .await
        .unwrap();
    assert!(pending_chunk.exists());
    fixture.close().await;
}

#[tokio::test]
async fn discarding_one_of_three_pending_attempts_keeps_the_others() {
    let fixture = Fixture::new().await;
    let directory =
        super::retention::session_directory(&fixture.directory, &fixture.session).unwrap();
    let chunks = directory.join("audio-recovery");
    std::fs::create_dir_all(&chunks).unwrap();
    let audio = directory.join("audio.mp3");
    std::fs::write(&audio, b"shared recording").unwrap();
    let stop = directory.join(".delete-audio-on-stop");
    std::fs::write(&stop, b"").unwrap();
    let (a, b, c) = (
        uuid::Uuid::new_v4().to_string(),
        uuid::Uuid::new_v4().to_string(),
        uuid::Uuid::new_v4().to_string(),
    );
    let (a_started, b_started, c_started) = (1000_i64, 2000_i64, 3000_i64);
    let a_chunk = chunks.join(format!("{a_started}-0-1000-0.mp3"));
    let b_chunk = chunks.join(format!("{b_started}-0-1000-0.mp3"));
    let c_chunk = chunks.join(format!("{c_started}-0-1000-0.mp3"));
    silent_chunk(&a_chunk);
    silent_chunk(&b_chunk);
    silent_chunk(&c_chunk);
    // C has a saved transcript row with no end time; discard must close it.
    fixture
        .sql(
            "INSERT INTO transcripts (id, session_id, started_at_ms, source) VALUES (?, ?, ?, 'live_capture')",
            vec![json!(c), json!(fixture.session), json!(c_started)],
        )
        .await;
    let key = format!("capture_lifecycle_pending:{}", fixture.session.0);
    fixture
        .setting(
            &key,
            json!({
                "version": 1,
                "sessionId": fixture.session,
                "transcriptId": c,
                "retainAudio": true,
                "startedAt": c_started,
                "inheritedCaptures": [
                    {"transcriptId": a, "startedAt": a_started, "createdAt": "", "ownerUserId": "", "memo": "", "retainAudio": true, "provider": "deepgram", "model": "nova-3"},
                    {"transcriptId": b, "startedAt": b_started, "createdAt": "", "ownerUserId": "", "memo": "", "retainAudio": true, "provider": "deepgram", "model": "nova-3"},
                ],
            }),
        )
        .await;
    let c_transcript: Arc<str> = c.clone().into();
    super::recovery::discard(
        &fixture.runtime,
        &fixture.directory,
        fixture.session.clone(),
        c_transcript,
        true,
        Activities::default(),
    )
    .await
    .unwrap();
    // Only C's chunk goes; A and B survive for their own transcripts.
    assert!(!c_chunk.exists());
    assert!(a_chunk.exists());
    assert!(b_chunk.exists());
    assert!(audio.exists());
    let marker: Value = serde_json::from_str(
        fixture
            .sql(
                "SELECT value_json FROM app_settings WHERE id = ?",
                vec![json!(key)],
            )
            .await[0]["value_json"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    // The newest remaining attempt (B) is promoted; A stays inherited.
    assert_eq!(marker["transcriptId"], json!(b));
    let inherited = marker["inheritedCaptures"].as_array().unwrap();
    assert_eq!(inherited.len(), 1);
    assert_eq!(inherited[0]["transcriptId"], json!(a));
    // C's transcript is closed even though it was discarded.
    let c_row = fixture
        .sql(
            "SELECT ended_at_ms FROM transcripts WHERE id = ?",
            vec![json!(c)],
        )
        .await;
    assert_eq!(c_row.len(), 1);
    assert!(c_row[0]["ended_at_ms"].as_i64().is_some());
    fixture.close().await;
}

#[tokio::test]
async fn discarding_a_retained_retry_over_a_dont_save_attempt_keeps_the_archive() {
    let fixture = Fixture::new().await;
    let directory =
        super::retention::session_directory(&fixture.directory, &fixture.session).unwrap();
    let chunks = directory.join("audio-recovery");
    std::fs::create_dir_all(&chunks).unwrap();
    let audio = directory.join("audio.mp3");
    std::fs::write(&audio, b"retained recording").unwrap();
    let (a, b) = (
        uuid::Uuid::new_v4().to_string(),
        uuid::Uuid::new_v4().to_string(),
    );
    let (a_started, b_started) = (1000_i64, 2000_i64);
    let a_chunk = chunks.join(format!("{a_started}-0-1000-0.mp3"));
    let b_chunk = chunks.join(format!("{b_started}-0-1000-0.mp3"));
    silent_chunk(&a_chunk);
    silent_chunk(&b_chunk);
    let key = format!("capture_lifecycle_pending:{}", fixture.session.0);
    fixture
        .setting(
            &key,
            json!({
                "version": 1,
                "sessionId": fixture.session,
                "transcriptId": b,
                "retainAudio": true,
                "startedAt": b_started,
                "inheritedCaptures": [
                    {"transcriptId": a, "startedAt": a_started, "createdAt": "", "ownerUserId": "", "memo": "", "retainAudio": false, "provider": "deepgram", "model": "nova-3"},
                ],
            }),
        )
        .await;
    let b_transcript: Arc<str> = b.clone().into();
    super::recovery::discard(
        &fixture.runtime,
        &fixture.directory,
        fixture.session.clone(),
        b_transcript,
        true,
        Activities::default(),
    )
    .await
    .unwrap();
    assert!(!b_chunk.exists());
    assert!(a_chunk.exists());
    // The promoted "Don't save" attempt must not delete the retained retry's archive.
    assert!(audio.exists());
    let marker: Value = serde_json::from_str(
        fixture
            .sql(
                "SELECT value_json FROM app_settings WHERE id = ?",
                vec![json!(key)],
            )
            .await[0]["value_json"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(marker["transcriptId"], json!(a));
    assert_eq!(marker["retainAudio"], json!(true));
    fixture.close().await;
}

#[tokio::test]
async fn backdated_retry_keeps_earlier_chunk_with_distinct_recorder_timestamp() {
    let fixture = Fixture::new().await;
    let directory =
        super::retention::session_directory(&fixture.directory, &fixture.session).unwrap();
    let chunks = directory.join("audio-recovery");
    std::fs::create_dir_all(&chunks).unwrap();
    // The earlier attempt's marker says 10_000 but its recorder chunk is stamped 10_020.
    let (a, b) = (
        uuid::Uuid::new_v4().to_string(),
        uuid::Uuid::new_v4().to_string(),
    );
    let a_marker_started = 10_000_i64;
    let a_chunk_started = 10_020_u64;
    let b_started = 8_000_i64;
    let a_chunk = chunks.join(format!("{a_chunk_started}-0-1000-0.mp3"));
    silent_chunk(&a_chunk);
    let key = format!("capture_lifecycle_pending:{}", fixture.session.0);
    fixture
        .setting(
            &key,
            json!({
                "version": 1,
                "sessionId": fixture.session,
                "transcriptId": a,
                "retainAudio": true,
                "startedAt": a_marker_started,
                "createdAt": "",
                "ownerUserId": "",
                "memo": "",
                "provider": "deepgram",
                "model": "nova-3",
            }),
        )
        .await;
    let primary = super::recovery::CaptureTarget {
        transcript: b.clone().into(),
        started: b_started,
        provider: String::new(),
        model: String::new(),
        memo: String::new(),
        created_at: None,
        owner_user_id: String::new(),
        retained: true,
    };
    let inherited = vec![super::recovery::CaptureTarget {
        transcript: a.clone().into(),
        started: a_marker_started,
        provider: "deepgram".into(),
        model: "nova-3".into(),
        memo: String::new(),
        created_at: None,
        owner_user_id: String::new(),
        retained: true,
    }];
    // The earlier chunk must resolve to the inherited attempt, not the backdated retry.
    super::capture::release_attempt_chunks(&directory, &primary, &inherited, i64::MAX, true)
        .await
        .unwrap();
    assert!(a_chunk.exists());
    fixture.close().await;
}

#[tokio::test]
async fn release_attempt_chunks_releases_current_past_a_full_page_of_inherited() {
    let fixture = Fixture::new().await;
    let directory =
        super::retention::session_directory(&fixture.directory, &fixture.session).unwrap();
    let chunks = directory.join("audio-recovery");
    std::fs::create_dir_all(&chunks).unwrap();
    // 130 inherited chunks that sort before the current capture's chunk.
    let inherited_started = 1_000_i64;
    for index in 0..130 {
        let start = index * 1_000;
        silent_chunk(&chunks.join(format!(
            "{inherited_started}-{start}-{}-{start}.mp3",
            start + 1_000
        )));
    }
    let current_started = 9_000_i64;
    let current_chunk = chunks.join(format!("{current_started}-0-1000-0.mp3"));
    silent_chunk(&current_chunk);
    let primary = super::recovery::CaptureTarget {
        transcript: fixture.transcript.clone(),
        started: current_started,
        provider: String::new(),
        model: String::new(),
        memo: String::new(),
        created_at: None,
        owner_user_id: String::new(),
        retained: true,
    };
    let inherited = vec![super::recovery::CaptureTarget {
        transcript: uuid::Uuid::new_v4().to_string().into(),
        started: inherited_started,
        provider: String::new(),
        model: String::new(),
        memo: String::new(),
        created_at: None,
        owner_user_id: String::new(),
        retained: true,
    }];
    super::capture::release_attempt_chunks(&directory, &primary, &inherited, i64::MAX, true)
        .await
        .unwrap();
    // The current chunk is released even though 130 inherited chunks sort before it.
    assert!(!current_chunk.exists());
    fixture.close().await;
}

#[tokio::test]
async fn remove_delete_on_stop_clears_the_marker() {
    let fixture = Fixture::new().await;
    let directory =
        super::retention::session_directory(&fixture.directory, &fixture.session).unwrap();
    std::fs::create_dir_all(&directory).unwrap();
    let stop = directory.join(".delete-audio-on-stop");
    std::fs::write(&stop, b"").unwrap();
    assert!(
        super::recovery::remove_delete_on_stop(directory.clone())
            .await
            .unwrap()
    );
    assert!(!stop.exists());
    // A second call reports the marker was already gone.
    assert!(
        !super::recovery::remove_delete_on_stop(directory)
            .await
            .unwrap()
    );
    fixture.close().await;
}

#[tokio::test]
async fn stale_discard_leaves_newer_audio_untouched() {
    let fixture = Fixture::new().await;
    let directory =
        super::retention::session_directory(&fixture.directory, &fixture.session).unwrap();
    let chunks = directory.join("audio-recovery");
    std::fs::create_dir_all(&chunks).unwrap();
    let audio = directory.join("audio.mp3");
    std::fs::write(&audio, b"new recording").unwrap();
    let pending = chunks.join("1000-0-1000-0.mp3");
    silent_chunk(&pending);
    let key = format!("capture_lifecycle_pending:{}", fixture.session.0);
    fixture
        .setting(
            &key,
            json!({
                "version": 1,
                "sessionId": fixture.session,
                "transcriptId": fixture.transcript,
                "retainAudio": true,
                "startedAt": 2000,
            }),
        )
        .await;
    let stale: Arc<str> = uuid::Uuid::new_v4().to_string().into();
    let error = super::recovery::discard(
        &fixture.runtime,
        &fixture.directory,
        fixture.session.clone(),
        stale,
        true,
        Activities::default(),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("left untouched"));
    assert!(audio.exists());
    assert!(pending.exists());

    let activities = Activities::default();
    let capture = activities.acquire(fixture.session.clone()).unwrap();
    let error = super::recovery::discard(
        &fixture.runtime,
        &fixture.directory,
        fixture.session.clone(),
        fixture.transcript.clone(),
        true,
        activities.clone(),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("is recording"));
    assert!(audio.exists());
    drop(capture);

    fixture
        .sql("DELETE FROM app_settings WHERE id = ?", vec![json!(key)])
        .await;
    let error = super::recovery::discard(
        &fixture.runtime,
        &fixture.directory,
        fixture.session.clone(),
        fixture.transcript.clone(),
        true,
        activities,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("left untouched"));
    assert!(audio.exists());
    assert!(pending.exists());
    fixture.close().await;
}

#[tokio::test]
async fn discarding_a_retry_keeps_inherited_pending_audio() {
    let fixture = Fixture::new().await;
    let directory =
        super::retention::session_directory(&fixture.directory, &fixture.session).unwrap();
    let chunks = directory.join("audio-recovery");
    std::fs::create_dir_all(&chunks).unwrap();
    let audio = directory.join("audio.mp3");
    std::fs::write(&audio, b"shared recording").unwrap();
    let stop = directory.join(".delete-audio-on-stop");
    std::fs::write(&stop, b"").unwrap();
    let inherited_started: i64 = 1_764_149_100_000;
    let primary_started: i64 = 1_764_149_200_000;
    let inherited: Arc<str> = uuid::Uuid::new_v4().to_string().into();
    let inherited_chunk = chunks.join(format!("{inherited_started}-0-1000-0.mp3"));
    let primary_chunk = chunks.join(format!("{primary_started}-0-1000-0.mp3"));
    silent_chunk(&inherited_chunk);
    silent_chunk(&primary_chunk);
    let key = format!("capture_lifecycle_pending:{}", fixture.session.0);
    fixture
        .setting(
            &key,
            json!({
                "version": 1,
                "sessionId": fixture.session,
                "transcriptId": fixture.transcript,
                "retainAudio": false,
                "startedAt": primary_started,
                "inheritedCaptures": [{
                    "transcriptId": inherited,
                    "startedAt": inherited_started,
                    "createdAt": "2026-11-26T00:00:00Z",
                    "ownerUserId": "",
                    "memo": "",
                    "retainAudio": true,
                    "provider": "deepgram",
                    "model": "nova-3",
                }],
            }),
        )
        .await;
    super::recovery::discard(
        &fixture.runtime,
        &fixture.directory,
        fixture.session.clone(),
        fixture.transcript.clone(),
        false,
        Activities::default(),
    )
    .await
    .unwrap();
    assert!(!primary_chunk.exists());
    assert!(inherited_chunk.exists());
    assert!(audio.exists());
    assert!(!stop.exists());
    let marker: Value = serde_json::from_str(
        fixture
            .sql(
                "SELECT value_json FROM app_settings WHERE id = ?",
                vec![json!(key)],
            )
            .await[0]["value_json"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(marker["transcriptId"], json!(inherited.to_string()));
    assert_eq!(marker["startedAt"], json!(inherited_started));
    assert_eq!(marker["retainAudio"], json!(true));
    assert!(marker.get("inheritedCaptures").is_none());
    fixture.close().await;
}

#[tokio::test]
async fn native_streaming_capture_saves_transcript_and_releases_dont_save_audio() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("listener_core=debug,owhisper_client=debug")
        .with_test_writer()
        .try_init();
    let fixture = Fixture::new().await;
    let server = provider_fixture().await;
    fixture
        .setting("current_stt_provider", json!("deepgram"))
        .await;
    fixture.setting("current_stt_model", json!("nova-3")).await;
    fixture.setting("spoken_languages", json!("[\"en\"]")).await;
    fixture
        .setting("ai_provider:stt:deepgram", json!({"base_url":server.url}))
        .await;
    fixture.setting("audio_retention", json!("none")).await;
    // Chunks left by finished captures must not fill the recovery page.
    let directory =
        super::retention::session_directory(&fixture.directory, &fixture.session).unwrap();
    std::fs::create_dir_all(directory.join("audio-recovery")).unwrap();
    for index in 0..130 {
        std::fs::write(
            directory
                .join("audio-recovery")
                .join(format!("1-{index}-{}-{index}.mp3", index + 1)),
            b"",
        )
        .unwrap();
    }
    let services = fixture.providers(&server.url);
    let capture = super::capture::CaptureService::spawn(
        fixture.runtime.clone(),
        Arc::new(anlg_audio_mock::MockAudio::new(1)),
        Arc::new(FixtureStorage(fixture.directory.clone())),
        services.start_resolver(),
    )
    .unwrap();
    capture
        .start(fixture.session.clone())
        .unwrap()
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(40), async {
        loop {
            let loaded = fixture
                .store
                .load(fixture.session.clone(), CancellationToken::new())
                .unwrap()
                .receive()
                .await
                .unwrap();
            if loaded
                .transcripts
                .iter()
                .any(|t| t.words.iter().any(|w| w.text.trim() == "résumé"))
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    capture.stop().unwrap().await.unwrap().unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(40), async {
        loop {
            let update = capture.take_update(0);
            assert_ne!(
                update.phase,
                super::capture::Phase::Failed,
                "{:?}",
                update.error
            );
            if update.phase == super::capture::Phase::Idle {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        fixture
            .sql(
                "SELECT id FROM app_settings WHERE id LIKE 'capture_lifecycle_pending:%'",
                vec![]
            )
            .await
            .is_empty()
    );
    assert!(!directory.join("audio-recovery").exists());
    assert!(!directory.join(".delete-audio-on-stop").exists());
    let reopened = fixture
        .runtime
        .open_session(fixture.session.clone(), CancellationToken::new())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(reopened.summary.id, fixture.session);
    let loaded = fixture
        .store
        .load(fixture.session.clone(), CancellationToken::new())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert!(
        loaded
            .transcripts
            .iter()
            .any(|t| t.words.iter().any(|w| w.text.trim() == "résumé"))
    );
    drop(capture);
    fixture.close().await;
}

#[tokio::test]
async fn retry_stop_finalizes_current_capture_when_inherited_repair_fails() {
    let fixture = Fixture::new().await;
    let server = provider_fixture().await;
    fixture
        .setting("current_stt_provider", json!("deepgram"))
        .await;
    fixture.setting("current_stt_model", json!("nova-3")).await;
    fixture.setting("spoken_languages", json!("[\"en\"]")).await;
    fixture
        .setting("ai_provider:stt:deepgram", json!({"base_url":server.url}))
        .await;

    let directory =
        super::retention::session_directory(&fixture.directory, &fixture.session).unwrap();
    let chunks = directory.join("audio-recovery");
    std::fs::create_dir_all(&chunks).unwrap();
    let inherited_started: i64 = 1_764_149_100_000;
    let inherited: Arc<str> = uuid::Uuid::new_v4().to_string().into();
    fixture
        .sql(
            "INSERT INTO transcripts (id, session_id, started_at_ms, source) VALUES (?, ?, ?, 'live_capture')",
            vec![json!(inherited), json!(fixture.session), json!(inherited_started)],
        )
        .await;
    let inherited_chunk = chunks.join(format!("{inherited_started}-0-1000-0.mp3"));
    silent_chunk(&inherited_chunk);
    let key = format!("capture_lifecycle_pending:{}", fixture.session.0);
    fixture
        .setting(
            &key,
            json!({
                "version": 1,
                "sessionId": fixture.session,
                "transcriptId": inherited,
                "retainAudio": true,
                "startedAt": inherited_started,
                "createdAt": "2026-11-26T00:00:00Z",
                "ownerUserId": "",
                "memo": "",
                "provider": "deepgram",
                "model": "nova-3",
            }),
        )
        .await;

    let services = fixture.providers(&server.url);
    let start = services.start_resolver();
    let recovery: super::recovery::RecoveryResolver =
        Arc::new(|_, _| Box::pin(async { Err(super::model::failure("no words")) }));
    let capture = super::capture::CaptureService::spawn(
        fixture.runtime.clone(),
        Arc::new(anlg_audio_mock::MockAudio::new(1)),
        Arc::new(FixtureStorage(fixture.directory.clone())),
        Arc::new(move |session| {
            let start = start.clone();
            let recovery = recovery.clone();
            Box::pin(async move {
                let mut config = start(session).await?;
                config.recovery = Some(recovery);
                Ok(config)
            })
        }),
    )
    .unwrap();
    capture
        .start(fixture.session.clone())
        .unwrap()
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(40), async {
        loop {
            let loaded = fixture
                .store
                .load(fixture.session.clone(), CancellationToken::new())
                .unwrap()
                .receive()
                .await
                .unwrap();
            if loaded
                .transcripts
                .iter()
                .any(|t| t.words.iter().any(|w| w.text.trim() == "résumé"))
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    capture.stop().unwrap().await.unwrap().unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(40), async {
        loop {
            let update = capture.take_update(0);
            assert_ne!(
                update.phase,
                super::capture::Phase::Failed,
                "{:?}",
                update.error
            );
            if update.phase == super::capture::Phase::Idle {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let update = capture.take_update(0);
    assert_eq!(update.phase, super::capture::Phase::Idle);
    assert!(inherited_chunk.exists());
    let marker: Value = serde_json::from_str(
        fixture
            .sql(
                "SELECT value_json FROM app_settings WHERE id = ?",
                vec![json!(key)],
            )
            .await[0]["value_json"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let current = marker["transcriptId"].as_str().unwrap();
    assert_ne!(current, inherited.as_ref());
    let transcript = fixture
        .sql(
            "SELECT ended_at_ms, words_json FROM transcripts WHERE id = ?",
            vec![json!(current)],
        )
        .await;
    assert_eq!(transcript.len(), 1);
    assert!(transcript[0]["ended_at_ms"].as_i64().is_some());
    assert!(
        transcript[0]["words_json"]
            .as_str()
            .unwrap()
            .contains("résumé")
    );
    drop(capture);
    fixture.close().await;
}

#[tokio::test]
async fn interrupted_mock_capture_recovers_through_configured_batch_provider() {
    let fixture = Fixture::new().await;
    let server = provider_fixture().await;
    fixture
        .setting("current_stt_provider", json!("deepgram"))
        .await;
    fixture.setting("current_stt_model", json!("nova-3")).await;
    fixture.setting("spoken_languages", json!("[\"en\"]")).await;
    fixture
        .setting("ai_provider:stt:deepgram", json!({"base_url":server.url}))
        .await;
    let services = fixture.providers(&server.url);
    let start = services.start_resolver();
    let capture = super::capture::CaptureService::spawn(
        fixture.runtime.clone(),
        Arc::new(anlg_audio_mock::MockAudio::new(1)),
        Arc::new(FixtureStorage(fixture.directory.clone())),
        Arc::new(move |session| {
            let start = start.clone();
            Box::pin(async move {
                let mut config = start(session).await?;
                config.params.transcription_mode = anlg_listener_core::TranscriptionMode::Batch;
                config.recovery = None;
                Ok(config)
            })
        }),
    )
    .unwrap();
    capture
        .start(fixture.session.clone())
        .unwrap()
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        while capture.take_update(0).phase != super::capture::Phase::Listening {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        while capture.take_update(0).amplitude == (0, 0) {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    capture.stop().unwrap().await.unwrap().unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        while capture.take_update(0).phase != super::capture::Phase::Failed {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    drop(capture);
    let reports = super::recovery::recover_startup(
        &fixture.runtime,
        fixture.directory.clone(),
        Activities::default(),
        services.recovery_resolver(),
    )
    .await
    .unwrap();
    assert_eq!(reports.len(), 1);
    reports[0].result.as_ref().unwrap();
    let loaded = fixture
        .store
        .load(fixture.session.clone(), CancellationToken::new())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert!(
        loaded
            .transcripts
            .iter()
            .any(|t| t.words.iter().any(|w| w.text.trim() == "résumé"))
    );
    assert!(
        fixture
            .sql(
                "SELECT id FROM app_settings WHERE id LIKE 'capture_lifecycle_pending:%'",
                vec![]
            )
            .await
            .is_empty()
    );
    fixture.close().await;
}

impl Fixture {
    async fn new() -> Self {
        let tempdir = tempfile::tempdir().unwrap();
        let directory = tempdir.path().to_path_buf();
        let (runtime, ready) = RuntimeHandle::start(Profile {
            database: directory.join("db.sqlite"),
        })
        .unwrap();
        ready.receive().await.unwrap();
        let session = runtime
            .create_note("Meeting fixture".into())
            .unwrap()
            .receive()
            .await
            .unwrap()
            .summary
            .id;
        let transcript: Arc<str> = uuid::Uuid::new_v4().to_string().into();
        let session_copy = session.clone();
        let id = transcript.clone();
        runtime.submit(move |services| async move {
            services.executor.execute("INSERT INTO transcripts (id, session_id, started_at_ms) VALUES (?, ?, 1000)".into(),
                vec![json!(id), json!(session_copy)]).await.map_err(super::model::failure)?;
            Ok(())
        }).unwrap().receive().await.unwrap();
        Self {
            store: TranscriptStore(runtime.clone()),
            runtime,
            session,
            transcript,
            directory,
            tempdir,
        }
    }

    async fn delta(&self, id: &str, delta: LiveTranscriptDelta) {
        self.store
            .journal(self.transcript.clone(), id.to_owned().into(), delta)
            .unwrap()
            .receive()
            .await
            .unwrap();
    }

    async fn sql(&self, sql: &str, params: Vec<Value>) -> Vec<Value> {
        let sql = sql.to_owned();
        self.runtime
            .submit(move |services| async move {
                services
                    .executor
                    .execute(sql, params)
                    .await
                    .map_err(super::model::failure)
            })
            .unwrap()
            .receive()
            .await
            .unwrap()
    }

    async fn close(self) {
        self.runtime.shutdown().await.unwrap();
        self.tempdir.close().unwrap();
    }
}

fn delta(id: &str, text: &str, start_ms: i64) -> LiveTranscriptDelta {
    LiveTranscriptDelta {
        new_words: vec![FinalizedWord {
            id: id.into(),
            text: text.into(),
            start_ms,
            end_ms: start_ms + 100,
            channel: 0,
            state: WordState::Final,
            speaker_index: Some(2),
        }],
        replaced_ids: Vec::new(),
        partials: Vec::new(),
    }
}

fn word(id: &str, start_ms: i64) -> Word {
    Word {
        id: id.into(),
        text: "word".into(),
        start_ms,
        end_ms: start_ms + 100,
        channel: 0,
        extra: BTreeMap::new(),
    }
}

#[tokio::test]
async fn journal_is_idempotent_and_fold_replays_corrections() {
    let fixture = Fixture::new().await;
    fixture.delta("op1", delta("a", "first ", 0)).await;
    fixture.delta("op1", delta("a", "first ", 0)).await;
    let mut replacement = delta("b", "corrected ", 0);
    replacement.replaced_ids.push("a".into());
    fixture.delta("op2", replacement).await;
    let snapshot = fixture
        .store
        .snapshot(fixture.transcript.clone())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(snapshot.sequence, 2);
    assert_eq!(snapshot.words.len(), 1);
    assert_eq!(snapshot.words[0].text, "corrected ");
    fixture
        .store
        .fold(fixture.transcript.clone())
        .unwrap()
        .receive()
        .await
        .unwrap();
    let snapshot = fixture
        .store
        .snapshot(fixture.transcript.clone())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(snapshot.sequence, 0);
    assert_eq!(snapshot.revision, 1);
    assert_eq!(snapshot.words[0].id, "b");
    assert!(
        fixture
            .sql("SELECT * FROM transcript_live_deltas", vec![])
            .await
            .is_empty()
    );
    fixture.close().await;
}

#[tokio::test]
async fn concurrent_edits_use_word_cas_and_recovery_never_resurrects_deletions() {
    let fixture = Fixture::new().await;
    fixture
        .delta("op", delta("original", "original", 100))
        .await;
    let snapshot = fixture
        .store
        .snapshot(fixture.transcript.clone())
        .unwrap()
        .receive()
        .await
        .unwrap();
    fixture
        .store
        .edit_word(fixture.transcript.clone(), snapshot.words[0].clone(), None)
        .unwrap()
        .receive()
        .await
        .unwrap();
    let stale = fixture
        .store
        .edit_word(
            fixture.transcript.clone(),
            snapshot.words[0].clone(),
            Some("stale".into()),
        )
        .unwrap()
        .receive()
        .await;
    assert!(matches!(stale, Err(ServiceError::Conflict)));
    fixture
        .store
        .repair(
            fixture.transcript.clone(),
            snapshot.words,
            vec![word("recovered-deleted", 100), word("gap", 2000)],
            Vec::new(),
            vec![Interval {
                start: 0,
                end: 3000,
            }],
        )
        .unwrap()
        .receive()
        .await
        .unwrap();
    let snapshot = fixture
        .store
        .snapshot(fixture.transcript.clone())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(snapshot.words.len(), 1);
    assert_eq!(snapshot.words[0].id, "gap");
    fixture.close().await;
}

#[tokio::test]
async fn worker_flushes_corrections_and_runtime_shutdown_barrier() {
    let fixture = Fixture::new().await;
    let persistence = Persistence::install(&fixture.runtime, fixture.transcript.clone())
        .await
        .unwrap();
    persistence.push(delta("a", "pending", 0)).await.unwrap();
    let mut correction = delta("b", "final", 0);
    correction.replaced_ids.push("a".into());
    persistence.push(correction).await.unwrap();
    persistence.flush().await.unwrap();
    let snapshot = fixture
        .store
        .snapshot(fixture.transcript.clone())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(snapshot.words.len(), 1);
    assert_eq!(snapshot.words[0].text, "final");
    assert_eq!(snapshot.sequence, 0);
    persistence
        .push(delta("c", "shutdown", 1000))
        .await
        .unwrap();
    fixture.runtime.shutdown().await.unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: fixture.directory.join("db.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    let snapshot = TranscriptStore(runtime.clone())
        .snapshot(fixture.transcript)
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(snapshot.words.len(), 2);
    runtime.shutdown().await.unwrap();
    std::fs::remove_dir_all(fixture.directory).unwrap();
}

#[tokio::test]
async fn audio_counts_as_persisted_only_once_every_channel_finalizes_it() {
    let fixture = Fixture::new().await;
    let persistence = Persistence::install(&fixture.runtime, fixture.transcript.clone())
        .await
        .unwrap();
    let persist = async |id: &str, channel: i32, end_ms: i64| {
        let mut words = delta(id, "word", end_ms - 100);
        words.new_words[0].channel = channel;
        persistence.push(words).await.unwrap();
        persistence.flush().await.unwrap();
        persistence.persisted_through()
    };
    // Channel 1 still owes the end of the first minute while channel 0 is past it.
    assert_eq!(persist("a", 1, 55_000).await, 55_000);
    assert_eq!(persist("b", 0, 61_000).await, 55_000);
    assert_eq!(persist("c", 1, 62_000).await, 61_000);
    // A channel that went quiet doesn't hold back every later chunk.
    assert_eq!(persist("d", 0, 600_000).await, 510_000);
    fixture.close().await;
}

#[tokio::test]
async fn locked_sessions_accept_transcript_writes_only_once_revealed_and_deleted_ones_never() {
    let fixture = Fixture::new().await;
    fixture
        .sql(
            "UPDATE sessions SET locked = 1 WHERE id = ?",
            vec![json!(fixture.session)],
        )
        .await;
    let result = fixture
        .store
        .journal(
            fixture.transcript.clone(),
            "locked".into(),
            delta("a", "locked", 0),
        )
        .unwrap()
        .receive()
        .await;
    assert!(result.is_err());
    assert!(
        fixture
            .sql("SELECT * FROM transcript_live_deltas", vec![])
            .await
            .is_empty()
    );
    // Revealed for viewing, the locked note takes writes like shipping's.
    super::access::open(&fixture.session.0);
    let revealed = fixture
        .store
        .journal(
            fixture.transcript.clone(),
            "revealed".into(),
            delta("a", "revealed", 0),
        )
        .unwrap()
        .receive()
        .await;
    super::access::close(&fixture.session.0);
    assert!(revealed.is_ok());
    fixture
        .sql(
            "UPDATE sessions SET locked = 0, deleted_at = 'deleted' WHERE id = ?",
            vec![json!(fixture.session)],
        )
        .await;
    assert!(
        fixture
            .store
            .fold(fixture.transcript.clone())
            .unwrap()
            .receive()
            .await
            .is_err()
    );
    fixture.close().await;
}

#[tokio::test]
async fn speaker_creation_and_participant_assignment_commit_together() {
    let fixture = Fixture::new().await;
    fixture
        .delta("speaker-word", delta("word", "hello", 0))
        .await;
    let assignment = SpeakerAssignment {
        transcript_id: fixture.transcript.clone(),
        anchor: "word".into(),
        human_id: "new-human".into(),
        scope: SpeakerScope::Segment {
            word_ids: vec!["word".into()],
        },
    };
    fixture
        .store
        .assign_participant(
            fixture.session.clone(),
            assignment.clone(),
            None,
            Some("Zoë".into()),
        )
        .unwrap()
        .receive()
        .await
        .unwrap();
    let people = fixture.sql("SELECT h.name FROM humans h JOIN session_participants p ON p.human_id = h.id WHERE p.session_id = ?", vec![json!(fixture.session)]).await;
    assert_eq!(people[0]["name"], "Zoë");
    let loaded = fixture
        .store
        .load(fixture.session.clone(), CancellationToken::new())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(loaded.segments[0].speaker_label, "Zoë");
    fixture
        .store
        .assign_participant(fixture.session.clone(), assignment, None, None)
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(
        fixture
            .sql(
                "SELECT COUNT(*) AS n FROM session_participants WHERE session_id = ?",
                vec![json!(fixture.session)]
            )
            .await[0]["n"],
        1
    );
    let invalid = SpeakerAssignment {
        transcript_id: fixture.transcript.clone(),
        anchor: "missing".into(),
        human_id: "rejected".into(),
        scope: SpeakerScope::Segment {
            word_ids: vec!["missing".into()],
        },
    };
    assert!(
        fixture
            .store
            .assign_participant(
                fixture.session.clone(),
                invalid,
                None,
                Some("Rejected".into())
            )
            .unwrap()
            .receive()
            .await
            .is_err()
    );
    assert!(
        fixture
            .sql("SELECT id FROM humans WHERE id = 'rejected'", vec![])
            .await
            .is_empty()
    );
    fixture.close().await;
}

#[tokio::test]
async fn speaker_assignment_is_anchor_scoped_and_survives_provider_correction() {
    let fixture = Fixture::new().await;
    fixture
        .sql(
            "INSERT INTO humans (id, name) VALUES ('human', 'Alice')",
            vec![],
        )
        .await;
    fixture.delta("op", delta("a", "hello ", 0)).await;
    fixture
        .store
        .assign(SpeakerAssignment {
            transcript_id: fixture.transcript.clone(),
            anchor: "a".into(),
            human_id: "human".into(),
            scope: SpeakerScope::Segment {
                word_ids: vec!["a".into()],
            },
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    let mut correction = delta("b", "Hello ", 0);
    correction.replaced_ids.push("a".into());
    fixture.delta("correction", correction).await;
    let rendered = fixture
        .store
        .load(fixture.session.clone(), CancellationToken::new())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(rendered.segments[0].speaker_label, "Alice");
    assert_eq!(rendered.segments[0].words[0].id.as_deref(), Some("b"));
    let invalid = fixture
        .store
        .assign(SpeakerAssignment {
            transcript_id: fixture.transcript.clone(),
            anchor: "b".into(),
            human_id: "human".into(),
            scope: SpeakerScope::Segment {
                word_ids: vec!["missing".into()],
            },
        })
        .unwrap()
        .receive()
        .await;
    assert!(invalid.is_err());
    fixture.close().await;
}

#[test]
fn recovery_filters_old_current_and_out_of_gap_words() {
    let before = vec![word("deleted", 0)];
    let current = vec![word("edited", 1000)];
    let recovered = vec![
        word("new-old", 0),
        word("new-current", 1000),
        word("new-gap", 2000),
        word("outside", 4000),
    ];
    let result = recovered_additions(
        recovered,
        &before,
        &current,
        &[Interval {
            start: 0,
            end: 3000,
        }],
    );
    assert_eq!(
        result.into_iter().map(|word| word.id).collect::<Vec<_>>(),
        vec!["new-gap"]
    );
}

#[test]
fn retention_normalizes_all_shipping_and_legacy_values() {
    for (value, retention) in [
        (json!(false), Retention::Never),
        (json!(true), Retention::Forever),
        (json!("none"), Retention::Never),
        (json!("forever"), Retention::Forever),
        (json!("oneDay"), Retention::Days(1)),
        (json!("threeDays"), Retention::Days(3)),
        (json!("oneWeek"), Retention::Days(7)),
        (json!("oneMonth"), Retention::Days(30)),
    ] {
        assert_eq!(Retention::from_setting(&value).unwrap(), retention);
    }
    assert!(Retention::from_setting(&json!("unexpected")).is_err());
}

#[test]
fn bounds_reject_oversized_authoritative_words_and_bad_timestamps() {
    assert!(validate_delta(&delta("a", &"x".repeat(MAX_TEXT), 0)).is_err());
    let mut invalid = delta("a", "ok", 1);
    invalid.new_words[0].end_ms = 0;
    assert!(validate_delta(&invalid).is_err());
}

#[test]
fn correction_chain_coalesces_without_losing_replacements() {
    let first = delta("a", "one", 0);
    let mut second = delta("b", "two", 0);
    second.replaced_ids = vec!["a".into()];
    let mut third = delta("c", "three", 0);
    third.replaced_ids = vec!["b".into()];
    let result = coalesce([first, second, third]);
    assert_eq!(result.new_words.len(), 1);
    assert_eq!(result.new_words[0].id, "c");
    assert_eq!(result.replaced_ids, ["a", "b"]);
}

#[tokio::test]
async fn retention_honors_activity_and_preserves_non_audio_data() {
    let fixture = Fixture::new().await;
    let directory =
        super::retention::session_directory(&fixture.directory, &fixture.session).unwrap();
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("audio.wav"), b"audio").unwrap();
    std::fs::write(directory.join("memo.json"), b"preserve").unwrap();
    fixture.sql("INSERT INTO session_attachments (id, session_id, source_type, source_id) VALUES ('audio', ?, 'session_audio', 'primary')", vec![json!(fixture.session)]).await;
    let activities = Activities::default();
    let lease = activities.acquire(fixture.session.clone()).unwrap();
    assert!(activities.startup().is_err());
    assert!(
        super::retention::cleanup(
            &fixture.runtime,
            fixture.directory.clone(),
            activities.clone(),
            Retention::Days(1),
            i64::MAX
        )
        .await
        .unwrap()
        .is_empty()
    );
    assert!(directory.join("audio.wav").exists());
    drop(lease);
    assert_eq!(
        super::retention::cleanup(
            &fixture.runtime,
            fixture.directory.clone(),
            activities,
            Retention::Days(1),
            i64::MAX
        )
        .await
        .unwrap(),
        vec![fixture.session.clone()]
    );
    assert!(!directory.join("audio.wav").exists());
    assert_eq!(
        std::fs::read(directory.join("memo.json")).unwrap(),
        b"preserve"
    );
    assert_eq!(
        fixture
            .sql(
                "SELECT availability FROM attachment_local_state WHERE attachment_id = 'audio'",
                vec![]
            )
            .await[0]["availability"],
        "absent"
    );
    assert!(
        fixture
            .store
            .snapshot(fixture.transcript.clone())
            .unwrap()
            .receive()
            .await
            .is_ok()
    );
    fixture.close().await;
}

#[tokio::test]
async fn retention_sweep_deletes_recordings_older_than_the_chosen_policy() {
    let fixture = Fixture::new().await;
    let fresh = fixture
        .runtime
        .create_note("Fresh meeting".into())
        .unwrap()
        .receive()
        .await
        .unwrap()
        .summary
        .id;
    fixture
        .sql(
            "UPDATE sessions SET created_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-2 days') WHERE id = ?",
            vec![json!(fixture.session)],
        )
        .await;
    let audio = |session: &SessionId| {
        let directory = super::retention::session_directory(&fixture.directory, session).unwrap();
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("audio.mp3"), b"audio").unwrap();
        directory.join("audio.mp3")
    };
    let (old, new) = (audio(&fixture.session), audio(&fresh));
    for (policy, kept) in [("oneDay", [false, true]), ("none", [false, true])] {
        fixture.setting("audio_retention", json!(policy)).await;
        super::retention::sweep(
            &fixture.runtime,
            fixture.directory.clone(),
            Activities::default(),
        )
        .await
        .unwrap();
        // "Don't save" never deletes audio that hasn't been transcribed yet.
        assert_eq!([old.exists(), new.exists()], kept, "{policy}");
    }
    fixture.close().await;
}

#[tokio::test]
async fn transcription_keeps_the_main_language_with_additional_ones() {
    let fixture = Fixture::new().await;
    fixture.setting("ai_language", json!("ko")).await;
    fixture
        .setting("spoken_languages", json!("[\"en\",\"ko-KR\"]"))
        .await;
    let preferences = super::config::preferences(&fixture.runtime).await.unwrap();
    let languages = preferences.languages().unwrap();
    assert_eq!(
        languages
            .iter()
            .map(|l| l.iso639_code())
            .collect::<Vec<_>>(),
        ["ko", "en"]
    );
    fixture.close().await;
}

/// A tenth of a second of silence; recovery chunks keep their `.mp3` name, decoders probe content.
fn silent_chunk(path: &std::path::Path) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for _ in 0..3_200 {
        writer.write_sample(0i16).unwrap();
    }
    writer.finalize().unwrap();
}

#[tokio::test]
async fn dont_save_startup_recovery_transcribes_every_chunk_page_before_deleting_audio() {
    let fixture = Fixture::new().await;
    let server = provider_fixture().await;
    fixture
        .setting("current_stt_provider", json!("deepgram"))
        .await;
    fixture.setting("current_stt_model", json!("nova-3")).await;
    fixture.setting("spoken_languages", json!("[\"en\"]")).await;
    fixture
        .setting("ai_provider:stt:deepgram", json!({"base_url":server.url}))
        .await;
    let directory =
        super::retention::session_directory(&fixture.directory, &fixture.session).unwrap();
    let chunks = directory.join("audio-recovery");
    std::fs::create_dir_all(&chunks).unwrap();
    std::fs::write(directory.join(".delete-audio-on-stop"), b"").unwrap();
    // More chunks than the recorder lists in one page.
    for index in 0..130 {
        let start = index * 1_000;
        silent_chunk(&chunks.join(format!("1000-{start}-{}-{start}.mp3", start + 1_000)));
    }
    let marker = json!({"version":1, "sessionId":fixture.session, "transcriptId":fixture.transcript, "retainAudio":false, "startedAt":1000});
    fixture
        .setting(
            &format!("capture_lifecycle_pending:{}", fixture.session.0),
            marker,
        )
        .await;
    let markers = async || {
        fixture
            .sql(
                "SELECT id FROM app_settings WHERE id GLOB 'capture_lifecycle_pending:*'",
                vec![],
            )
            .await
            .len()
    };
    let offline: super::recovery::RecoveryResolver =
        Arc::new(|_, _| Box::pin(async { Err(super::model::failure("Provider is offline.")) }));
    for (resolver, recovered) in [
        (offline, false),
        (fixture.providers(&server.url).recovery_resolver(), true),
    ] {
        let reports = super::recovery::recover_startup(
            &fixture.runtime,
            fixture.directory.clone(),
            Activities::default(),
            resolver,
        )
        .await
        .unwrap();
        assert_eq!(
            reports[0].result.is_ok(),
            recovered,
            "{:?}",
            reports[0].result
        );
        // Audio survives until its speech is transcribed; the marker until recovery finishes.
        assert_eq!(chunks.exists(), !recovered);
        assert_eq!(directory.join(".delete-audio-on-stop").exists(), !recovered);
        assert_eq!(markers().await, usize::from(!recovered));
    }
    assert!(
        fixture
            .store
            .snapshot(fixture.transcript.clone())
            .unwrap()
            .receive()
            .await
            .unwrap()
            .words
            .iter()
            .any(|word| word.start_ms >= 129_000)
    );
    fixture.close().await;
}

#[tokio::test]
async fn startup_recovery_accepts_shipping_float_started_at_markers() {
    let fixture = Fixture::new().await;
    let server = provider_fixture().await;
    fixture
        .setting("current_stt_provider", json!("deepgram"))
        .await;
    fixture.setting("current_stt_model", json!("nova-3")).await;
    fixture.setting("spoken_languages", json!("[\"en\"]")).await;
    fixture
        .setting("ai_provider:stt:deepgram", json!({"base_url": server.url}))
        .await;
    let started_ms: i64 = 1_764_149_234_123;
    fixture
        .sql(
            "UPDATE transcripts SET started_at_ms = ? WHERE id = ?",
            vec![json!(started_ms), json!(fixture.transcript)],
        )
        .await;
    let directory =
        super::retention::session_directory(&fixture.directory, &fixture.session).unwrap();
    let chunks = directory.join("audio-recovery");
    std::fs::create_dir_all(&chunks).unwrap();
    silent_chunk(&chunks.join(format!("{started_ms}-0-1000-0.mp3")));
    // Shipping Tauri serializes `CaptureLifecycleMarker::started_at` and `version` via f64/JS
    // numbers, which can persist as JSON floats (`1764149234123.0`) rather than integer tokens.
    let marker = serde_json::json!({
        "version": 1.0,
        "sessionId": fixture.session,
        "transcriptId": fixture.transcript,
        "retainAudio": true,
        "startedAt": 1_764_149_234_123.0_f64,
        "inheritedCaptures": [
            {"transcriptId": "inherited-prev", "startedAt": 1_764_149_200_000.0_f64}
        ]
    });
    fixture
        .setting(
            &format!("capture_lifecycle_pending:{}", fixture.session.0),
            marker,
        )
        .await;

    let reports = super::recovery::recover_startup(
        &fixture.runtime,
        fixture.directory.clone(),
        Activities::default(),
        fixture.providers(&server.url).recovery_resolver(),
    )
    .await
    .unwrap();
    assert_eq!(reports.len(), 1);
    reports[0].result.as_ref().unwrap();

    assert!(
        fixture
            .sql(
                "SELECT id FROM app_settings WHERE id GLOB 'capture_lifecycle_pending:*'",
                vec![],
            )
            .await
            .is_empty()
    );
    let loaded = fixture
        .store
        .load(fixture.session.clone(), CancellationToken::new())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert!(
        loaded
            .transcripts
            .iter()
            .any(|t| t.words.iter().any(|w| w.text.trim() == "résumé"))
    );
    fixture.close().await;
}

#[tokio::test]
async fn startup_recovery_skips_malformed_marker_without_blocking_valid_captures() {
    let fixture = Fixture::new().await;
    let malformed_key = "capture_lifecycle_pending:0000-malformed-session";
    fixture
        .setting(
            malformed_key,
            json!({
                "version": 1,
                "sessionId": "0000-malformed-session",
                "transcriptId": "bad-transcript",
                "startedAt": "not-a-timestamp",
            }),
        )
        .await;
    fixture
        .setting(
            &format!("capture_lifecycle_pending:{}", fixture.session.0),
            json!({
                "version": 1,
                "sessionId": fixture.session,
                "transcriptId": fixture.transcript,
                "retainAudio": true,
                "startedAt": 1000,
            }),
        )
        .await;
    let resolver: super::recovery::RecoveryResolver =
        Arc::new(|_, _| Box::pin(async { Err(super::model::failure("Unused offline resolver")) }));

    let reports = super::recovery::recover_startup(
        &fixture.runtime,
        fixture.directory.clone(),
        Activities::default(),
        resolver,
    )
    .await
    .unwrap();
    assert_eq!(reports.len(), 1);
    reports[0].result.as_ref().unwrap();

    let remaining = fixture
        .sql(
            "SELECT id FROM app_settings WHERE id GLOB 'capture_lifecycle_pending:*'",
            vec![],
        )
        .await;
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0]["id"], json!(malformed_key));
    fixture.close().await;
}

#[tokio::test]
async fn startup_recovery_creates_missing_legacy_transcript_for_speech_chunk_and_rejects_tombstoned()
 {
    let fixture = Fixture::new().await;
    let server = provider_fixture().await;
    fixture
        .setting("current_stt_provider", json!("deepgram"))
        .await;
    fixture.setting("current_stt_model", json!("nova-3")).await;
    fixture.setting("spoken_languages", json!("[\"en\"]")).await;
    fixture
        .setting("ai_provider:stt:deepgram", json!({"base_url": server.url}))
        .await;

    let directory =
        super::retention::session_directory(&fixture.directory, &fixture.session).unwrap();
    let chunks = directory.join("audio-recovery");
    std::fs::create_dir_all(&chunks).unwrap();
    std::fs::write(directory.join(".delete-audio-on-stop"), b"").unwrap();

    let missing_transcript: Arc<str> = uuid::Uuid::new_v4().to_string().into();
    let started_ms: i64 = 1_764_149_234_123;
    silent_chunk(&chunks.join(format!("{started_ms}-0-1000-0.mp3")));
    let key = format!("capture_lifecycle_pending:{}", fixture.session.0);
    fixture
        .setting(
            &key,
            json!({
                "version": 1.0,
                "sessionId": fixture.session,
                "transcriptId": missing_transcript,
                "retainAudio": false,
                "startedAt": started_ms as f64,
                "createdAt": "2026-11-26T00:00:00.000Z",
                "provider": "deepgram",
                "model": "nova-3",
                "memo": "Recovered memo",
            }),
        )
        .await;

    let reports = super::recovery::recover_startup(
        &fixture.runtime,
        fixture.directory.clone(),
        Activities::default(),
        fixture.providers(&server.url).recovery_resolver(),
    )
    .await
    .unwrap();
    assert_eq!(reports.len(), 1);
    reports[0].result.as_ref().unwrap();

    let created = fixture
        .sql(
            "SELECT ended_at_ms, provider, model, memo, created_at FROM transcripts WHERE id = ? AND session_id = ? AND deleted_at IS NULL",
            vec![json!(missing_transcript), json!(fixture.session)],
        )
        .await;
    assert_eq!(created.len(), 1);
    assert!(created[0]["ended_at_ms"].as_i64().is_some());
    assert_eq!(created[0]["provider"], json!("deepgram"));
    assert_eq!(created[0]["model"], json!("nova-3"));
    assert_eq!(created[0]["memo"], json!("Recovered memo"));
    assert_eq!(created[0]["created_at"], json!("2026-11-26T00:00:00.000Z"));
    let loaded = fixture
        .store
        .load(fixture.session.clone(), CancellationToken::new())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert!(
        loaded.transcripts.iter().any(
            |t| t.id == missing_transcript && t.words.iter().any(|w| w.text.trim() == "résumé")
        )
    );
    assert!(!chunks.exists());
    assert!(
        fixture
            .sql(
                "SELECT id FROM app_settings WHERE id GLOB 'capture_lifecycle_pending:*'",
                vec![]
            )
            .await
            .is_empty()
    );

    // Tombstoned transcript rows must not be resurrected, and discard cleans up without a row.
    let deleted_transcript: Arc<str> = uuid::Uuid::new_v4().to_string().into();
    fixture
        .sql(
            "INSERT INTO transcripts (id, session_id, started_at_ms, deleted_at) VALUES (?, ?, ?, '2026-11-26T01:00:00.000Z')",
            vec![json!(deleted_transcript), json!(fixture.session), json!(started_ms)],
        )
        .await;
    std::fs::create_dir_all(&chunks).unwrap();
    silent_chunk(&chunks.join(format!("{started_ms}-0-1000-0.mp3")));
    fixture
        .setting(
            &key,
            json!({
                "version": 1,
                "sessionId": fixture.session,
                "transcriptId": deleted_transcript,
                "retainAudio": false,
                "startedAt": started_ms,
            }),
        )
        .await;
    let tombstoned_reports = super::recovery::recover_startup(
        &fixture.runtime,
        fixture.directory.clone(),
        Activities::default(),
        fixture.providers(&server.url).recovery_resolver(),
    )
    .await
    .unwrap();
    assert_eq!(tombstoned_reports.len(), 1);
    assert!(tombstoned_reports[0].result.is_err());

    let absent_transcript: Arc<str> = uuid::Uuid::new_v4().to_string().into();
    fixture
        .setting(
            &key,
            json!({
                "version": 1,
                "sessionId": fixture.session,
                "transcriptId": absent_transcript,
                "retainAudio": false,
                "startedAt": started_ms,
            }),
        )
        .await;
    super::recovery::discard(
        &fixture.runtime,
        &fixture.directory,
        fixture.session.clone(),
        absent_transcript,
        false,
        Activities::default(),
    )
    .await
    .unwrap();
    assert!(!chunks.exists());
    assert!(
        fixture
            .sql(
                "SELECT id FROM app_settings WHERE id GLOB 'capture_lifecycle_pending:*'",
                vec![]
            )
            .await
            .is_empty()
    );
    fixture.close().await;
}

#[tokio::test]
async fn startup_recovery_routes_inherited_chunks_and_does_not_block_newer_capture() {
    let fixture = Fixture::new().await;
    let server = provider_fixture().await;
    fixture
        .setting("current_stt_provider", json!("deepgram"))
        .await;
    fixture.setting("current_stt_model", json!("nova-3")).await;
    fixture.setting("spoken_languages", json!("[\"en\"]")).await;
    fixture
        .setting("ai_provider:stt:deepgram", json!({"base_url": server.url}))
        .await;

    let directory =
        super::retention::session_directory(&fixture.directory, &fixture.session).unwrap();
    let chunks = directory.join("audio-recovery");
    std::fs::create_dir_all(&chunks).unwrap();

    let inherited_started: i64 = 1_764_149_100_000;
    let primary_started: i64 = 1_764_149_200_000;
    let deleted_inherited: Arc<str> = uuid::Uuid::new_v4().to_string().into();
    fixture
        .sql(
            "INSERT INTO transcripts (id, session_id, started_at_ms, deleted_at) VALUES (?, ?, ?, '2026-11-26T01:00:00.000Z')",
            vec![json!(deleted_inherited), json!(fixture.session), json!(inherited_started)],
        )
        .await;
    fixture
        .sql(
            "UPDATE transcripts SET started_at_ms = ? WHERE id = ?",
            vec![json!(primary_started), json!(fixture.transcript)],
        )
        .await;

    let inherited_chunk = chunks.join(format!("{inherited_started}-0-1000-0.mp3"));
    let primary_chunk = chunks.join(format!("{primary_started}-0-1000-0.mp3"));
    silent_chunk(&inherited_chunk);
    silent_chunk(&primary_chunk);
    let key = format!("capture_lifecycle_pending:{}", fixture.session.0);
    fixture
        .setting(
            &key,
            json!({
                "version": 1.0,
                "sessionId": fixture.session,
                "transcriptId": fixture.transcript,
                "retainAudio": true,
                "startedAt": primary_started as f64,
                "inheritedCaptures": [
                    {
                        "transcriptId": deleted_inherited,
                        "startedAt": inherited_started as f64,
                    }
                ],
            }),
        )
        .await;

    // Inherited repair failure must not prevent the newer recording's chunk from recovering.
    let partial_reports = super::recovery::recover_startup(
        &fixture.runtime,
        fixture.directory.clone(),
        Activities::default(),
        fixture.providers(&server.url).recovery_resolver(),
    )
    .await
    .unwrap();
    assert_eq!(partial_reports.len(), 1);
    assert!(partial_reports[0].result.is_err());
    assert!(inherited_chunk.exists());
    assert!(!primary_chunk.exists());
    let primary_snapshot = fixture
        .store
        .snapshot(fixture.transcript.clone())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert!(
        primary_snapshot
            .words
            .iter()
            .any(|word| word.text.trim() == "résumé")
    );
    assert_eq!(
        fixture
            .sql(
                "SELECT id FROM app_settings WHERE id GLOB 'capture_lifecycle_pending:*'",
                vec![]
            )
            .await
            .len(),
        1
    );

    // When both inherited and newer targets are valid, both transcripts recover and finalize.
    let valid_inherited: Arc<str> = uuid::Uuid::new_v4().to_string().into();
    let second_primary_chunk = chunks.join(format!("{primary_started}-1000-2000-1000.mp3"));
    silent_chunk(&second_primary_chunk);
    fixture
        .setting(
            &key,
            json!({
                "version": 1.0,
                "sessionId": fixture.session,
                "transcriptId": fixture.transcript,
                "retainAudio": true,
                "startedAt": primary_started as f64,
                "inheritedCaptures": [
                    {
                        "transcriptId": valid_inherited,
                        "startedAt": inherited_started as f64,
                        "provider": "deepgram",
                        "model": "nova-3",
                    }
                ],
            }),
        )
        .await;

    let reports = super::recovery::recover_startup(
        &fixture.runtime,
        fixture.directory.clone(),
        Activities::default(),
        fixture.providers(&server.url).recovery_resolver(),
    )
    .await
    .unwrap();
    assert_eq!(reports.len(), 1);
    reports[0].result.as_ref().unwrap();
    assert!(!inherited_chunk.exists());
    assert!(!second_primary_chunk.exists());
    let inherited_snapshot = fixture
        .store
        .snapshot(valid_inherited)
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert!(
        inherited_snapshot
            .words
            .iter()
            .any(|word| word.text.trim() == "résumé")
    );
    assert!(
        fixture
            .sql(
                "SELECT id FROM app_settings WHERE id GLOB 'capture_lifecycle_pending:*'",
                vec![]
            )
            .await
            .is_empty()
    );
    fixture.close().await;
}

#[tokio::test]
async fn retained_startup_recovery_keeps_unreadable_partial_audio_until_discarded() {
    let fixture = Fixture::new().await;
    let directory =
        super::retention::session_directory(&fixture.directory, &fixture.session).unwrap();
    std::fs::create_dir_all(directory.join("audio-recovery")).unwrap();
    let key = format!("capture_lifecycle_pending:{}", fixture.session.0);
    let marker = json!({"version":1, "sessionId":fixture.session, "transcriptId":fixture.transcript, "retainAudio":true, "startedAt":1000});
    let markers = async || {
        fixture
            .sql(
                "SELECT id FROM app_settings WHERE id GLOB 'capture_lifecycle_pending:*'",
                vec![],
            )
            .await
            .len()
    };
    let resolver: super::recovery::RecoveryResolver =
        Arc::new(|_, _| Box::pin(async { Err(super::model::failure("Nothing to transcribe.")) }));
    let recover = async || {
        super::recovery::recover_startup(
            &fixture.runtime,
            fixture.directory.clone(),
            Activities::default(),
            resolver.clone(),
        )
        .await
        .unwrap()
        .remove(0)
    };
    // An empty chunk page with nothing left over is a finished recovery.
    fixture.setting(&key, marker.clone()).await;
    recover().await.result.unwrap();
    assert_eq!(markers().await, 0);
    // A partial chunk the recorder couldn't decode is not.
    let partial = directory.join("audio-recovery").join("1000-0-0.part");
    std::fs::write(&partial, b"not audio").unwrap();
    fixture.setting(&key, marker).await;
    let interrupted = recover().await;
    assert!(interrupted.result.is_err());
    assert_eq!(markers().await, 1);
    assert!(partial.exists());
    super::recovery::discard(
        &fixture.runtime,
        &fixture.directory,
        interrupted.session,
        interrupted.transcript,
        interrupted.retained,
        Activities::default(),
    )
    .await
    .unwrap();
    assert_eq!(markers().await, 0);
    assert!(!partial.exists());
    fixture.close().await;
}

#[tokio::test]
async fn cleanup_preserves_audio_owned_by_durable_finalization_marker() {
    let fixture = Fixture::new().await;
    let directory =
        super::retention::session_directory(&fixture.directory, &fixture.session).unwrap();
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("audio.wav"), b"unfinished").unwrap();
    fixture
        .sql(
            "INSERT INTO app_settings (id, value_json) VALUES (?, '{}')",
            vec![json!(format!(
                "capture_lifecycle_pending:{}",
                fixture.session.0
            ))],
        )
        .await;
    assert!(
        super::retention::cleanup(
            &fixture.runtime,
            fixture.directory.clone(),
            Activities::default(),
            Retention::Never,
            i64::MAX
        )
        .await
        .unwrap()
        .is_empty()
    );
    assert!(directory.join("audio.wav").exists());
    fixture.close().await;
}

#[test]
fn activity_exclusion_and_path_provenance() {
    let activities = Activities::default();
    let startup = activities.startup().unwrap();
    assert!(activities.acquire(SessionId("session".into())).is_err());
    drop(startup);
    assert!(activities.acquire(SessionId("session".into())).is_ok());
    for unsafe_id in ["", "..", "../another", "/outside", "x\\y"] {
        assert!(
            super::retention::session_directory(
                std::path::Path::new("/vault"),
                &SessionId(unsafe_id.into())
            )
            .is_err()
        );
    }
}

struct FixtureProvider(Arc<AtomicBool>);

impl ProviderAdapter for FixtureProvider {
    fn stream(
        &self,
        _: Request,
    ) -> BoxFuture<
        'static,
        desktop_runtime::Result<BoxStream<'static, desktop_runtime::Result<ProviderEvent>>>,
    > {
        let fail = self.0.load(Ordering::Acquire);
        Box::pin(async move {
            if fail {
                return Err(super::model::failure("Fixture provider failure"));
            }
            Ok(futures::stream::iter([
                Ok(ProviderEvent::Text("answer ".into())),
                Ok(ProviderEvent::Text("completed".into())),
            ])
            .boxed())
        })
    }
}

#[tokio::test]
async fn chat_repairs_user_row_rejects_orphans_and_preserves_failed_regeneration() {
    let fixture = Fixture::new().await;
    fixture
        .sql("INSERT INTO chat_groups (id) VALUES ('group')", vec![])
        .await;
    let fail = Arc::new(AtomicBool::new(false));
    let provider: Arc<dyn ProviderAdapter> = Arc::new(FixtureProvider(fail.clone()));
    let ai = Arc::new(
        AiServices::new(
            fixture.runtime.clone(),
            Arc::new(move |_| {
                let provider = provider.clone();
                Box::pin(async move { Ok(provider) })
            }),
            Arc::new(|_, _, _| Box::pin(async { Err(super::model::failure("No fixture tools")) })),
            Arc::new(|_| false),
        )
        .unwrap(),
    );
    let chat = ai.chat(fixture.session.clone(), "group".into()).unwrap();
    assert!(Arc::ptr_eq(
        &chat,
        &ai.chat(fixture.session.clone(), "group".into()).unwrap()
    ));
    let user = Message {
        id: "user".into(),
        role: Role::User,
        parts: vec![Part::Text {
            text: "question".into(),
        }],
    };
    let assistant = chat
        .queue_send(user.clone(), Vec::new(), None, Arc::new(|_| {}))
        .unwrap()
        .await
        .unwrap()
        .unwrap();
    let rows = fixture.sql("SELECT role, content FROM chat_messages WHERE deleted_at IS NULL ORDER BY created_at, id", vec![]).await;
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|row| row["role"] == "user"));
    assert!(rows.iter().any(|row| row["content"] == "answer completed"));
    fail.store(true, Ordering::Release);
    assert!(
        chat.queue_send(
            user.clone(),
            Vec::new(),
            Some(assistant.id.clone()),
            Arc::new(|_| {})
        )
        .unwrap()
        .await
        .unwrap()
        .is_err()
    );
    assert_eq!(
        fixture
            .sql(
                "SELECT id FROM chat_messages WHERE id = ? AND deleted_at IS NULL",
                vec![json!(assistant.id)]
            )
            .await
            .len(),
        1
    );
    fixture
        .sql(
            "UPDATE chat_groups SET deleted_at = 'deleted' WHERE id = 'group'",
            vec![],
        )
        .await;
    assert!(
        chat.queue_send(user, Vec::new(), None, Arc::new(|_| {}))
            .unwrap()
            .await
            .unwrap()
            .is_err()
    );
    chat.flush().await;
    fixture.close().await;
}

#[test]
fn history_window_retains_the_user_that_started_the_tool_chain() {
    let history = (0..30)
        .map(|index| Message {
            id: index.to_string(),
            role: if index % 3 == 0 {
                Role::User
            } else {
                Role::Assistant
            },
            parts: vec![],
        })
        .collect();
    let window = super::ai::window_history(history);
    assert_eq!(window[0].id, "9");
    assert_eq!(window.last().unwrap().id, "29");
}

#[tokio::test]
async fn provider_corrections_preserve_unknown_word_metadata() {
    let fixture = Fixture::new().await;
    let mut original = word("word", 0);
    original
        .extra
        .insert("future_metadata".into(), json!({"nested": [1, 2]}));
    fixture
        .sql(
            "UPDATE transcripts SET words_json = ? WHERE id = ?",
            vec![
                json!(serde_json::to_string(&vec![original]).unwrap()),
                json!(fixture.transcript),
            ],
        )
        .await;
    fixture
        .delta("correction", delta("word", "corrected", 0))
        .await;
    fixture
        .store
        .fold(fixture.transcript.clone())
        .unwrap()
        .receive()
        .await
        .unwrap();
    let snapshot = fixture
        .store
        .snapshot(fixture.transcript.clone())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(snapshot.words[0].text, "corrected");
    assert_eq!(
        snapshot.words[0].extra["future_metadata"],
        json!({"nested": [1, 2]})
    );
    fixture.close().await;
}

fn summary_context() -> super::ai::SummaryContext {
    super::ai::SummaryContext {
        system: anlg_template_app::EnhanceSystem {
            language: Some("en".into()),
            format_override: "Use headings and preserve decisions.".into(),
        },
        user: anlg_template_app::EnhanceUser {
            session: anlg_template_app::Session {
                title: Some("Fixture".into()),
                started_at: None,
                ended_at: None,
                event: None,
            },
            participants: Vec::new(),
            template: None,
            transcripts: Vec::new(),
            pre_meeting_memo: "Initial note".into(),
            post_meeting_memo: String::new(),
        },
    }
}

#[tokio::test]
async fn retired_capture_workers_do_not_exhaust_runtime_flush_slots() {
    let fixture = Fixture::new().await;
    let group = super::persistence::PersistenceGroup::install(&fixture.runtime)
        .await
        .unwrap();
    for _ in 0..70 {
        let worker = group
            .transcript(&fixture.runtime, fixture.transcript.clone())
            .await
            .unwrap();
        worker.retire().await.unwrap();
        assert!(matches!(
            worker.push(delta("late", "late", 0)).await,
            Err(ServiceError::Closed)
        ));
    }
    fixture.close().await;
}

#[tokio::test]
async fn summaries_reject_notes_active_capture_and_stale_targets() {
    let fixture = Fixture::new().await;
    let base = fixture
        .runtime
        .open_session(fixture.session.clone(), CancellationToken::new())
        .unwrap()
        .receive()
        .await
        .unwrap()
        .note
        .unwrap();
    let active = Arc::new(AtomicBool::new(false));
    let recording = active.clone();
    let provider: Arc<dyn ProviderAdapter> =
        Arc::new(FixtureProvider(Arc::new(AtomicBool::new(false))));
    let services = AiServices::new(
        fixture.runtime.clone(),
        Arc::new(move |_| {
            let provider = provider.clone();
            Box::pin(async move { Ok(provider) })
        }),
        Arc::new(|_, _, _| Box::pin(async { Ok(json!({})) })),
        Arc::new(move |_| recording.load(Ordering::Acquire)),
    )
    .unwrap();
    assert!(matches!(
        services
            .summarize(
                base.clone(),
                summary_context(),
                CancellationToken::new(),
                Arc::new(|_| {})
            )
            .await,
        Err(ServiceError::Conflict)
    ));
    fixture
        .sql(
            "UPDATE session_documents SET kind = 'summary' WHERE id = ?",
            vec![json!(base.id)],
        )
        .await;
    active.store(true, Ordering::Release);
    assert!(matches!(
        services
            .summarize(
                base.clone(),
                summary_context(),
                CancellationToken::new(),
                Arc::new(|_| {})
            )
            .await,
        Err(ServiceError::Busy)
    ));
    active.store(false, Ordering::Release);
    let saved = services
        .summarize(
            base.clone(),
            summary_context(),
            CancellationToken::new(),
            Arc::new(|_| {}),
        )
        .await
        .unwrap();
    assert!(saved.body.contains("answer completed"));
    assert!(matches!(
        services
            .summarize(
                base,
                summary_context(),
                CancellationToken::new(),
                Arc::new(|_| {})
            )
            .await,
        Err(ServiceError::Conflict)
    ));
    fixture.close().await;
}

struct ToolProvider;
impl ProviderAdapter for ToolProvider {
    fn stream(
        &self,
        request: Request,
    ) -> BoxFuture<
        'static,
        desktop_runtime::Result<BoxStream<'static, desktop_runtime::Result<ProviderEvent>>>,
    > {
        Box::pin(async move {
            let event = if request.tools_allowed {
                ProviderEvent::Tool {
                    call_id: uuid::Uuid::new_v4().to_string(),
                    name: "get_meeting".into(),
                    input: json!({"meeting_id": request.session.0}),
                }
            } else {
                ProviderEvent::Text("Final report".into())
            };
            Ok(futures::stream::iter([Ok(event)]).boxed())
        })
    }
}

#[tokio::test]
async fn tool_loop_executes_five_typed_calls_then_a_final_report() {
    let fixture = Fixture::new().await;
    fixture
        .sql("INSERT INTO chat_groups (id) VALUES ('tools')", vec![])
        .await;
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = calls.clone();
    let ai = Arc::new(
        AiServices::new(
            fixture.runtime.clone(),
            Arc::new(|_| {
                Box::pin(async { Ok(Arc::new(ToolProvider) as Arc<dyn ProviderAdapter>) })
            }),
            Arc::new(move |tool, _, _| {
                assert!(matches!(tool, super::ai::MeetingTool::GetMeeting(_)));
                observed.fetch_add(1, Ordering::AcqRel);
                Box::pin(async { Ok(json!({"title": "Meeting fixture"})) })
            }),
            Arc::new(|_| false),
        )
        .unwrap(),
    );
    let chat = ai.chat(fixture.session.clone(), "tools".into()).unwrap();
    let answer = chat
        .queue_send(
            Message {
                id: "question".into(),
                role: Role::User,
                parts: vec![Part::Text {
                    text: "Find this meeting".into(),
                }],
            },
            Vec::new(),
            None,
            Arc::new(|_| {}),
        )
        .unwrap()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(calls.load(Ordering::Acquire), 5);
    assert!(matches!(answer.parts.last(), Some(Part::Text { text }) if text == "Final report"));
    assert_eq!(
        answer
            .parts
            .iter()
            .filter(|part| matches!(part, Part::Tool { .. }))
            .count(),
        5
    );
    // Stored like shipping's typed tool parts, so both apps render the saved reply.
    let stored = fixture
        .sql(
            "SELECT parts_json FROM chat_messages WHERE chat_group_id = 'tools' AND role = 'assistant'",
            vec![],
        )
        .await;
    let parts: Vec<Value> =
        serde_json::from_str(stored[0]["parts_json"].as_str().unwrap()).unwrap();
    assert_eq!(parts[0]["type"], "tool-get_meeting");
    assert!(parts[0].get("toolName").is_none());
    fixture.close().await;
}

#[tokio::test]
async fn cancelling_queued_chat_prevents_provider_calls_and_persistence() {
    let fixture = Fixture::new().await;
    fixture
        .sql("INSERT INTO chat_groups (id) VALUES ('cancelled')", vec![])
        .await;
    let (release, blocked) = tokio::sync::oneshot::channel();
    let ai = Arc::new(
        AiServices::new(
            fixture.runtime.clone(),
            Arc::new(|_| {
                Box::pin(async { panic!("Cancelled queued work must never request a provider") })
            }),
            Arc::new(|_, _, _| Box::pin(async { Ok(json!({})) })),
            Arc::new(|_| false),
        )
        .unwrap(),
    );
    let blocker = ai
        .enqueue(Box::pin(async move {
            blocked.await.map_err(super::model::failure)
        }))
        .unwrap();
    let chat = ai
        .chat(fixture.session.clone(), "cancelled".into())
        .unwrap();
    let answer = chat
        .queue_send(
            Message {
                id: "cancelled".into(),
                role: Role::User,
                parts: vec![Part::Text {
                    text: "queued".into(),
                }],
            },
            Vec::new(),
            None,
            Arc::new(|_| {}),
        )
        .unwrap();
    chat.cancel().unwrap();
    release.send(()).unwrap();
    blocker.await.unwrap().unwrap();
    assert!(matches!(
        answer.await.unwrap(),
        Err(ServiceError::Cancelled)
    ));
    assert!(
        fixture
            .sql("SELECT * FROM chat_messages", vec![])
            .await
            .is_empty()
    );
    ai.flush().await.unwrap();
    fixture.close().await;
}

#[test]
fn stereo_waveform_is_normalized_and_bounded_for_long_input() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("stereo.wav");
    let frames = 16_000 * 45;
    let data_length = frames * 4;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36_u32 + data_length).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_000_u32.to_le_bytes());
    bytes.extend_from_slice(&64_000_u32.to_le_bytes());
    bytes.extend_from_slice(&4_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_length.to_le_bytes());
    for _ in 0..frames {
        bytes.extend_from_slice(&8192_i16.to_le_bytes());
        bytes.extend_from_slice(&16384_i16.to_le_bytes());
    }
    std::fs::write(&path, bytes).unwrap();
    let (peaks, duration) = super::playback::decode_waveform(&path).unwrap();
    assert!(peaks.len() <= 4096);
    assert_eq!(duration.as_secs(), 45);
    assert!(peaks.iter().all(|peak| peak[0] == 0.5 && peak[1] == 1.0));
}

struct FixtureStorage(PathBuf);

#[test]
fn recovery_chunks_align_resumed_capture_and_overlap_offsets() {
    let chunk = anlg_listener_core::actors::recorder::RecoveryAudioChunk {
        id: "chunk".into(),
        path: "unused.mp3".into(),
        capture_started_at: 105_000,
        start_ms: 60_000,
        audio_start_ms: 58_000,
        end_ms: 120_000,
    };
    assert_eq!(
        super::recovery::chunk_interval(&chunk, 100_000),
        Interval {
            start: 65_000,
            end: 125_000
        }
    );
    assert_eq!(
        chunk.capture_started_at as i64 - 100_000 + chunk.audio_start_ms as i64,
        63_000
    );
}

impl anlg_storage::StorageRuntime for FixtureStorage {
    fn global_base(&self) -> std::result::Result<PathBuf, anlg_storage::Error> {
        Ok(self.0.clone())
    }
    fn vault_base(&self) -> std::result::Result<PathBuf, anlg_storage::Error> {
        Ok(self.0.clone())
    }
}

#[tokio::test]
async fn native_capture_uses_fixture_audio_and_keeps_unresolved_batch_marker() {
    let fixture = Fixture::new().await;
    let capture = super::capture::CaptureService::spawn(
        fixture.runtime.clone(),
        Arc::new(anlg_audio_mock::MockAudio::new(1)),
        Arc::new(FixtureStorage(fixture.directory.clone())),
        Arc::new(|session| {
            Box::pin(async move {
                Ok(super::capture::CaptureConfig {
                    params: anlg_listener_core::actors::SessionParams {
                        session_id: session.0.to_string(),
                        retain_audio: Some(true),
                        languages: Vec::new(),
                        onboarding: false,
                        transcription_mode: anlg_listener_core::TranscriptionMode::Batch,
                        model: "fixture-batch".into(),
                        base_url: "http://127.0.0.1:1".into(),
                        api_key: String::new(),
                        keywords: Vec::new(),
                        mic_device: None,
                        participant_human_ids: Vec::new(),
                        self_human_id: None,
                        speaker_assignments: Vec::new(),
                        capture_started_at_ms: None,
                    },
                    provider: "fixture".into(),
                    retention: Retention::Forever,
                    memo: String::new(),
                    recovery: None,
                })
            })
        }),
    )
    .unwrap();
    let devices = capture.devices().unwrap().await.unwrap().unwrap();
    assert_eq!(devices.default, "mock-mic");
    assert!(devices.microphones.contains(&"mock-mic".to_owned()));
    capture.microphone(None).unwrap().await.unwrap().unwrap();
    capture
        .start(fixture.session.clone())
        .unwrap()
        .await
        .unwrap()
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        while capture.take_update(0).phase != super::capture::Phase::Listening {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let marker = fixture
        .sql(
            "SELECT value_json FROM app_settings WHERE id = ?",
            vec![json!(format!(
                "capture_lifecycle_pending:{}",
                fixture.session.0
            ))],
        )
        .await;
    assert_eq!(marker.len(), 1);
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    capture.stop().unwrap().await.unwrap().unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        while capture.take_update(0).phase != super::capture::Phase::Failed {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        capture
            .take_update(0)
            .error
            .unwrap()
            .to_string()
            .contains("incomplete")
    );
    assert_eq!(
        fixture
            .sql(
                "SELECT value_json FROM app_settings WHERE id = ?",
                vec![json!(format!(
                    "capture_lifecycle_pending:{}",
                    fixture.session.0
                ))]
            )
            .await
            .len(),
        1
    );
    drop(capture);
    fixture.close().await;
}

#[cfg(unix)]
#[test]
fn retention_refuses_symlinked_session_directories() {
    let vault = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let sessions = vault.path().join("sessions");
    std::fs::create_dir(&sessions).unwrap();
    std::os::unix::fs::symlink(outside.path(), sessions.join("meeting")).unwrap();
    assert!(
        super::retention::session_directory(vault.path(), &SessionId("meeting".into())).is_err()
    );
}

#[test]
fn playback_centers_both_recording_channels_without_clipping() {
    let source = rodio::buffer::SamplesBuffer::new(
        std::num::NonZeroU16::new(2).unwrap(),
        std::num::NonZeroU32::new(16_000).unwrap(),
        vec![1., 0., 0., 1., 0.8, 0.2, 1., 1.],
    );
    let centered = super::playback::Centered(source);
    assert_eq!(anlg_audio_utils::Source::channels(&centered).get(), 1);
    assert_eq!(centered.collect::<Vec<_>>(), vec![0.5, 0.5, 0.5, 1.]);
}

#[tokio::test]
async fn on_device_and_cloud_stt_and_llm_providers_resolve_across_modes() {
    let fixture = Fixture::new().await;
    let server = provider_fixture().await;
    fixture.setting("ai_language", json!("en")).await;
    let paid = Arc::new(AtomicBool::new(true));
    let has_secret = Arc::new(AtomicBool::new(true));
    let local_calls: Arc<std::sync::Mutex<Vec<(String, bool)>>> = Arc::default();

    let mut services = fixture.providers(&server.url);
    {
        let paid = paid.clone();
        services.cloud = Arc::new(move || {
            let is_paid = paid.load(Ordering::SeqCst);
            Box::pin(async move {
                Ok(super::config::CloudAccess {
                    access_token: "paid-token".into(),
                    user_id: "owner".into(),
                    is_paid,
                })
            })
        });
    }
    {
        let local_calls = local_calls.clone();
        services.local = Arc::new(move |request| {
            local_calls
                .lock()
                .unwrap()
                .push((request.model.clone(), request.batch));
            let endpoint = if request.model == "apple-speech" {
                anlg_transcribe_speechanalyzer::LOCAL_BASE_URL
            } else {
                anlg_transcribe_soniqo::LOCAL_BASE_URL
            };
            Box::pin(async move { Ok(endpoint.into()) })
        });
    }
    {
        let has_secret = has_secret.clone();
        services.secret = Arc::new(move |_, _| {
            let present = has_secret.load(Ordering::SeqCst);
            Box::pin(async move { Ok(present.then(|| "byok-key".into())) })
        });
    }

    // 1. Anarlog Pro Cloud STT
    fixture
        .setting("current_stt_provider", json!("anarlog"))
        .await;
    fixture.setting("current_stt_model", json!("cloud")).await;
    let capture = services.capture(fixture.session.clone()).await.unwrap();
    assert_eq!(capture.params.base_url, format!("{}/stt", server.url));
    assert_eq!(capture.params.api_key, "paid-token");
    assert_eq!(
        capture.params.effective_transcription_mode(),
        anlg_listener_core::TranscriptionMode::Live
    );
    let batch = services
        .batch(fixture.session.clone(), PathBuf::from("meeting.wav"))
        .await
        .unwrap();
    assert!(matches!(
        batch.provider,
        anlg_listener2_core::BatchProvider::Anarlog
    ));

    // 2. BYOK Cloud STT
    fixture
        .setting("current_stt_provider", json!("openai"))
        .await;
    fixture
        .setting("current_stt_model", json!("gpt-live-transcribe"))
        .await;
    let openai = services.capture(fixture.session.clone()).await.unwrap();
    assert_eq!(openai.params.base_url, "https://api.openai.com/v1");
    assert_eq!(openai.params.api_key, "byok-key");
    assert_eq!(
        openai.params.effective_transcription_mode(),
        anlg_listener_core::TranscriptionMode::Live
    );
    let openai_batch = services
        .batch(fixture.session.clone(), PathBuf::from("meeting.wav"))
        .await
        .unwrap();
    assert!(matches!(
        openai_batch.provider,
        anlg_listener2_core::BatchProvider::OpenAI
    ));

    // 3. On-device Soniqo streaming (both explicit provider and legacy "anarlog" provider)
    let expected_soniqo_live = if anlg_transcribe_soniqo::SoniqoModel::ParakeetStreaming
        .supports_live_on_current_platform()
    {
        anlg_listener_core::TranscriptionMode::Live
    } else {
        anlg_listener_core::TranscriptionMode::Batch
    };
    for provider in ["soniqo", "anarlog"] {
        fixture
            .setting("current_stt_provider", json!(provider))
            .await;
        fixture
            .setting("current_stt_model", json!("soniqo-parakeet-streaming"))
            .await;
        let soniqo = services.capture(fixture.session.clone()).await.unwrap();
        assert_eq!(
            soniqo.params.base_url,
            anlg_transcribe_soniqo::LOCAL_BASE_URL
        );
        assert_eq!(
            soniqo.params.effective_transcription_mode(),
            expected_soniqo_live
        );
        let soniqo_batch = services
            .batch(fixture.session.clone(), PathBuf::from("meeting.wav"))
            .await
            .unwrap();
        assert!(matches!(
            soniqo_batch.provider,
            anlg_listener2_core::BatchProvider::Soniqo
        ));
    }

    // 4. On-device Soniqo batch (capture uses batch = false, forces Batch mode)
    fixture
        .setting("current_stt_provider", json!("soniqo"))
        .await;
    fixture
        .setting("current_stt_model", json!("soniqo-parakeet-batch"))
        .await;
    let soniqo_batch_capture = services.capture(fixture.session.clone()).await.unwrap();
    assert_eq!(
        soniqo_batch_capture.params.base_url,
        anlg_transcribe_soniqo::LOCAL_BASE_URL
    );
    assert_eq!(
        soniqo_batch_capture.params.effective_transcription_mode(),
        anlg_listener_core::TranscriptionMode::Batch
    );
    let soniqo_batch_recovery = services
        .batch(fixture.session.clone(), PathBuf::from("meeting.wav"))
        .await
        .unwrap();
    assert!(matches!(
        soniqo_batch_recovery.provider,
        anlg_listener2_core::BatchProvider::Soniqo
    ));

    // 5. On-device Apple Speech (explicit and legacy)
    for provider in ["apple_speech", "anarlog"] {
        fixture
            .setting("current_stt_provider", json!(provider))
            .await;
        fixture
            .setting("current_stt_model", json!("apple-speech"))
            .await;
        let apple = services.capture(fixture.session.clone()).await.unwrap();
        assert_eq!(
            apple.params.base_url,
            anlg_transcribe_speechanalyzer::LOCAL_BASE_URL
        );
        let apple_batch = services
            .batch(fixture.session.clone(), PathBuf::from("meeting.wav"))
            .await
            .unwrap();
        assert!(matches!(
            apple_batch.provider,
            anlg_listener2_core::BatchProvider::AppleSpeech
        ));
    }
    assert!(
        local_calls
            .lock()
            .unwrap()
            .contains(&("soniqo-parakeet-batch".to_owned(), false))
    );

    async fn read_completion(
        resolver: &super::ai::ProviderResolver,
        session: &SessionId,
    ) -> String {
        let adapter = resolver(session.clone()).await.unwrap();
        let mut stream = adapter
            .stream(Request {
                session: session.clone(),
                messages: vec![Message {
                    id: "u1".into(),
                    role: Role::User,
                    parts: vec![Part::Text {
                        text: "Summarize".into(),
                    }],
                }],
                tools_allowed: false,
                structured: None,
                cancellation: CancellationToken::new(),
            })
            .await
            .unwrap();
        let mut streamed = String::new();
        while let Some(event) = stream.next().await {
            if let ProviderEvent::Text(text) = event.unwrap() {
                streamed.push_str(&text);
            }
        }
        streamed
    }

    // 6. Anarlog Pro Cloud LLM
    fixture
        .setting("current_llm_provider", json!("anarlog"))
        .await;
    fixture.setting("current_llm_model", json!("Auto")).await;
    let resolver = services.ai_resolver();
    assert_eq!(
        read_completion(&resolver, &fixture.session).await,
        "fixture résumé"
    );
    paid.store(false, Ordering::SeqCst);
    assert!(resolver(fixture.session.clone()).await.is_err());

    // 7. Local Ollama & LM Studio (no API key required)
    has_secret.store(false, Ordering::SeqCst);
    fixture
        .setting("current_llm_provider", json!("lmstudio"))
        .await;
    fixture.setting("current_llm_model", json!("qwen2.5")).await;
    let lmstudio_prefs = super::config::preferences(&fixture.runtime).await.unwrap();
    let lmstudio = services
        .connection(&lmstudio_prefs, super::config::ProviderKind::Llm, false)
        .await
        .unwrap();
    assert_eq!(lmstudio.base_url, "http://127.0.0.1:1234/v1");

    fixture
        .setting("current_llm_provider", json!("ollama"))
        .await;
    fixture
        .setting("current_llm_model", json!("llama3.2"))
        .await;
    let ollama_prefs = super::config::preferences(&fixture.runtime).await.unwrap();
    let ollama = services
        .connection(&ollama_prefs, super::config::ProviderKind::Llm, false)
        .await
        .unwrap();
    assert_eq!(ollama.base_url, "http://127.0.0.1:11434/v1");
    fixture
        .setting("ai_provider:llm:ollama", json!({"base_url": server.url}))
        .await;
    assert_eq!(
        read_completion(&resolver, &fixture.session).await,
        "fixture résumé"
    );

    // 8. On-device Apple Intelligence resolves without cloud auth or API key
    fixture
        .setting("current_llm_provider", json!("apple_foundation"))
        .await;
    fixture
        .setting("current_llm_model", json!("System Language Model"))
        .await;
    assert!(resolver(fixture.session.clone()).await.is_ok());

    // 9. Real ModelService local resolver validates availability/install without opening a sidecar slot
    let models_dir = tempfile::tempdir().unwrap();
    let engines_dir = tempfile::tempdir().unwrap();
    let model_service = Arc::new(crate::product::local::models::ModelService::new(
        models_dir.path().to_owned(),
        crate::product::local::models::EngineResources {
            argmax: engines_dir.path().join("argmax"),
            llama_server: engines_dir.path().join("llama-server"),
            argmax_api_key: String::new(),
        },
    ));
    let real_models = model_service.clone();
    services.local = Arc::new(move |request| {
        let models = real_models.clone();
        Box::pin(async move {
            let model = crate::product::local::models::ModelService::resolve(&request.model)?;
            let cancel = CancellationToken::new();
            if request.batch {
                models.batch(model, cancel).await
            } else {
                models.start(model, cancel).await
            }
        })
    });
    for (provider, model_id, expected_url) in [
        (
            "soniqo",
            "soniqo-parakeet-streaming",
            anlg_transcribe_soniqo::LOCAL_BASE_URL,
        ),
        (
            "soniqo",
            "soniqo-parakeet-batch",
            anlg_transcribe_soniqo::LOCAL_BASE_URL,
        ),
        (
            "apple_speech",
            "apple-speech",
            anlg_transcribe_speechanalyzer::LOCAL_BASE_URL,
        ),
    ] {
        fixture
            .setting("current_stt_provider", json!(provider))
            .await;
        fixture.setting("current_stt_model", json!(model_id)).await;
        let model = crate::product::local::models::ModelService::resolve(model_id).unwrap();
        let state = model_service.state(model.clone()).await.unwrap();
        if state.installed {
            let config = services.capture(fixture.session.clone()).await.unwrap();
            assert_eq!(config.params.base_url, expected_url);
            let params = services
                .batch(fixture.session.clone(), PathBuf::from("meeting.wav"))
                .await
                .unwrap();
            assert_eq!(params.base_url, expected_url);
        } else {
            let Err(expected_error) = model_service
                .batch(model.clone(), desktop_runtime::CancellationToken::new())
                .await
            else {
                panic!("expected model service rejection for uninstalled {model_id}");
            };
            let expected_error = expected_error.to_string();
            let Err(capture_error) = services.capture(fixture.session.clone()).await else {
                panic!("expected local capture error for uninstalled {model_id}");
            };
            assert_eq!(capture_error.to_string(), expected_error);
            let Err(batch_error) = services
                .batch(fixture.session.clone(), PathBuf::from("meeting.wav"))
                .await
            else {
                panic!("expected local batch error for uninstalled {model_id}");
            };
            assert_eq!(batch_error.to_string(), expected_error);
        }
        assert!(!model_service.state(model).await.unwrap().running);
    }
    fixture.close().await;
}

#[test]
fn resolve_chunk_target_prefers_started_capture_before_tolerance() {
    let chunk = anlg_listener_core::actors::recorder::RecoveryAudioChunk {
        id: "10001-0-1000-0.mp3".into(),
        path: "10001-0-1000-0.mp3".into(),
        capture_started_at: 10_001,
        start_ms: 0,
        end_ms: 1000,
        audio_start_ms: 0,
    };
    let primary = super::recovery::CaptureTarget {
        transcript: "primary".into(),
        started: 20_000,
        provider: String::new(),
        model: String::new(),
        memo: String::new(),
        created_at: None,
        owner_user_id: String::new(),
        retained: true,
    };
    let inherited = vec![
        super::recovery::CaptureTarget {
            transcript: "older".into(),
            started: 10_000,
            provider: String::new(),
            model: String::new(),
            memo: String::new(),
            created_at: None,
            owner_user_id: String::new(),
            retained: true,
        },
        super::recovery::CaptureTarget {
            transcript: "newer".into(),
            started: 10_004,
            provider: String::new(),
            model: String::new(),
            memo: String::new(),
            created_at: None,
            owner_user_id: String::new(),
            retained: true,
        },
    ];
    let target = super::recovery::resolve_chunk_target(&chunk, &primary, &inherited).unwrap();
    assert_eq!(target.transcript.as_ref(), "older");
}

#[tokio::test]
async fn startup_recovery_finalizes_already_repaired_inherited_transcript_on_retry() {
    let fixture = Fixture::new().await;
    let inherited_id: Arc<str> = uuid::Uuid::new_v4().to_string().into();
    let other_missing_inherited: Arc<str> = uuid::Uuid::new_v4().to_string().into();
    fixture
        .sql(
            "INSERT INTO transcripts (id, session_id, owner_user_id, workspace_id, source, provider, model, started_at_ms, memo, created_at) SELECT ?, id, owner_user_id, workspace_id, 'live_capture', '', '', 1000, '', '2026-11-26T00:00:00Z' FROM sessions WHERE id = ?",
            vec![json!(inherited_id), json!(fixture.session)],
        )
        .await;
    let key = format!("capture_lifecycle_pending:{}", fixture.session.0);
    fixture
        .setting(
            &key,
            json!({
                "version": 1,
                "sessionId": fixture.session,
                "transcriptId": fixture.transcript,
                "retainAudio": true,
                "startedAt": 2000,
                "inheritedCaptures": [
                    { "transcriptId": inherited_id, "startedAt": 1000 },
                    { "transcriptId": other_missing_inherited, "startedAt": 1500 },
                ],
            }),
        )
        .await;
    let resolver: super::recovery::RecoveryResolver =
        Arc::new(|_, _| Box::pin(async { Err(super::model::failure("unused")) }));
    let reports = super::recovery::recover_startup(
        &fixture.runtime,
        fixture.directory.clone(),
        Activities::default(),
        resolver,
    )
    .await
    .unwrap();
    assert_eq!(reports.len(), 1);
    reports[0].result.as_ref().unwrap();
    let rows = fixture
        .sql(
            "SELECT ended_at_ms FROM transcripts WHERE id = ?",
            vec![json!(inherited_id)],
        )
        .await;
    assert_eq!(rows.len(), 1);
    assert!(!rows[0]["ended_at_ms"].is_null());
    assert!(
        fixture
            .sql(
                "SELECT id FROM transcripts WHERE id = ?",
                vec![json!(other_missing_inherited)],
            )
            .await
            .is_empty()
    );
    fixture.close().await;
}
