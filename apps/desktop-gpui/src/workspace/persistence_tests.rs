use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::Arc,
    time::Duration,
};

use chrono::NaiveDate;
use desktop_runtime::{CancellationToken, Profile, RuntimeHandle, ServiceError, SessionId};
use futures::executor::block_on;
use serde_json::{Value, json};

use super::{
    automations::{self, Step, StepType, Target, Workflow},
    calendar::{Clock, by_day, load_calendar, month_days},
    folders,
    navigation::Route,
    notes::{self, NoteCommand},
    picker::{NOTES_QUERY, load_recent, save_recent},
    pins, templates,
};

#[test]
fn pins_use_cas_and_malformed_storage_cannot_become_empty_success() {
    block_on(async {
        let (_directory, runtime) = start().await;
        let (base, routes) = pins::load(&runtime).unwrap().receive().await.unwrap();
        assert!(routes.is_empty());
        let routes: Arc<[Route]> = vec![Route::Contacts, Route::Calendar].into();
        pins::save(&runtime, base.clone(), routes.clone())
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert!(matches!(
            pins::save(&runtime, base, Arc::from([]))
                .unwrap()
                .receive()
                .await,
            Err(ServiceError::Conflict)
        ));
        let (_, restored) = pins::load(&runtime).unwrap().receive().await.unwrap();
        assert_eq!(restored.as_slice(), routes.as_ref());
        execute(
            &runtime,
            "UPDATE app_settings SET value_json = '{broken' WHERE id = 'gpui_pinned_tabs'",
            vec![],
        )
        .await;
        assert!(pins::load(&runtime).unwrap().receive().await.is_err());
        let raw = execute(
            &runtime,
            "SELECT value_json FROM app_settings WHERE id = 'gpui_pinned_tabs'",
            vec![],
        )
        .await;
        assert_eq!(raw[0]["value_json"], "{broken");
        runtime.shutdown().await.unwrap();
    });
}

#[test]
fn picker_scopes_shared_cache_to_viewer_and_deduplicates_managed_local_notes() {
    block_on(async {
        let (_directory, runtime) = start().await;
        execute(&runtime,"INSERT INTO sessions(id,title,created_at) VALUES ('local','Local','2024-01-01'),('older','Older','2023-01-01T00:00:00Z')",vec![]).await;
        for (share, viewer, session, managed) in [
            ("duplicate", "viewer", "local", 1),
            ("shared", "viewer", "remote", 0),
            ("other-account", "other", "remote2", 0),
        ] {
            execute(&runtime,"INSERT INTO shared_session_cache(share_id,viewer_user_id,workspace_id,session_id,content_revision,access_version,published_at,title,manage_access) VALUES (?,?,'workspace',?,1,1,'2025-01-01',?,?)",
                vec![json!(share),json!(viewer),json!(session),json!(share),json!(managed)]).await;
        }
        let ids = |rows: Vec<Value>| {
            rows.iter()
                .map(|row| row["id"].as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            ids(execute(&runtime, NOTES_QUERY, vec![json!("viewer")]).await),
            ["shared", "local", "older"]
        );
        assert_eq!(
            ids(execute(&runtime, NOTES_QUERY, vec![Value::Null]).await),
            ["local", "older"]
        );
        execute(
            &runtime,
            "UPDATE sessions SET deleted_at='deleted' WHERE id='local'",
            vec![],
        )
        .await;
        assert_eq!(
            ids(execute(&runtime, NOTES_QUERY, vec![json!("viewer")]).await),
            ["duplicate", "shared", "older"]
        );
        runtime.shutdown().await.unwrap();
    });
}

#[test]
fn recently_opened_notes_persist_and_malformed_storage_reads_as_empty() {
    block_on(async {
        let (_directory, runtime) = start().await;
        let load = || async {
            load_recent(&runtime, CancellationToken::new())
                .unwrap()
                .receive()
                .await
                .unwrap()
        };
        assert!(load().await.is_empty());
        let recent = vec![SessionId("b".into()), SessionId("a".into())];
        save_recent(&runtime, recent.clone())
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert_eq!(load().await, recent);
        execute(
            &runtime,
            "UPDATE app_settings SET value_json='{oops' WHERE id='gpui_recent_sessions'",
            vec![],
        )
        .await;
        assert!(load().await.is_empty());
        runtime.shutdown().await.unwrap();
    });
}

#[test]
fn malformed_workflow_storage_is_an_error_and_remains_unchanged() {
    block_on(async {
        let (_directory, runtime) = start().await;
        execute(
            &runtime,
            "INSERT INTO app_settings(id,value_json) VALUES ('automation_workflows','{invalid')",
            vec![],
        )
        .await;
        let workflow = Workflow::empty(None, None, "Untitled automation");
        assert!(
            automations::save(&runtime, workflow)
                .unwrap()
                .receive()
                .await
                .is_err()
        );
        let rows = execute(
            &runtime,
            "SELECT value_json FROM app_settings WHERE id='automation_workflows'",
            vec![],
        )
        .await;
        assert_eq!(rows[0]["value_json"], "{invalid");
        runtime.shutdown().await.unwrap();
    });
}

async fn execute(runtime: &RuntimeHandle, sql: &str, params: Vec<Value>) -> Vec<Value> {
    let sql = sql.to_owned();
    runtime
        .submit(move |services| async move {
            services
                .executor
                .execute(sql, params)
                .await
                .map_err(|error| ServiceError::Failed(error.to_string().into()))
        })
        .unwrap()
        .receive()
        .await
        .unwrap()
}

async fn start() -> (tempfile::TempDir, RuntimeHandle) {
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("workspace.sqlite"),
    })
    .unwrap();
    ready.receive().await.unwrap();
    (directory, runtime)
}

