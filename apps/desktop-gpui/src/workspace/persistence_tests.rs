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
    calendar::{Clock, by_day, load_calendar, month_days},
    folders,
    mutations::{self, Command},
    navigation::Route,
    notes::{self, NoteCommand},
    picker::PICKER_QUERY,
    pins,
    ports::{Catalog, CatalogQuery, catalog_detail, catalog_page, decode_catalog},
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
fn contacts_prefer_authenticated_self_and_keep_self_visible_during_search() {
    block_on(async {
        let (_directory, runtime) = start().await;
        execute(&runtime,"INSERT INTO humans(id,name) VALUES ('signed-in','Signed in self'),('00000000-0000-0000-0000-000000000000','Local self'),('other','Another person')",vec![]).await;
        let query = CatalogQuery {
            catalog: Catalog::Contacts,
            search: "does not match".into(),
            offset: 0,
        };
        let (sql, params) = query.sql_for_viewer(Some("signed-in"));
        let page = decode_catalog(&execute(&runtime, &sql, params).await).unwrap();
        assert_eq!(page.rows[0].id.as_ref(), "signed-in");
        assert!(page.rows[0].self_contact);
        assert_eq!(
            page.rows[1].id.as_ref(),
            "00000000-0000-0000-0000-000000000000"
        );
        assert_eq!(page.rows.len(), 2);
        let (sql, params) = query.sql();
        let page = decode_catalog(&execute(&runtime, &sql, params).await).unwrap();
        assert_eq!(page.rows.len(), 1);
        assert!(page.rows[0].self_contact);
        runtime.shutdown().await.unwrap();
    });
}

#[test]
fn picker_scopes_shared_cache_to_viewer_and_deduplicates_managed_local_notes() {
    block_on(async {
        let (_directory, runtime) = start().await;
        execute(&runtime,"INSERT INTO sessions(id,title,created_at) VALUES ('local','Local','2024-01-01'),('recent','Recent','2023-01-01')",vec![]).await;
        for (share, viewer, session, managed) in [
            ("duplicate", "viewer", "local", 1),
            ("shared", "viewer", "remote", 0),
            ("other-account", "other", "remote2", 0),
        ] {
            execute(&runtime,"INSERT INTO shared_session_cache(share_id,viewer_user_id,workspace_id,session_id,content_revision,access_version,published_at,title,manage_access) VALUES (?,?,'workspace',?,1,1,'2025-01-01',?,?)",
                vec![json!(share),json!(viewer),json!(session),json!(share),json!(managed)]).await;
        }
        let rows = execute(
            &runtime,
            PICKER_QUERY,
            vec![json!("%"), json!("viewer"), json!("[\"recent\"]")],
        )
        .await;
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0]["id"], "recent");
        assert_eq!(rows[1]["kind"], "shared");
        assert_eq!(rows[1]["id"], "shared");
        assert_eq!(rows[2]["id"], "local");
        let signed_out = execute(
            &runtime,
            PICKER_QUERY,
            vec![json!("%"), Value::Null, json!("[]")],
        )
        .await;
        assert_eq!(signed_out.len(), 2);
        assert!(signed_out.iter().all(|row| row["kind"] == "session"));
        execute(
            &runtime,
            "UPDATE sessions SET deleted_at='deleted' WHERE id='local'",
            vec![],
        )
        .await;
        let rows = execute(
            &runtime,
            PICKER_QUERY,
            vec![json!("%"), json!("viewer"), json!("[]")],
        )
        .await;
        assert!(rows.iter().any(|row| row["id"] == "duplicate"));
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
        let query = CatalogQuery {
            catalog: Catalog::Automations,
            search: "".into(),
            offset: 0,
        };
        assert!(
            catalog_page(&runtime, query, CancellationToken::new())
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
        let mut draft = mutations::blank(Catalog::Folders);
        edit(&mut draft, "path", "Research");
        mutations::dispatch(&runtime, Command::Save(draft), None)
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
        let mut draft = mutations::blank(Catalog::Automations);
        edit(&mut draft, "enabled", "true");
        edit(
            &mut draft,
            "steps",
            &json!([
                {"type":"slack_recap","target":{"id":"channel","name":"Channel"}},
                {"type":"notion_update","target":{"id":"page","name":"Page"}},
                {"type":"linear_issues","target":{"id":"team","name":"Team"}}
            ])
            .to_string(),
        );
        let row = mutations::dispatch(&runtime, Command::Save(draft), None)
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
        super::automation_runner::run(&runtime, row.id.clone(), session.clone(), Some(client))
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
            super::ports::decode_json_text(stored[0]["value_json"].as_str().unwrap()).unwrap();
        assert_eq!(workflows[0]["lastRun"]["status"], "success");
        super::automation_runner::allow_retry(&runtime, row.id.clone(), session.clone())
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
            super::automation_runner::run(&runtime, row.id, session.clone(), Some(client))
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
            super::ports::decode_json_text(stored[0]["value_json"].as_str().unwrap()).unwrap();
        assert_eq!(workflows[0]["lastRun"]["status"], "error");
        assert!(super::automations::already_processed(
            &workflows[0],
            &session.0
        ));
        runtime.shutdown().await.unwrap();
    });
}

