use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    sync::Arc,
    thread,
    time::Duration,
};

use anlg_calendar::CalendarProviderType;
use desktop_runtime::{CancellationToken, LibraryQuery, Profile, RuntimeHandle, ServiceError};
use futures::future::BoxFuture;
use serde_json::{Value, json};

use super::{
    calendar::{AccessToken, CalendarAuth, CalendarService},
    data,
    export::{self, ExportOptions, Format},
    storage::PreparedMove,
};

struct FixtureAuth;
impl CalendarAuth for FixtureAuth {
    fn token(
        &self,
        _: CancellationToken,
    ) -> BoxFuture<'static, desktop_runtime::Result<AccessToken>> {
        Box::pin(async { Ok(AccessToken::new("fixture-token".into())) })
    }
    fn connect(
        &self,
        _: CalendarProviderType,
        _: CancellationToken,
    ) -> BoxFuture<'static, desktop_runtime::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn disconnect(
        &self,
        _: CalendarProviderType,
        _: String,
        _: CancellationToken,
    ) -> BoxFuture<'static, desktop_runtime::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

#[tokio::test]
async fn calendar_uses_typed_auth_and_fetches_all_event_pages() {
    let root = tempfile::tempdir().unwrap();
    let runtime = runtime(root.path()).await;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let service = CalendarService::new(
        format!("http://{}", listener.local_addr().unwrap()).into(),
        Arc::new(FixtureAuth),
    );
    let server = thread::spawn(move || {
        let replies = [
            (
                "/nango/connections",
                json!({"connections":[{"integration_id":"google-calendar","connection_id":"google-1"},{"integration_id":"outlook","connection_id":"outlook-1"}]}),
            ),
            (
                "/calendar/google/list-calendars",
                json!({"items":[{"id":"calendar","summary":"日本語","accessRole":"owner"}]}),
            ),
            (
                "/calendar/google/list-events",
                json!({"items":[],"nextPageToken":"page-two"}),
            ),
            (
                "/calendar/google/list-events",
                json!({"items":[
                    {"id":"event","summary":"Meeting","organizer":{"email":"me@example.com","self":true},"start":{"dateTime":"2026-09-21T09:00:00Z"},"end":{"dateTime":"2026-09-21T10:00:00Z"}},
                    {"id":"declined","summary":"Declined","attendees":[{"email":"me@example.com","self":true,"responseStatus":"declined"}],"start":{"dateTime":"2026-09-21T11:00:00Z"},"end":{"dateTime":"2026-09-21T12:00:00Z"}}
                ]}),
            ),
        ];
        for (index, (path, response)) in replies.into_iter().enumerate() {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            assert!(first.contains(path));
            let mut authorized = false;
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                if line.to_ascii_lowercase().starts_with("authorization:") {
                    authorized = line.trim().ends_with("Bearer fixture-token");
                }
                if let Some((_, value)) = line
                    .to_ascii_lowercase()
                    .strip_prefix("content-length")
                    .and_then(|line| line.split_once(':'))
                {
                    length = value.trim().parse::<usize>().unwrap();
                }
            }
            assert!(authorized);
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            if index == 3 {
                assert_eq!(
                    serde_json::from_slice::<Value>(&body).unwrap()["page_token"],
                    "page-two"
                );
            }
            let body = response.to_string();
            write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        }
    });
    let connections = service.connections(CancellationToken::new()).await.unwrap();
    assert_eq!(connections[0].connection_ids, vec!["google-1"]);
    assert_eq!(connections[1].connection_ids, vec!["outlook-1"]);
    let calendars = service
        .discover(
            &runtime,
            CalendarProviderType::Google,
            "google-1".into(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(calendars[0]["name"], "日本語");
    let id = calendars[0]["id"].as_str().unwrap().to_owned();
    service
        .toggle(&runtime, id.clone(), CancellationToken::new())
        .await
        .unwrap();
    let events = runtime
        .submit(|services| async move {
            services
                .executor
                .execute("SELECT * FROM events".into(), vec![])
                .await
                .map_err(super::failure)
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["title"], "Meeting");
    assert_eq!(events[0]["calendar_id"], id);
    assert!(events[0]["deleted_at"].is_null());
    // Declined events are not kept; the attendance seen now gates automatic recording.
    let attendance = events[0]["attendance_json"].as_str().unwrap();
    assert!(attendance.contains(r#""self_status":"organizer""#));
    assert!(
        crate::meeting::schedule::attendance::decide(
            Some(attendance),
            chrono::Utc::now().timestamp_millis()
        )
        .eligible()
    );
    service
        .toggle(&runtime, id, CancellationToken::new())
        .await
        .unwrap();
    let events = runtime
        .submit(|services| async move {
            services
                .executor
                .execute("SELECT * FROM events".into(), vec![])
                .await
                .map_err(super::failure)
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert!(events[0]["deleted_at"].as_str().is_some());
    assert_eq!(
        service.meeting_link("Meet https://meet.google.com/abc-defg-hij"),
        Some("https://meet.google.com/abc-defg-hij".into())
    );
    server.join().unwrap();
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        service.connections(cancelled).await,
        Err(ServiceError::Cancelled)
    ));
    runtime.shutdown().await.unwrap();
}

async fn runtime(root: &std::path::Path) -> RuntimeHandle {
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: root.join("library.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    runtime
}

/// A user who finished onboarding in the shipping app isn't sent back to it, and finishing it
/// here is recorded where a rollback reads it.
#[tokio::test]
async fn channel_onboarding_follows_shipping_store_json() {
    use crate::product::onboarding;
    use anlg_desktop_identity::store_json::StoreJson;
    let root = tempfile::tempdir().unwrap();
    let runtime = runtime(root.path()).await;
    let needed = || async {
        onboarding::needed(
            data::setting_value(&runtime, onboarding::NEEDED_KEY.into())
                .await
                .unwrap()
                .as_ref(),
        )
    };
    let fresh = Arc::new(StoreJson::at(root.path().join("fresh/store.json")));
    onboarding::import_shipping(&runtime, fresh.clone())
        .await
        .unwrap();
    assert!(needed().await, "no shipping onboarding yet");

    let shipping = Arc::new(StoreJson::at(root.path().join("store.json")));
    shipping
        .set("desktop", "OnboardingNeeded2", json!(false))
        .unwrap();
    onboarding::import_shipping(&runtime, shipping)
        .await
        .unwrap();
    assert!(!needed().await);

    onboarding::complete(&runtime, Some(fresh.clone()))
        .await
        .unwrap();
    assert_eq!(
        fresh.get("desktop", "OnboardingNeeded2").unwrap(),
        Some(json!(false))
    );
}

#[tokio::test]
async fn generated_imports_and_onboarding_use_the_shipping_note_id() {
    let root = tempfile::tempdir().unwrap();
    let runtime = runtime(root.path()).await;
    let welcome = data::complete_onboarding(&runtime).await.unwrap();
    let mut ids = vec![welcome];
    for (extension, content) in [
        ("md", "# Hello 日本語 🚀"),
        ("txt", "Hello 日本語 🚀"),
        ("srt", "1\n00:00:01,000 --> 00:00:02,500\nHello\n\n"),
        ("vtt", "WEBVTT\n\n00:00:01.000 --> 00:00:02.500\nHello\n\n"),
        (
            "json",
            r#"{"notes":"Hello","summary":"Keep summary separate"}"#,
        ),
        ("csv", "title,notes\nImported,Hello\n"),
    ] {
        ids.extend(
            data::import_meetings(
                &runtime,
                data::parse(extension, "Cross-client import", content).unwrap(),
                CancellationToken::new(),
            )
            .await
            .unwrap(),
        );
    }
    for id in ids {
        let saved = data::snapshot(&runtime, id.clone()).await.unwrap();
        let note = saved
            .documents
            .iter()
            .find(|doc| doc["kind"] == "note")
            .unwrap();
        assert_eq!(note["id"], id.0.as_ref());
        assert_eq!(note["session_id"], id.0.as_ref());
        assert_eq!(note["body_format"], "prosemirror_json");
        for summary in saved
            .documents
            .iter()
            .filter(|doc| doc["kind"] == "summary")
        {
            assert_ne!(summary["id"], id.0.as_ref());
        }
    }
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn imports_join_the_connected_library_with_their_own_dates() {
    let root = tempfile::tempdir().unwrap();
    let runtime = runtime(root.path()).await;
    runtime
        .submit(|services| async move {
            for sql in [
                "INSERT OR REPLACE INTO app_settings (id, value_json) VALUES ('cloudsync_workspace_binding', '{\"workspace_id\":\"device-workspace\"}')",
                "INSERT INTO local_library_connections (account_user_id, library_workspace_id, active) VALUES ('account', 'connected-library', 1)",
            ] {
                services.executor.execute(sql.into(), vec![]).await.map_err(super::failure)?;
            }
            Ok(())
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    let content = json!({
        "title": "Standup", "notes": "Agenda", "start_time": "2026-01-05T10:00:00+01:00",
        "end_time": 1767607200, "transcript": [{"text": "Hello"}], "attendees": [{"name": "Ada"}],
    });
    let ids = data::import_meetings(
        &runtime,
        data::parse("json", "Standup", &content.to_string()).unwrap(),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    let imported = data::snapshot(&runtime, ids[0].clone()).await.unwrap();
    assert_eq!(imported.session["created_at"], "2026-01-05T09:00:00.000Z");
    assert_eq!(imported.session["started_at"], "2026-01-05T09:00:00.000Z");
    assert_eq!(imported.session["ended_at"], "2026-01-05T10:00:00.000Z");
    assert_eq!(imported.session["owner_user_id"], "connected-library");
    let rows = [&imported.session]
        .into_iter()
        .chain(&imported.documents)
        .chain(&imported.transcripts)
        .chain(&imported.participants)
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 4);
    for row in rows {
        assert_eq!(row["workspace_id"], "device-workspace", "{row}");
    }
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn canonical_import_preserves_existing_document_ids() {
    let root = tempfile::tempdir().unwrap();
    let runtime = runtime(root.path()).await;
    let mut meeting = data::parse("md", "Imported archive", "Exact note")
        .unwrap()
        .remove(0);
    meeting.documents[0]["id"] = json!("archived-note-id");
    meeting.documents.push(json!({
        "id": "archived-summary-id",
        "session_id": meeting.session["id"],
        "kind": "summary",
        "body_format": "prosemirror_json",
        "body": r#"{"type":"doc","content":[{"type":"paragraph"}]}"#,
    }));
    let parsed = data::parse("json", "Archive", &serde_json::to_string(&meeting).unwrap()).unwrap();
    let ids = data::import_meetings(&runtime, parsed, CancellationToken::new())
        .await
        .unwrap();
    let imported = data::snapshot(&runtime, ids[0].clone()).await.unwrap();
    for original in meeting.documents {
        let restored = imported
            .documents
            .iter()
            .find(|doc| doc["id"] == original["id"])
            .unwrap();
        assert_eq!(restored["session_id"], original["session_id"]);
        assert_eq!(restored["body"], original["body"]);
    }
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn onboarding_is_durable_and_reuses_the_welcome_session() {
    let root = tempfile::tempdir().unwrap();
    let runtime = runtime(root.path()).await;
    let first = data::complete_onboarding(&runtime).await.unwrap();
    let second = data::complete_onboarding(&runtime).await.unwrap();
    assert_eq!(first, second);
    let meeting = data::snapshot(&runtime, first.clone()).await.unwrap();
    assert!(
        meeting.documents[0]["body"]
            .as_str()
            .unwrap()
            .contains("Join &amp; record")
            || meeting.documents[0]["body"]
                .as_str()
                .unwrap()
                .contains("Join & record")
    );
    assert_eq!(
        data::setting_value(&runtime, "onboarding_needed".into())
            .await
            .unwrap(),
        Some(json!(false))
    );
    runtime.shutdown().await.unwrap();
    let reopened = self::runtime(root.path()).await;
    assert_eq!(data::complete_onboarding(&reopened).await.unwrap(), first);
    reopened.shutdown().await.unwrap();
}

#[tokio::test]
async fn imports_are_atomic_conflict_preserving_and_keep_opaque_documents() {
    let root = tempfile::tempdir().unwrap();
    let runtime = runtime(root.path()).await;
    let mut meeting = data::parse("md", "日本語 😀", "# Hello").unwrap().remove(0);
    let opaque = r#" { "type":"doc", "extra":{"keep":true}, "content":[{"type":"futureNode","attrs":{"value":"😀"}}] } "#;
    meeting.documents[0]["body"] = json!(opaque);
    let ids = data::import_meetings(&runtime, vec![meeting.clone()], CancellationToken::new())
        .await
        .unwrap();
    let restored = data::snapshot(&runtime, ids[0].clone()).await.unwrap();
    assert_eq!(restored.documents[0]["body"], opaque);
    assert!(matches!(
        data::import_meetings(&runtime, vec![meeting], CancellationToken::new()).await,
        Err(ServiceError::Conflict)
    ));
    let mut other = data::parse("md", "New", "not committed").unwrap().remove(0);
    let duplicate = other.clone();
    assert!(
        data::import_meetings(
            &runtime,
            vec![other.clone(), duplicate],
            CancellationToken::new()
        )
        .await
        .is_err()
    );
    other.session["future_column"] = json!("must not be dropped");
    assert!(
        data::import_meetings(&runtime, vec![other], CancellationToken::new())
            .await
            .is_err()
    );
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(matches!(
        data::import_files(&runtime, vec![root.path().join("missing.md")], cancelled).await,
        Err(ServiceError::Cancelled)
    ));
    assert_eq!(
        runtime
            .library(LibraryQuery::default(), CancellationToken::new())
            .unwrap()
            .receive()
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    let path = root.path().join("opaque.json");
    export::export(
        &restored,
        &ExportOptions {
            format: Format::Canonical,
            ..Default::default()
        },
        &path,
    )
    .unwrap();
    let exported: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(exported["documents"][0]["body"], opaque);
    assert!(
        export::content(
            &restored,
            &ExportOptions {
                memo: true,
                ..Default::default()
            }
        )
        .is_err()
    );
    runtime.shutdown().await.unwrap();
}

#[test]
fn transcript_only_imports_provide_editable_empty_memos() {
    for (extension, content) in [
        ("srt", "1\n00:00:01,000 --> 00:00:02,500\nAda: Hello\n\n"),
        ("vtt", "WEBVTT\n\n00:00:01.000 --> 00:00:02.500\nHello\n\n"),
        ("txt", "Hello"),
        ("json", r#"{"transcript":"Hello"}"#),
        ("md", ""),
    ] {
        let meeting = data::parse(extension, "Imported", content)
            .unwrap()
            .remove(0);
        let body = meeting.documents[0]["body"].as_str().unwrap();
        let document = crate::editor::document::Document::parse(body.into()).unwrap();
        let mut editor = crate::editor::model::EditorModel::new(document);
        editor.replace(editor.selection.range(), "Memo 😀").unwrap();
        let saved = editor.document.serialize_for_save().unwrap();
        assert!(saved.contains("Memo 😀"));
        crate::editor::document::Document::parse(saved).unwrap();
    }
}

#[test]
fn source_formats_preserve_unicode_speakers_timing_and_original_fields() {
    for extension in ["srt", "vtt"] {
        let text = if extension == "srt" {
            "1\n00:00:01,000 --> 00:00:02,500\nAda: こんにちは 😀\n\n"
        } else {
            "WEBVTT\n\n00:00:01.000 --> 00:00:02.500\n<v Ada>こんにちは 😀</v>\n\n"
        };
        let meeting = data::parse(extension, "Captions", text).unwrap().remove(0);
        let words: Value =
            serde_json::from_str(meeting.transcripts[0]["words_json"].as_str().unwrap()).unwrap();
        assert_eq!(words[0]["start_ms"], 1000);
        assert_eq!(words[0]["end_ms"], 2500);
        assert_eq!(words[0]["speaker"], "Ada");
        let output = export::content(
            &meeting,
            &ExportOptions {
                transcript: true,
                ..Default::default()
            },
        )
        .unwrap();
        let items = output.transcript.unwrap().items;
        assert_eq!(items[0].speaker.as_deref(), Some("Ada"));
        assert!(items[0].text.contains("こんにちは 😀"));
    }
    let meeting = data::parse(
        "csv",
        "CSV",
        "title,notes,custom\n\"日本語 😀\",\"memo, with comma\",preserve\n",
    )
    .unwrap()
    .remove(0);
    let original: Value =
        serde_json::from_str(meeting.session["metadata_json"].as_str().unwrap()).unwrap();
    assert_eq!(original["import_original"]["custom"], "preserve");
    assert_eq!(meeting.title(), "日本語 😀");
}

#[test]
fn all_shipping_export_formats_write_real_output() {
    let root = tempfile::tempdir().unwrap();
    let meeting = data::parse(
        "md",
        "Unicode 😀",
        "# Heading\n\n**Memo** and 日本語.\n\n- One\n- Two",
    )
    .unwrap()
    .remove(0);
    for format in [
        Format::Text,
        Format::Markdown,
        Format::Org,
        Format::Pdf,
        Format::Canonical,
    ] {
        let path = root.path().join(format!("export.{}", format.extension()));
        export::export(
            &meeting,
            &ExportOptions {
                format,
                memo: true,
                ..Default::default()
            },
            &path,
        )
        .unwrap();
        let bytes = fs::read(path).unwrap();
        assert!(!bytes.is_empty());
        if format == Format::Pdf {
            assert!(bytes.starts_with(b"%PDF"));
        } else {
            assert!(String::from_utf8(bytes).unwrap().contains("日本語"));
        }
    }
}

#[test]
fn storage_move_checks_containment_tampering_and_rolls_back_pointer() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let target = root.path().join("target");
    fs::create_dir_all(source.join("empty")).unwrap();
    fs::write(source.join("database"), b"sqlite fixture").unwrap();
    let cancel = CancellationToken::new();
    assert!(PreparedMove::prepare(&source, &source.join("child"), &cancel).is_err());
    let prepared = PreparedMove::prepare(&source, &target, &cancel).unwrap();
    fs::write(source.join("database"), b"changed after prepare").unwrap();
    assert!(matches!(
        prepared.commit(&root.path().join("pointer.json"), &cancel),
        Err(ServiceError::Conflict)
    ));
    prepared.abort().unwrap();
    let prepared = PreparedMove::prepare(&source, &target, &cancel).unwrap();
    let pointer = root.path().join("pointer.json");
    fs::write(&pointer, "\"old\"").unwrap();
    let committed = prepared.commit(&pointer, &cancel).unwrap();
    assert_eq!(
        fs::read(target.join("database")).unwrap(),
        b"changed after prepare"
    );
    assert!(source.join("database").exists());
    assert!(target.join("empty").is_dir());
    committed.rollback().unwrap();
    assert_eq!(fs::read_to_string(&pointer).unwrap(), "\"old\"");
    assert!(PreparedMove::prepare(&source, &target, &cancel).is_err());
    cancel.cancel();
    assert!(matches!(
        PreparedMove::prepare(&source, &root.path().join("cancelled"), &cancel),
        Err(ServiceError::Cancelled)
    ));
}