#[test]
fn timeline_events_skip_ignored_declined_and_noted_events() {
    block_on(async {
        let (_directory, runtime) = start().await;
        execute(
            &runtime,
            "INSERT INTO calendars(id,name,enabled) VALUES ('work','Work',1)",
            vec![],
        )
        .await;
        for (id, tracking, offset, attendance) in [
            ("upcoming", "a", "+1 hours", None),
            ("ignored", "b", "+1 hours", None),
            (
                "declined",
                "c",
                "+1 hours",
                Some(r#"{"self_status":"declined"}"#),
            ),
            ("noted", "d", "+1 hours", None),
            ("next-week", "f", "+7 days", None),
        ] {
            execute(
                &runtime,
                "INSERT INTO events(id,title,calendar_id,tracking_id_event,started_at,ended_at,attendance_json)
                VALUES (?1,?1,'work',?2,strftime('%Y-%m-%dT%H:%M:%SZ','now',?3),
                    strftime('%Y-%m-%dT%H:%M:%SZ','now',?3,'+1 hours'),?4)",
                vec![json!(id), json!(tracking), json!(offset), json!(attendance)],
            )
            .await;
        }
        execute(
            &runtime,
            "INSERT INTO sessions(id,title,event_json) VALUES ('note','Noted',?)",
            vec![json!(r#"{"tracking_id":"d"}"#)],
        )
        .await;
        execute(
            &runtime,
            "INSERT INTO app_settings(id,value_json) VALUES ('ignored_events',?)",
            vec![json!(r#"[{"tracking_id":"b"},"legacy"]"#)],
        )
        .await;
        let window = super::timeline::load_events(&runtime, CancellationToken::new())
            .unwrap()
            .receive()
            .await
            .unwrap();
        let ids = window
            .events
            .iter()
            .map(|event| event.id.as_ref())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["upcoming"]);
        assert!(
            window.later,
            "next week's event leaves more for the calendar"
        );
        runtime.shutdown().await.unwrap();
    });
}

#[test]
fn note_folder_resolution_follows_moved_notes_and_rejects_locked_or_deleted_notes() {
    block_on(async {
        let (directory, runtime) = start().await;
        let session = runtime
            .create_note("Folder reveal".into())
            .unwrap()
            .receive()
            .await
            .unwrap()
            .summary
            .id;
        let path = directory
            .path()
            .join("vault/sessions/Research")
            .join(session.0.as_ref());
        std::fs::create_dir_all(&path).unwrap();
        assert_eq!(
            notes::directory(&runtime, session.clone())
                .unwrap()
                .receive()
                .await
                .unwrap(),
            // The resolver canonicalizes, and macOS temp directories live behind /var -> /private/var.
            path.canonicalize().unwrap()
        );
        execute(
            &runtime,
            "UPDATE sessions SET locked=1 WHERE id=?",
            vec![json!(session)],
        )
        .await;
        assert!(
            notes::directory(&runtime, session.clone())
                .unwrap()
                .receive()
                .await
                .is_err()
        );
        execute(
            &runtime,
            "UPDATE sessions SET locked=0,deleted_at='deleted' WHERE id=?",
            vec![json!(session)],
        )
        .await;
        assert!(
            notes::directory(&runtime, session)
                .unwrap()
                .receive()
                .await
                .is_err()
        );
        assert!(path.is_dir());
        runtime.shutdown().await.unwrap();
    });
}

#[test]
fn note_folder_resolution_rejects_missing_directories_and_regular_files() {
    block_on(async {
        let (directory, runtime) = start().await;
        let session = runtime
            .create_note("Missing folder".into())
            .unwrap()
            .receive()
            .await
            .unwrap()
            .summary
            .id;
        let path = directory
            .path()
            .join("vault/sessions")
            .join(session.0.as_ref());
        let error = notes::directory(&runtime, session.clone())
            .unwrap()
            .receive()
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Note folder is unavailable"));
        assert!(!path.exists());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "not a directory").unwrap();
        let error = notes::directory(&runtime, session)
            .unwrap()
            .receive()
            .await
            .unwrap_err();
        assert!(error.to_string().contains("not a directory"));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "not a directory");
        runtime.shutdown().await.unwrap();
    });
}

#[test]
fn folder_material_commands_copy_checksum_and_soft_delete_the_catalog_entry() {
    block_on(async {
        let (directory, runtime) = start().await;
        folders::create(&runtime, "Research".into())
            .unwrap()
            .receive()
            .await
            .unwrap();
        let path = directory.path().join("syllabus.txt");
        std::fs::write(&path, "日本 syllabus").unwrap();
        super::folders::material_command(
            &runtime,
            "Research".into(),
            super::folders::MaterialCommand::Import(path),
        )
        .unwrap()
        .receive()
        .await
        .unwrap();
        let materials = super::folders::materials(&runtime, "Research".into())
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert_eq!(materials.len(), 1);
        let rows = execute(&runtime, "SELECT * FROM folder_attachments", vec![]).await;
        assert_eq!(rows[0]["sha256"].as_str().unwrap().len(), 64);
        assert_eq!(rows[0]["relative_path"], "materials/syllabus.txt");
        assert_eq!(
            std::fs::read_to_string(
                directory
                    .path()
                    .join("vault/sessions/Research/materials/syllabus.txt")
            )
            .unwrap(),
            "日本 syllabus"
        );
        super::folders::material_command(
            &runtime,
            "Research".into(),
            super::folders::MaterialCommand::Remove(materials[0].id.clone()),
        )
        .unwrap()
        .receive()
        .await
        .unwrap();
        assert!(
            super::folders::materials(&runtime, "Research".into())
                .unwrap()
                .receive()
                .await
                .unwrap()
                .is_empty()
        );
        runtime.shutdown().await.unwrap();
    });
}

fn provider_fixture(
    responses: Vec<(&'static str, u16, Value)>,
) -> (String, std::thread::JoinHandle<Vec<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let worker = std::thread::spawn(move || {
        let mut bodies = Vec::new();
        for (expected, status, response) in responses {
            let started = std::time::Instant::now();
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            started.elapsed() < Duration::from_secs(15),
                            "Fixture did not receive {expected}"
                        );
                        std::thread::park_timeout(Duration::from_millis(5));
                    }
                    Err(error) => panic!("{error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0; 4096];
            let body = loop {
                let size = stream.read(&mut buffer).unwrap();
                assert!(size > 0);
                bytes.extend_from_slice(&buffer[..size]);
                assert!(bytes.len() < 1024 * 1024);
                if let Some(end) = bytes.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                    let header = std::str::from_utf8(&bytes[..end]).unwrap();
                    assert!(header.lines().next().unwrap().contains(expected));
                    assert!(
                        header
                            .to_ascii_lowercase()
                            .contains("authorization: bearer fixture-token")
                    );
                    let size = header
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|value| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= end + 4 + size {
                        break if size == 0 {
                            Value::Null
                        } else {
                            serde_json::from_slice(&bytes[end + 4..end + 4 + size]).unwrap()
                        };
                    }
                }
            };
            bodies.push(body);
            let encoded = response.to_string();
            write!(stream,"HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{encoded}",encoded.len()).unwrap();
        }
        bodies
    });
    (address, worker)
}

#[test]
fn automation_provider_adapters_follow_openapi_and_durable_errors_never_claim_success() {
    block_on(async {
        let (_directory, runtime) = start().await;
        let session = runtime
            .create_note("Provider contract".into())
            .unwrap()
            .receive()
            .await
            .unwrap()
            .summary
            .id;
        execute(&runtime,"INSERT INTO session_documents(id,session_id,kind,body_format,body) VALUES ('summary',?,'summary','markdown','Summary text')",vec![json!(session.0)]).await;
        execute(
            &runtime,
            "INSERT INTO action_items(id,session_id,text) VALUES ('action',?,'Follow up')",
            vec![json!(session.0)],
        )
        .await;
        let connections = json!({"connections":[{"integration_id":"notion","connection_id":"notion-connection","status":"connected"},{"integration_id":"linear","connection_id":"linear-connection","status":"connected"}]});
        let (address, worker) = provider_fixture(vec![
            ("/messenger/slack/messages", 200, json!({"ok":true})),
            ("/nango/connections", 200, connections.clone()),
            ("/notion/append-update", 200, json!({})),
            ("/nango/connections", 200, connections),
            ("/ticket/linear/create-issue", 200, json!({"id":"issue"})),
        ]);
        let target = |kind, id: &str, name: &str| Step::Target {
            id: id.into(),
            kind,
            target: Some(Target {
                id: id.into(),
                name: name.into(),
            }),
        };
        let mut workflow = Workflow::empty(None, None, "Provider contract");
        workflow.enabled = true;
        workflow.steps = vec![
            target(StepType::Slack, "channel", "Channel"),
            target(StepType::Notion, "page", "Page"),
            target(StepType::Linear, "team", "Team"),
        ];
        let id = workflow.id.clone();
        automations::save(&runtime, workflow)
            .unwrap()
            .receive()
            .await
            .unwrap();
        let client = super::automation_runner::AutomationClient::new(
            &address,
            "fixture-token".into(),
            "Fixture".into(),
        )
        .unwrap();
        super::automation_runner::run(&runtime, id.clone(), session.clone(), Some(client))
            .unwrap()
            .receive()
            .await
            .unwrap();
        let bodies = worker.join().unwrap();
        assert_eq!(bodies[0]["channel"], "channel");
        assert!(bodies[0]["text"].as_str().unwrap().contains("Summary text"));
        assert_eq!(bodies[2]["connection_id"], "notion-connection");
        assert_eq!(bodies[4]["title"], "Follow up");
        let stored = execute(
            &runtime,
            "SELECT value_json FROM app_settings WHERE id='automation_workflows'",
            vec![],
        )
        .await;
        let workflows =
            super::mutations::decode_json_text(stored[0]["value_json"].as_str().unwrap()).unwrap();
        assert_eq!(workflows[0]["lastRun"]["status"], "success");
        super::automation_runner::allow_retry(&runtime, id.clone(), session.clone())
            .unwrap()
            .receive()
            .await
            .unwrap();
        let (address, worker) = provider_fixture(vec![(
            "/messenger/slack/messages",
            503,
            json!({"error":"unavailable"}),
        )]);
        let client = super::automation_runner::AutomationClient::new(
            &address,
            "fixture-token".into(),
            "Fixture".into(),
        )
        .unwrap();
        assert!(
            super::automation_runner::run(&runtime, id, session.clone(), Some(client))
                .unwrap()
                .receive()
                .await
                .is_err()
        );
        worker.join().unwrap();
        let stored = execute(
            &runtime,
            "SELECT value_json FROM app_settings WHERE id='automation_workflows'",
            vec![],
        )
        .await;
        let workflows =
            super::mutations::decode_json_text(stored[0]["value_json"].as_str().unwrap()).unwrap();
        assert_eq!(workflows[0]["lastRun"]["status"], "error");
        assert!(super::automations::already_processed(
            &workflows[0],
            &session.0
        ));
        runtime.shutdown().await.unwrap();
    });
}

#[test]
fn selecting_a_template_for_a_note_keeps_its_memo() {
    block_on(async {
        let (_directory, runtime) = start().await;
        let template = templates::create(&runtime)
            .unwrap()
            .receive()
            .await
            .unwrap();
        execute(
            &runtime,
            "INSERT INTO sessions(id,title) VALUES ('imported','Imported')",
            vec![],
        )
        .await;
        execute(&runtime,"INSERT INTO session_documents(id,session_id,body) VALUES ('imported-document','imported','original')",vec![]).await;
        super::notes::select_template(
            &runtime,
            desktop_runtime::SessionId("imported".into()),
            template.clone(),
        )
        .unwrap()
        .receive()
        .await
        .unwrap();
        let memo = execute(
            &runtime,
            "SELECT template_id,body FROM session_documents WHERE id='imported-document'",
            vec![],
        )
        .await;
        assert_eq!(memo[0]["template_id"], template.as_ref());
        assert_eq!(memo[0]["body"], "original");
        runtime.shutdown().await.unwrap();
    });
}

#[test]
fn folders_and_note_commands_move_nested_paths_and_soft_delete_related_rows() {
    block_on(async {
        let (directory, runtime) = start().await;
        folders::create(&runtime, "Clients/日本".into())
            .unwrap()
            .receive()
            .await
            .unwrap();
        execute(&runtime,"INSERT INTO sessions(id,title,folder_path) VALUES ('one','One','Clients/日本'),('two','Two','Clients/日本/sub')",vec![]).await;
        execute(
            &runtime,
            "INSERT INTO folders(id,path) VALUES ('nested','Clients/日本/sub')",
            vec![],
        )
        .await;
        folders::rename(&runtime, "Clients/日本".into(), "Projects/日本".into())
            .unwrap()
            .receive()
            .await
            .unwrap();
        let rows = execute(
            &runtime,
            "SELECT folder_path FROM sessions ORDER BY id",
            vec![],
        )
        .await;
        assert_eq!(rows[0]["folder_path"], "Projects/日本");
        assert_eq!(rows[1]["folder_path"], "Projects/日本/sub");
        assert!(
            directory
                .path()
                .join("vault/sessions/Projects/日本")
                .exists()
        );
        notes::dispatch(
            &runtime,
            NoteCommand::Move {
                ids: vec![SessionId("one".into())].into(),
                folder: "".into(),
            },
        )
        .unwrap()
        .receive()
        .await
        .unwrap();
        assert_eq!(
            execute(
                &runtime,
                "SELECT folder_path FROM sessions WHERE id='one'",
                vec![]
            )
            .await[0]["folder_path"],
            ""
        );
        execute(&runtime,"INSERT INTO session_documents(id,session_id,body) VALUES ('two','two','{\"type\":\"doc\",\"content\":[]}')",vec![]).await;
        notes::dispatch(
            &runtime,
            NoteCommand::Delete(vec![SessionId("two".into())].into()),
        )
        .unwrap()
        .receive()
        .await
        .unwrap();
        assert!(
            !execute(
                &runtime,
                "SELECT deleted_at FROM session_documents WHERE id='two'",
                vec![]
            )
            .await[0]["deleted_at"]
                .is_null()
        );
        folders::delete(&runtime, "Projects/日本".into())
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert!(
            execute(
                &runtime,
                "SELECT id FROM folders WHERE path LIKE 'Projects/日本%' AND deleted_at IS NULL",
                vec![]
            )
            .await
            .is_empty()
        );
        runtime.shutdown().await.unwrap();
    });
}

#[test]
fn folder_route_creates_renames_updates_and_deletes_paths() {
    block_on(async {
        let (directory, runtime) = start().await;
        for _ in 0..2 {
            let path = folders::create(&runtime, " Clients ".into())
                .unwrap()
                .receive()
                .await
                .unwrap();
            assert_eq!(path.as_ref(), "Clients");
        }
        assert!(directory.path().join("vault/sessions/Clients").exists());
        execute(
            &runtime,
            "INSERT INTO sessions(id,title,folder_path) VALUES ('note','Kept','Clients/Sub')",
            vec![],
        )
        .await;
        let rows = execute(&runtime, folders::LIBRARY_SQL, vec![]).await;
        assert_eq!(
            folders::decode_library(&rows).paths.as_slice(),
            [Arc::<str>::from("Clients"), Arc::from("Clients/Sub")]
        );
        // A path only notes reference gains catalog rows when renamed.
        let renamed = folders::rename(&runtime, "Clients/Sub".into(), "Clients/Renamed".into())
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert_eq!(renamed.as_ref(), "Clients/Renamed");
        assert_eq!(
            execute(
                &runtime,
                "SELECT folder_path FROM sessions WHERE id='note'",
                vec![]
            )
            .await[0]["folder_path"],
            "Clients/Renamed"
        );
        folders::create(&runtime, "Other".into())
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert!(
            folders::rename(&runtime, "Other".into(), "Clients".into())
                .unwrap()
                .receive()
                .await
                .is_err()
        );
        let icon = super::resource_list::Glyph::Emoji("📁".into());
        for update in [
            folders::Update::Instructions("Weekly client calls".into()),
            folders::Update::Icon(icon.clone()),
        ] {
            folders::update(&runtime, "Clients".into(), update)
                .unwrap()
                .receive()
                .await
                .unwrap();
        }
        let row = &execute(
            &runtime,
            "SELECT instructions,icon_json FROM folders WHERE path='Clients' AND deleted_at IS NULL",
            vec![],
        )
        .await[0];
        assert_eq!(row["instructions"], "Weekly client calls");
        assert_eq!(
            super::resource_list::Glyph::folder(row["icon_json"].as_str().unwrap()),
            icon
        );
        folders::delete(&runtime, "Clients".into())
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert!(
            execute(
                &runtime,
                "SELECT id FROM folders WHERE path LIKE 'Clients%' AND deleted_at IS NULL",
                vec![]
            )
            .await
            .is_empty()
        );
        let note = &execute(
            &runtime,
            "SELECT folder_path,deleted_at FROM sessions WHERE id='note'",
            vec![],
        )
        .await[0];
        assert_eq!(note["folder_path"], "");
        assert!(note["deleted_at"].is_null());
        runtime.shutdown().await.unwrap();
    });
}

#[test]
fn template_route_writes_keep_stored_json_and_settings_consistent() {
    block_on(async {
        let (_directory, runtime) = start().await;
        let id = templates::create(&runtime)
            .unwrap()
            .receive()
            .await
            .unwrap();
        let library = |rows: Vec<Value>| templates::decode_library(&rows);
        let stored = library(execute(&runtime, templates::LIBRARY_SQL, vec![]).await);
        let created = stored
            .templates
            .iter()
            .find(|template| template.id == id)
            .unwrap();
        assert_eq!(created.title.as_ref(), "New Template");
        assert_eq!(
            created.icon,
            super::resource_list::Glyph::template_default()
        );
        templates::save(
            &runtime,
            id.clone(),
            templates::Changes {
                title: Some("Standup".into()),
                targets: Some(vec!["Engineering".into()]),
                sections: Some(vec![templates::Section {
                    title: "Blockers".into(),
                    description: "{{ notes }}".into(),
                }]),
                ..Default::default()
            },
        )
        .unwrap()
        .receive()
        .await
        .unwrap();
        let row = &execute(
            &runtime,
            "SELECT title,description,targets_json,sections_json FROM templates WHERE id=?",
            vec![json!(id)],
        )
        .await[0];
        assert_eq!(row["title"], "Standup");
        assert_eq!(row["description"], "");
        assert_eq!(row["targets_json"], r#"["Engineering"]"#);
        assert_eq!(
            row["sections_json"],
            r#"[{"title":"Blockers","description":"{{ notes }}"}]"#
        );
        // Duplicates copy stored JSON verbatim, even when it cannot be decoded.
        execute(
            &runtime,
            "UPDATE templates SET sections_json='{kept' WHERE id=?",
            vec![json!(id)],
        )
        .await;
        let copy = templates::duplicate(&runtime, id.clone())
            .unwrap()
            .receive()
            .await
            .unwrap();
        let row = &execute(
            &runtime,
            "SELECT title,pinned,sections_json FROM templates WHERE id=?",
            vec![json!(copy)],
        )
        .await[0];
        assert_eq!(row["title"], "Standup (Copy)");
        assert_eq!(row["sections_json"], "{kept");
        let updated = |runtime: &RuntimeHandle, id: Arc<str>| {
            let runtime = runtime.clone();
            async move {
                execute(
                    &runtime,
                    "SELECT pinned,pin_order,updated_at FROM templates WHERE id=?",
                    vec![json!(id)],
                )
                .await[0]
                    .clone()
            }
        };
        let before = updated(&runtime, copy.clone()).await;
        for target in [id.clone(), copy.clone()] {
            templates::toggle_favorite(&runtime, target)
                .unwrap()
                .receive()
                .await
                .unwrap();
        }
        let after = updated(&runtime, copy.clone()).await;
        assert_eq!(after["pinned"], 1);
        assert_eq!(after["pin_order"], 2);
        assert_eq!(after["updated_at"], before["updated_at"]);
        templates::toggle_favorite(&runtime, copy.clone())
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert_eq!(updated(&runtime, copy.clone()).await["pin_order"], 0);
        templates::set_default(&runtime, copy.clone())
            .unwrap()
            .receive()
            .await
            .unwrap();
        let stored = library(execute(&runtime, templates::LIBRARY_SQL, vec![]).await);
        assert_eq!(stored.default_id, copy);
        templates::delete(&runtime, copy.clone())
            .unwrap()
            .receive()
            .await
            .unwrap();
        let stored = library(execute(&runtime, templates::LIBRARY_SQL, vec![]).await);
        assert!(stored.default_id.is_empty());
        assert!(stored.templates.iter().all(|template| template.id != copy));
        runtime.shutdown().await.unwrap();
    });
}

#[test]
fn event_command_reuses_a_note_for_the_same_event() {
    block_on(async {
        let (_directory, runtime) = start().await;
        execute(&runtime,"INSERT INTO events(id,title,started_at,ended_at,participants_json) VALUES ('event','Planning','2026-09-21T10:00:00Z','2026-09-21T11:00:00Z','[{\"email\":\"person@example.com\",\"name\":\"Person\"},{\"email\":\"PERSON@example.com\"}]')",vec![]).await;
        let first = super::calendar::open_event(&runtime, "event".into(), Some("owner".into()))
            .unwrap()
            .receive()
            .await
            .unwrap();
        let again = super::calendar::open_event(&runtime, "event".into(), Some("owner".into()))
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert_eq!(first, again);
        assert_eq!(
            execute(
                &runtime,
                "SELECT owner_user_id FROM sessions WHERE id=?",
                vec![json!(first.0)]
            )
            .await[0]["owner_user_id"],
            "owner"
        );
        assert_eq!(
            execute(
                &runtime,
                "SELECT id FROM session_participants WHERE session_id=?",
                vec![json!(first.0)]
            )
            .await
            .len(),
            1
        );
        runtime.shutdown().await.unwrap();
    });
}

#[test]
fn enabled_starters_run_on_their_trigger_and_record_the_last_run() {
    block_on(async {
        let (directory, runtime) = start().await;
        let session = runtime
            .create_note("Weekly sync".into())
            .unwrap()
            .receive()
            .await
            .unwrap()
            .summary
            .id;
        let exports = directory.path().join("exports");
        execute(
            &runtime,
            "INSERT INTO app_settings(id,value_json) VALUES ('automation_markdown_export_enabled','true'),('automation_markdown_export_directory',?),('automation_slack_recap_channel',?)",
            vec![
                json!(json!(exports.to_string_lossy()).to_string()),
                json!(json!(r#"{"id":"C1","name":"eng"}"#).to_string()),
            ],
        )
        .await;
        let run = |trigger| {
            super::automation_runner::run_starters(&runtime, trigger, session.clone(), None)
                .unwrap()
                .receive()
        };
        // Slack is not enabled, so the summary trigger has nothing to run.
        assert!(
            run(super::automation_runner::Trigger::NoteEnhanced)
                .await
                .unwrap()
                .is_empty()
        );
        for _ in 0..2 {
            let messages = run(super::automation_runner::Trigger::MeetingCompleted)
                .await
                .unwrap();
            assert!(messages[0].ends_with(".md"), "{messages:?}");
        }
        let settings = automations::Settings::decode(
            &execute(&runtime, automations::SETTINGS_SQL, vec![]).await,
        );
        let last = settings
            .starter_last_run(automations::StarterId::MarkdownExport)
            .unwrap();
        assert!(last.success);
        assert_eq!(std::fs::read_dir(&exports).unwrap().count(), 1);
        runtime.shutdown().await.unwrap();
    });
}

#[test]
fn workflow_crud_executes_real_markdown_export_and_blocks_duplicate_delivery() {
    block_on(async {
        let (directory, runtime) = start().await;
        let session = runtime
            .create_note("Résumé / 日本".into())
            .unwrap()
            .receive()
            .await
            .unwrap();
        let mut workflow = Workflow::empty(None, None, "Export");
        workflow.steps = vec![Step::Markdown {
            id: "step".into(),
            directory: directory.path().join("exports").to_string_lossy().into(),
            options: None,
        }];
        workflow.enabled = true;
        let id = workflow.id.clone();
        automations::save(&runtime, workflow)
            .unwrap()
            .receive()
            .await
            .unwrap();
        let path =
            super::automation_runner::run(&runtime, id.clone(), session.summary.id.clone(), None)
                .unwrap()
                .receive()
                .await
                .unwrap();
        assert!(
            std::fs::read_to_string(path)
                .unwrap()
                .contains("Résumé / 日本")
        );
        assert!(
            super::automation_runner::run(&runtime, id.clone(), session.summary.id, None)
                .unwrap()
                .receive()
                .await
                .is_err()
        );
        let stored = execute(
            &runtime,
            "SELECT value_json FROM app_settings WHERE id='automation_workflows'",
            vec![],
        )
        .await;
        let settings = automations::Settings::decode(&[json!({
            "id": automations::WORKFLOWS,
            "value_json": stored[0]["value_json"],
        })]);
        let run = settings.workflow(&id).unwrap().last_run.clone().unwrap();
        assert!(run.success);
        automations::delete(&runtime, Some(id.clone()), None)
            .unwrap()
            .receive()
            .await
            .unwrap();
        let stored = execute(
            &runtime,
            "SELECT value_json FROM app_settings WHERE id='automation_workflows'",
            vec![],
        )
        .await;
        assert_eq!(
            super::mutations::decode_json_text(stored[0]["value_json"].as_str().unwrap()).unwrap(),
            json!([])
        );
        runtime.shutdown().await.unwrap();
    });
}

#[test]
fn calendar_lists_events_and_unlinked_notes_on_their_start_day() {
    block_on(async {
        let (_directory, runtime) = start().await;
        execute(
            &runtime,
            "INSERT INTO calendars(id,name,enabled,color) VALUES ('work','Work',1,'#4f46e5')",
            vec![],
        )
        .await;
        for (id, tracking, start, end, all_day) in [
            ("all-day", "all", "2024-02-28", "2024-03-01", 1),
            ("ignored", "ignored", "2024-02-29", "2024-03-01", 1),
            (
                "meeting",
                "timed",
                "2024-02-29T12:00:00Z",
                "2024-02-29T13:00:00Z",
                0,
            ),
            ("outside", "far", "2024-05-01T12:00:00Z", "", 0),
        ] {
            execute(&runtime,"INSERT INTO events(id,title,calendar_id,tracking_id_event,started_at,ended_at,is_all_day) VALUES (?,?,'work',?,?,?,?)",
                vec![json!(id),json!(id),json!(tracking),json!(start),json!(end),json!(all_day)]).await;
        }
        execute(
            &runtime,
            "INSERT INTO sessions(id,title,event_id,created_at) VALUES
                ('linked','Meeting','meeting','2024-02-29T12:00:00Z'),
                ('note','Standalone','','2024-02-29T15:00:00Z'),
                ('untitled','','','2024-02-29T15:00:00Z')",
            vec![],
        )
        .await;
        execute(
            &runtime,
            "INSERT INTO app_settings(id,value_json) VALUES ('ignored_events',?)",
            vec![json!(
                "[{\"tracking_id\":\"ignored\",\"last_seen\":\"2024-02-29\"}]"
            )],
        )
        .await;
        let days = month_days(NaiveDate::from_ymd_opt(2024, 2, 1).unwrap(), 1);
        let page = load_calendar(
            &runtime,
            days[0],
            *days.last().unwrap(),
            CancellationToken::new(),
        )
        .unwrap()
        .receive()
        .await
        .unwrap();
        let clock = Clock {
            use_24_hour_time: false,
            timezone: Some(chrono_tz::UTC),
        };
        let by_day = by_day(&page.items, &clock);
        let leap = by_day[&NaiveDate::from_ymd_opt(2024, 2, 29).unwrap()]
            .iter()
            .map(|item| (item.id.as_ref(), item.note, item.session.clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            leap,
            [
                ("meeting", false, Some(SessionId("linked".into()))),
                ("note", true, Some(SessionId("note".into()))),
            ]
        );
        assert_eq!(
            by_day[&NaiveDate::from_ymd_opt(2024, 2, 28).unwrap()][0]
                .id
                .as_ref(),
            "all-day"
        );
        assert!(!page.items.iter().any(|item| item.id.as_ref() == "outside"));
        assert!(!by_day.contains_key(&NaiveDate::from_ymd_opt(2024, 3, 1).unwrap()));
        runtime.shutdown().await.unwrap();
    });
}

#[test]
fn deleting_an_event_or_its_series_hides_it_from_the_timeline() {
    use anlg_db_app::IgnoredCalendarItemKind;
    block_on(async {
        let (_directory, runtime) = start().await;
        for (id, tracking, series, offset) in [
            ("once", "t-once", "", "+1 hours"),
            ("weekly", "t-weekly", "s-weekly", "+2 hours"),
            ("weekly-next", "t-weekly-next", "s-weekly", "+3 hours"),
        ] {
            execute(
                &runtime,
                "INSERT INTO events(id,title,calendar_id,tracking_id_event,recurrence_series_id,started_at,ended_at)
                VALUES (?1,?1,'work',?2,?3,strftime('%Y-%m-%dT%H:%M:%SZ','now',?4),
                    strftime('%Y-%m-%dT%H:%M:%SZ','now',?4,'+1 hours'))",
                vec![json!(id), json!(tracking), json!(series), json!(offset)],
            )
            .await;
        }
        let events = || async {
            super::timeline::load_events(&runtime, CancellationToken::new())
                .unwrap()
                .receive()
                .await
                .unwrap()
                .events
        };
        let first = events().await;
        assert_eq!(
            (first[0].tracking.as_ref(), first[1].series.as_ref()),
            ("t-once", "s-weekly")
        );
        let ignore = |kind, id: &str| super::timeline::ignore(&runtime, kind, id.into());
        ignore(IgnoredCalendarItemKind::Events, "t-once")
            .unwrap()
            .receive()
            .await
            .unwrap();
        let ids = events()
            .await
            .iter()
            .map(|event| event.id.to_string())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["weekly", "weekly-next"]);
        ignore(IgnoredCalendarItemKind::Series, "s-weekly")
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert!(events().await.is_empty());
        runtime.shutdown().await.unwrap();
    });
}