fn edit(draft: &mut super::mutations::Draft, key: &str, value: &str) {
    draft
        .fields
        .iter_mut()
        .find(|field| field.key == key)
        .unwrap()
        .value = value.into();
}

async fn reload_draft(
    runtime: &RuntimeHandle,
    row: super::ports::CatalogRow,
) -> super::mutations::Draft {
    super::mutations::load(runtime, row, CancellationToken::new())
        .unwrap()
        .receive()
        .await
        .unwrap()
}

#[test]
fn catalog_commands_persist_contact_merge_and_conflicts_without_losing_extensions() {
    block_on(async {
        let (_directory, runtime) = start().await;
        let mut draft = mutations::blank(Catalog::Contacts);
        edit(&mut draft, "name", "안녕 Café");
        edit(&mut draft, "email", "person@example.com");
        let row = mutations::dispatch(&runtime, Command::Save(draft), None)
            .unwrap()
            .receive()
            .await
            .unwrap();
        let draft = reload_draft(&runtime, row.clone()).await;
        assert_eq!(draft.fields[0].value, "안녕 Café");
        let mut changed = draft.clone();
        edit(&mut changed, "name", "Renamed");
        mutations::dispatch(&runtime, Command::Save(changed), None)
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert!(matches!(
            mutations::dispatch(&runtime, Command::Save(draft), None)
                .unwrap()
                .receive()
                .await,
            Err(ServiceError::Conflict)
        ));
        execute(&runtime,"INSERT INTO humans(id,name,phone,metadata_json) VALUES ('duplicate','Duplicate','123','{\"unknown\":true}')",vec![]).await;
        execute(&runtime,"INSERT INTO session_participants(id,session_id,human_id) VALUES ('one','s',?),('two','s','duplicate'),('three','other','duplicate')",vec![json!(row.id)]).await;
        let primary = reload_draft(&runtime, row.clone()).await;
        mutations::dispatch(
            &runtime,
            Command::Merge {
                primary,
                duplicate: "duplicate".into(),
            },
            None,
        )
        .unwrap()
        .receive()
        .await
        .unwrap();
        let rows = execute(
            &runtime,
            "SELECT * FROM session_participants WHERE deleted_at IS NULL",
            vec![],
        )
        .await;
        assert_eq!(rows.len(), 2);
        assert!(
            rows.iter()
                .all(|entry| entry["human_id"] == row.id.as_ref())
        );
        assert_eq!(
            execute(
                &runtime,
                "SELECT phone FROM humans WHERE id=?",
                vec![json!(row.id)]
            )
            .await[0]["phone"],
            "123"
        );
        assert_eq!(
            execute(
                &runtime,
                "SELECT metadata_json FROM humans WHERE id='duplicate'",
                vec![]
            )
            .await[0]["metadata_json"],
            "{\"unknown\":true}"
        );
        execute(
            &runtime,
            "INSERT INTO humans(id,name) VALUES ('00000000-0000-0000-0000-000000000000','Me')",
            vec![],
        )
        .await;
        let mut owner = row.clone();
        owner.id = "00000000-0000-0000-0000-000000000000".into();
        let owner = reload_draft(&runtime, owner).await;
        assert!(
            mutations::dispatch(&runtime, Command::Delete(owner), None)
                .unwrap()
                .receive()
                .await
                .is_err()
        );
        runtime.shutdown().await.unwrap();
    });
}

#[test]
fn template_commands_duplicate_raw_json_and_keep_conflicting_edits() {
    block_on(async {
        let (_directory, runtime) = start().await;
        let mut draft = mutations::blank(Catalog::Templates);
        edit(&mut draft, "title", "Interview");
        edit(
            &mut draft,
            "sections_json",
            r#"[{"title":"Agenda","extension":{"emoji":"世界"}}]"#,
        );
        let row = mutations::dispatch(&runtime, Command::Save(draft), None)
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
            row.id.clone(),
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
        assert_eq!(memo[0]["template_id"], row.id.as_ref());
        assert_eq!(memo[0]["body"], "original");
        let draft = reload_draft(&runtime, row.clone()).await;
        let duplicate = mutations::dispatch(&runtime, Command::Duplicate(draft), None)
            .unwrap()
            .receive()
            .await
            .unwrap();
        let data = execute(
            &runtime,
            "SELECT * FROM templates WHERE id=?",
            vec![json!(duplicate.id)],
        )
        .await;
        assert_eq!(data[0]["title"], "Interview (Copy)");
        assert_eq!(
            data[0]["sections_json"],
            r#"[{"title":"Agenda","extension":{"emoji":"世界"}}]"#
        );
        let draft = reload_draft(&runtime, duplicate.clone()).await;
        mutations::dispatch(&runtime, Command::Pin(draft), None)
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert_eq!(
            execute(
                &runtime,
                "SELECT pinned FROM templates WHERE id=?",
                vec![json!(duplicate.id)]
            )
            .await[0]["pinned"],
            1
        );
        let draft = reload_draft(&runtime, duplicate.clone()).await;
        for expected in [duplicate.id.as_ref(), "", duplicate.id.as_ref()] {
            mutations::dispatch(&runtime, Command::SelectDefault(draft.clone()), None)
                .unwrap()
                .receive()
                .await
                .unwrap();
            let settings = execute(
                &runtime,
                "SELECT value_json FROM app_settings WHERE id='selected_template_id'",
                vec![],
            )
            .await;
            assert_eq!(
                serde_json::from_str::<String>(settings[0]["value_json"].as_str().unwrap())
                    .unwrap(),
                expected
            );
        }
        mutations::dispatch(&runtime, Command::Delete(draft), None)
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert_eq!(
            execute(
                &runtime,
                "SELECT value_json FROM app_settings WHERE id='selected_template_id'",
                vec![]
            )
            .await[0]["value_json"],
            "\"\""
        );
        assert!(
            execute(
                &runtime,
                "SELECT id FROM templates WHERE id=?",
                vec![json!(duplicate.id)]
            )
            .await
            .is_empty()
        );
        assert_eq!(
            execute(
                &runtime,
                "SELECT id FROM templates WHERE id=?",
                vec![json!(row.id)]
            )
            .await
            .len(),
            1
        );
        runtime.shutdown().await.unwrap();
    });
}

#[test]
fn folder_notes_keep_normalized_live_tag_metadata() {
    block_on(async {
        let (_directory, runtime) = start().await;
        execute(
            &runtime,
            "INSERT INTO sessions(id,title,folder_path) VALUES ('tagged','Tagged','Work/日本')",
            vec![],
        )
        .await;
        execute(
            &runtime,
            "INSERT INTO tags(id,name) VALUES ('tag',' release, prep ')",
            vec![],
        )
        .await;
        execute(
            &runtime,
            "INSERT INTO session_tags(id,session_id,tag_id) VALUES ('link','tagged','tag')",
            vec![],
        )
        .await;
        let page = folders::notes(&runtime, "Work".into(), 0, CancellationToken::new())
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].tag_line.as_ref(), "#release, prep");
        execute(
            &runtime,
            "UPDATE tags SET deleted_at='2026-09-21' WHERE id='tag'",
            vec![],
        )
        .await;
        let page = folders::notes(&runtime, "Work".into(), 0, CancellationToken::new())
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert_eq!(page.items[0].title.as_ref(), "Tagged");
        assert!(page.items[0].tag_line.is_empty());
        runtime.shutdown().await.unwrap();
    });
}

#[test]
fn folders_and_note_commands_move_nested_paths_and_soft_delete_related_rows() {
    block_on(async {
        let (directory, runtime) = start().await;
        let mut draft = mutations::blank(Catalog::Folders);
        edit(&mut draft, "path", "Clients/日本");
        let row = mutations::dispatch(&runtime, Command::Save(draft), None)
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
        let mut draft = reload_draft(&runtime, row.clone()).await;
        edit(&mut draft, "path", "Projects/日本");
        mutations::dispatch(&runtime, Command::Save(draft), None)
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
        let draft = reload_draft(&runtime, row).await;
        mutations::dispatch(&runtime, Command::Delete(draft), None)
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
fn selected_folder_commands_collapse_nested_roots_and_keep_notes_on_delete() {
    block_on(async {
        let (_directory, runtime) = start().await;
        let mut ids = Vec::<Arc<str>>::new();
        for path in ["One", "One/Child", "Two"] {
            let mut draft = mutations::blank(Catalog::Folders);
            edit(&mut draft, "path", path);
            let row = mutations::dispatch(&runtime, Command::Save(draft), None)
                .unwrap()
                .receive()
                .await
                .unwrap();
            ids.push(row.id);
        }
        execute(
            &runtime,
            "INSERT INTO sessions(id,title,folder_path) VALUES ('note','Retained','One/Child')",
            vec![],
        )
        .await;
        execute(
            &runtime,
            "INSERT INTO sessions(id,title,folder_path) VALUES ('other','Outside','One2')",
            vec![],
        )
        .await;
        let notes = super::folders::notes(&runtime, "One".into(), 0, CancellationToken::new())
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert_eq!(notes.items.len(), 1);
        assert_eq!(notes.items[0].id.0.as_ref(), "note");
        assert!(
            super::folders::notes(&runtime, "On%".into(), 0, CancellationToken::new())
                .unwrap()
                .receive()
                .await
                .unwrap()
                .items
                .is_empty()
        );
        assert!(
            super::folders::batch(&runtime, ids.clone().into(), Some("One/Child".into()))
                .unwrap()
                .receive()
                .await
                .is_err()
        );
        assert_eq!(
            super::folders::batch(&runtime, ids.clone().into(), Some("Archive".into()))
                .unwrap()
                .receive()
                .await
                .unwrap(),
            "Moved 2 folder roots"
        );
        assert_eq!(
            execute(
                &runtime,
                "SELECT folder_path FROM sessions WHERE id='note'",
                vec![]
            )
            .await[0]["folder_path"],
            "Archive/One/Child"
        );
        assert_eq!(
            super::folders::batch(&runtime, ids.into(), None)
                .unwrap()
                .receive()
                .await
                .unwrap(),
            "Deleted 2 folder roots"
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
fn event_command_reuses_a_note_and_search_finds_local_transcript_and_scoped_shared_content() {
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
        execute(&runtime,"INSERT INTO transcripts(id,session_id,words_json) VALUES ('transcript',?,'[{\"text\":\"검색 transcript\"}]')",vec![json!(first.0)]).await;
        execute(&runtime,"INSERT INTO shared_session_cache(share_id,viewer_user_id,workspace_id,session_id,content_revision,access_version,published_at,title,body_json) VALUES ('shared','viewer','workspace','remote',1,1,'2026-09-21','Shared','{\"type\":\"doc\",\"content\":[{\"type\":\"paragraph\",\"content\":[{\"type\":\"text\",\"text\":\"검색 secret\"}]}]}')",vec![]).await;
        let search = super::search::SearchEngine::default();
        let results = search
            .query(
                &runtime,
                "검색".into(),
                Some("viewer".into()),
                CancellationToken::new(),
            )
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert_eq!(results.hits.len(), 2);
        let results = search
            .query(&runtime, "검색".into(), None, CancellationToken::new())
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert_eq!(results.hits.len(), 1);
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            search
                .query(&runtime, "anything".into(), None, cancel)
                .unwrap()
                .receive()
                .await,
            Err(ServiceError::Cancelled)
        ));
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
        let mut draft = mutations::blank(Catalog::Automations);
        edit(&mut draft, "enabled", "true");
        edit(&mut draft,"steps",&json!([{"id":"step","type":"markdown_export","directory":directory.path().join("exports"),"unknown":"retained"}]).to_string());
        let row = mutations::dispatch(&runtime, Command::Save(draft), None)
            .unwrap()
            .receive()
            .await
            .unwrap();
        let path = super::automation_runner::run(
            &runtime,
            row.id.clone(),
            session.summary.id.clone(),
            None,
        )
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
            super::automation_runner::run(&runtime, row.id.clone(), session.summary.id, None)
                .unwrap()
                .receive()
                .await
                .is_err()
        );
        let draft = reload_draft(&runtime, row.clone()).await;
        let copy = mutations::dispatch(&runtime, Command::Duplicate(draft), None)
            .unwrap()
            .receive()
            .await
            .unwrap();
        let copy_draft = reload_draft(&runtime, copy.clone()).await;
        let settings = super::ports::decode_json_text(
            copy_draft.base.as_ref().unwrap()["value_json"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        let copy_value = settings
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == copy.id.as_ref())
            .unwrap();
        assert_eq!(copy_value["processedSessionIds"], json!([]));
        assert_eq!(copy_value["steps"][0]["unknown"], "retained");
        mutations::dispatch(&runtime, Command::Delete(copy_draft), None)
            .unwrap()
            .receive()
            .await
            .unwrap();
        runtime.shutdown().await.unwrap();
    });
}

#[test]
fn catalogs_read_canonical_schema_and_preserve_malformed_template() {
    block_on(async {
        let (_directory, runtime) = start().await;
        execute(&runtime,"INSERT INTO humans(id,name,email) VALUES ('human','Person','p@example.com'),('00000000-0000-0000-0000-000000000000','Me','')",vec![]).await;
        execute(
            &runtime,
            "INSERT INTO organizations(id,name,pinned,pin_order) VALUES ('org','Company',1,0)",
            vec![],
        )
        .await;
        execute(&runtime,"INSERT INTO folders(id,path,instructions) VALUES ('folder','Clients/100%_complete','Original instructions')",vec![]).await;
        execute(&runtime,"INSERT INTO templates(id,title,sections_json,targets_json) VALUES ('template','Damaged template','{broken','\"Team\"')",vec![]).await;
        execute(&runtime,"INSERT INTO app_settings(id,value_json) VALUES ('automation_workflows',?)",vec![json!(serde_json::to_string(&json!([{
            "id":"workflow","title":"Export","enabled":false,"trigger":"meeting_completed","steps":[{"type":"markdown_export","directory":"/fixture/export","options":{"include_memo":false}}],
            "lastRun":null,"processedSessionIds":[],"chatGroupId":"chat"
        }])).unwrap())]).await;
        for catalog in [
            Catalog::Contacts,
            Catalog::Folders,
            Catalog::Templates,
            Catalog::Automations,
        ] {
            let page = catalog_page(
                &runtime,
                CatalogQuery {
                    catalog,
                    search: "".into(),
                    offset: 0,
                },
                CancellationToken::new(),
            )
            .unwrap()
            .receive()
            .await
            .unwrap();
            assert!(!page.rows.is_empty(), "{catalog:?}");
            for row in page.rows.iter() {
                let detail = catalog_detail(&runtime, row.clone(), CancellationToken::new())
                    .unwrap()
                    .receive()
                    .await
                    .unwrap();
                assert!(!detail.title.is_empty(), "{catalog:?}");
                if row.id.as_ref() == "template" {
                    assert!(!detail.warnings.is_empty());
                }
            }
            if catalog == Catalog::Contacts {
                assert!(page.rows[0].self_contact);
                assert_eq!(page.rows[1].id.as_ref(), "org");
            }
        }
        let page = catalog_page(
            &runtime,
            CatalogQuery {
                catalog: Catalog::Folders,
                search: "100%_".into(),
                offset: 0,
            },
            CancellationToken::new(),
        )
        .unwrap()
        .receive()
        .await
        .unwrap();
        assert_eq!(page.rows.len(), 1);
        let rows = execute(
            &runtime,
            "SELECT sections_json FROM templates WHERE id='template'",
            vec![],
        )
        .await;
        assert_eq!(rows[0]["sections_json"], "{broken");
        execute(
            &runtime,
            "UPDATE humans SET deleted_at = 'removed' WHERE id='human'",
            vec![],
        )
        .await;
        let page = catalog_page(
            &runtime,
            CatalogQuery {
                catalog: Catalog::Contacts,
                search: "".into(),
                offset: 0,
            },
            CancellationToken::new(),
        )
        .unwrap()
        .receive()
        .await
        .unwrap();
        assert!(!page.rows.iter().any(|row| row.id.as_ref() == "human"));
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
fn catalog_page_is_bounded_and_cancelled_reads_cannot_succeed() {
    block_on(async {
        let (_directory, runtime) = start().await;
        execute(&runtime,"WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<150) INSERT INTO folders(id,path) SELECT 'id-'||x, printf('Folder-%03d',x) FROM n",vec![]).await;
        let query = CatalogQuery {
            catalog: Catalog::Folders,
            search: "".into(),
            offset: 0,
        };
        let page = catalog_page(&runtime, query.clone(), CancellationToken::new())
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert_eq!(page.rows.len(), 100);
        assert!(page.has_more);
        let next = catalog_page(
            &runtime,
            CatalogQuery {
                offset: 100,
                ..query.clone()
            },
            CancellationToken::new(),
        )
        .unwrap()
        .receive()
        .await
        .unwrap();
        assert_eq!(next.rows.len(), 50);
        assert!(!next.has_more);
        assert_ne!(page.rows[99].id, next.rows[0].id);
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            catalog_page(&runtime, query, cancel)
                .unwrap()
                .receive()
                .await,
            Err(ServiceError::Cancelled)
        ));
        runtime.shutdown().await.unwrap();
    });
}
