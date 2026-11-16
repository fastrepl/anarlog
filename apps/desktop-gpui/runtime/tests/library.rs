use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use desktop_runtime::{
    CancellationToken, Generation, LibraryQuery, Profile, QUEUE_CAPACITY, RenameSession,
    RuntimeHandle, SaveDocument, ServiceError,
};
use serde_json::json;
use sqlx::{Connection, sqlite::SqliteConnectOptions};
use tokio::sync::oneshot;

async fn start(profile: &Profile) -> RuntimeHandle {
    let (runtime, ready) = RuntimeHandle::start(profile.clone()).unwrap();
    ready.receive().await.unwrap();
    runtime
}

#[tokio::test]
async fn tags_refresh_cached_pages_and_watch_without_modifying_notes() {
    let directory = tempfile::tempdir().unwrap();
    let profile = Profile {
        database: directory.path().join("tags.sqlite"),
    };
    let runtime = start(&profile).await;
    runtime.submit(|services| async move {
        for sql in [
            "INSERT INTO sessions(id,title,created_at) VALUES ('one','First','2026-09-02T00:00:00Z'),('two','Second','2026-09-01T00:00:00Z')",
            "INSERT INTO tags(id,name,deleted_at) VALUES ('a','  prep  ',NULL),('b','launch, prep',NULL),('c','prep',NULL),('d','deleted','2026-09-01'),('e','unlinked',NULL)",
            "INSERT INTO session_tags(id,session_id,tag_id,deleted_at) VALUES ('a','one','a',NULL),('b','one','b',NULL),('c','one','c',NULL),('d','one','d',NULL),('e','one','e','2026-09-01'),('f','two','b',NULL)",
        ] {
            services.executor.execute(sql.into(), vec![]).await.unwrap();
        }
        Ok(())
    }).unwrap().receive().await.unwrap();
    for (offset, expected) in [(0, "#launch, prep #prep"), (1, "#launch, prep")] {
        let page = runtime
            .library(
                LibraryQuery {
                    offset,
                    limit: 1,
                    ..Default::default()
                },
                CancellationToken::new(),
            )
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert_eq!(page.items[0].tag_line.as_ref(), expected);
    }
    let mut watch = runtime.watch_library().unwrap().receive().await.unwrap();
    watch.snapshots.borrow_and_update();
    for (sql, expected) in [
        (
            "UPDATE tags SET name='a renamed' WHERE id='b'",
            "#a renamed #prep",
        ),
        (
            "UPDATE session_tags SET deleted_at='2026-09-02' WHERE id='a'",
            "#a renamed #prep",
        ),
        (
            "UPDATE tags SET deleted_at='2026-09-02' WHERE id='c'",
            "#a renamed",
        ),
        ("DELETE FROM session_tags WHERE id='b'", ""),
    ] {
        runtime
            .submit(move |services| async move {
                services.executor.execute(sql.into(), vec![]).await.unwrap();
                Ok(())
            })
            .unwrap()
            .receive()
            .await
            .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), watch.snapshots.changed())
            .await
            .unwrap()
            .unwrap();
        watch.snapshots.borrow_and_update();
        let page = runtime
            .library(LibraryQuery::default(), CancellationToken::new())
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert_eq!(page.items[0].tag_line.as_ref(), expected);
        assert_eq!(page.items[0].title.as_ref(), "First");
    }
    watch.unsubscribe().await.unwrap();
    runtime.shutdown().await.unwrap();
    let runtime = start(&profile).await;
    let page = runtime
        .library(
            LibraryQuery {
                search: "Second".into(),
                ..Default::default()
            },
            CancellationToken::new(),
        )
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].tag_line.as_ref(), "#a renamed");
    let open = runtime
        .open_session(page.items[0].id.clone(), CancellationToken::new())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(open.summary.tag_line, page.items[0].tag_line);
    assert!(open.note.is_none());
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn library_folder_projection_survives_cache_slicing_and_external_moves() {
    let directory = tempfile::tempdir().unwrap();
    let profile = Profile {
        database: directory.path().join("folders.sqlite"),
    };
    let runtime = start(&profile).await;
    let first = runtime
        .create_note("First".into())
        .unwrap()
        .receive()
        .await
        .unwrap();
    let second = runtime
        .create_note("Second".into())
        .unwrap()
        .receive()
        .await
        .unwrap();
    let mut external = sqlx::SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(&profile.database),
    )
    .await
    .unwrap();
    for (session, date, folder) in [
        (&first, "2026-09-02T00:00:00Z", "Work/日本語"),
        (&second, "2026-09-01T00:00:00Z", "Personal"),
    ] {
        sqlx::query("UPDATE sessions SET folder_path=?,created_at=? WHERE id=?")
            .bind(folder)
            .bind(date)
            .bind(session.summary.id.0.as_ref())
            .execute(&mut external)
            .await
            .unwrap();
    }
    for (offset, session, folder) in [(0, &first, "Work/日本語"), (1, &second, "Personal")] {
        let page = runtime
            .library(
                LibraryQuery {
                    offset,
                    limit: 1,
                    ..Default::default()
                },
                CancellationToken::new(),
            )
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, session.summary.id);
        assert_eq!(page.items[0].folder_path.as_ref(), folder);
        assert_eq!(page.has_more, offset == 0);
    }
    sqlx::query("UPDATE sessions SET folder_path=? WHERE id=?")
        .bind("Archive/日本語")
        .bind(first.summary.id.0.as_ref())
        .execute(&mut external)
        .await
        .unwrap();
    let page = runtime
        .library(LibraryQuery::default(), CancellationToken::new())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(page.items[0].folder_path.as_ref(), "Archive/日本語");
    external.close().await.unwrap();
    runtime.shutdown().await.unwrap();

    let runtime = start(&profile).await;
    let reopened = runtime
        .open_session(first.summary.id, CancellationToken::new())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(reopened.summary.folder_path.as_ref(), "Archive/日本語");
    assert_eq!(reopened.summary.title, first.summary.title);
    assert_eq!(reopened.note.unwrap().body, first.note.unwrap().body);
    let page = runtime
        .library(
            LibraryQuery {
                search: "First".into(),
                ..Default::default()
            },
            CancellationToken::new(),
        )
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].folder_path.as_ref(), "Archive/日本語");
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn native_notes_share_the_shipping_editors_document() {
    let directory = tempfile::tempdir().unwrap();
    let profile = Profile {
        database: directory.path().join("shared.sqlite"),
    };
    let runtime = start(&profile).await;
    let session = runtime
        .create_note("Cross-client 日本語".into())
        .unwrap()
        .receive()
        .await
        .unwrap();
    let native_body: Arc<str> = r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Native 日本語 🚀"}]}]}"#.into();
    let saved = runtime
        .save_document(SaveDocument {
            base: session.note.unwrap(),
            body: native_body.clone(),
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    let shipping_select = include_str!("../../../desktop/src/session/queries/sessions.ts")
        .split_once("const SESSION_SELECT_SQL = `")
        .unwrap()
        .1
        .split_once("`;")
        .unwrap()
        .0;
    let id = session.summary.id.clone();
    let rows = runtime
        .submit(move |services| async move {
            services
                .executor
                .execute(shipping_select.into(), vec![json!(id)])
                .await
                .map_err(|e| ServiceError::Failed(e.to_string().into()))
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(rows[0]["raw_body"], native_body.as_ref());
    assert_eq!(rows[0]["raw_body_format"], "prosemirror_json");
    assert_eq!(saved.id.0, session.summary.id.0);

    let external_body = r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Tauri 日本語 🚀"}]}]}"#;
    let mut other = sqlx::SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(&profile.database),
    )
    .await
    .unwrap();
    let written = sqlx::query(
        "UPDATE session_documents SET body=?, updated_at='2026-09-21T12:00:00.000Z' WHERE id=?",
    )
    .bind(external_body)
    .bind(session.summary.id.0.as_ref())
    .execute(&mut other)
    .await
    .unwrap();
    assert_eq!(written.rows_affected(), 1);
    assert!(matches!(
        runtime
            .save_document(SaveDocument {
                base: saved,
                body: native_body
            })
            .unwrap()
            .receive()
            .await,
        Err(ServiceError::Conflict)
    ));
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM session_documents WHERE session_id=? AND kind='note'",
    )
    .bind(session.summary.id.0.as_ref())
    .fetch_one(&mut other)
    .await
    .unwrap();
    assert_eq!(count, 1);
    other.close().await.unwrap();
    runtime.shutdown().await.unwrap();

    let runtime = start(&profile).await;
    let reopened = runtime
        .open_session(session.summary.id, CancellationToken::new())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(reopened.note.unwrap().body.as_ref(), external_body);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn markdown_notes_save_edits_as_prosemirror_json() {
    let directory = tempfile::tempdir().unwrap();
    let profile = Profile {
        database: directory.path().join("markdown.sqlite"),
    };
    let runtime = start(&profile).await;
    let session = runtime
        .create_note("Imported".into())
        .unwrap()
        .receive()
        .await
        .unwrap();
    let id = session.summary.id.clone();
    runtime
        .submit(move |services| async move {
            services
                .executor
                .execute(
                    "UPDATE session_documents SET body_format = 'markdown', body = '## Agenda' WHERE session_id = ?".into(),
                    vec![json!(id)],
                )
                .await
                .map_err(|e| ServiceError::Failed(e.to_string().into()))
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    let base = runtime
        .open_session(session.summary.id.clone(), CancellationToken::new())
        .unwrap()
        .receive()
        .await
        .unwrap()
        .note
        .unwrap();
    assert_eq!(base.body_format.as_ref(), "markdown");
    let body: Arc<str> = r#"{"type":"doc","content":[{"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"Agenda!"}]}]}"#.into();
    let saved = runtime
        .save_document(SaveDocument {
            base,
            body: body.clone(),
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(saved.body_format.as_ref(), "prosemirror_json");
    assert_eq!(saved.body, body);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn library_cache_observes_other_connections_and_rollbacks() {
    let directory = tempfile::tempdir().unwrap();
    let profile = Profile {
        database: directory.path().join("cache.sqlite"),
    };
    let runtime = start(&profile).await;
    let note = runtime
        .create_note("Before".into())
        .unwrap()
        .receive()
        .await
        .unwrap();
    let read = || {
        runtime
            .library(LibraryQuery::default(), CancellationToken::new())
            .unwrap()
    };
    assert_eq!(
        read().receive().await.unwrap().items[0].title.as_ref(),
        "Before"
    );
    let mut other = sqlx::SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(&profile.database),
    )
    .await
    .unwrap();
    sqlx::query("BEGIN").execute(&mut other).await.unwrap();
    sqlx::query("UPDATE sessions SET title='Uncommitted' WHERE id=?")
        .bind(note.summary.id.0.as_ref())
        .execute(&mut other)
        .await
        .unwrap();
    assert_eq!(
        read().receive().await.unwrap().items[0].title.as_ref(),
        "Before"
    );
    sqlx::query("ROLLBACK").execute(&mut other).await.unwrap();
    assert_eq!(
        read().receive().await.unwrap().items[0].title.as_ref(),
        "Before"
    );
    sqlx::query("UPDATE sessions SET title='External 日本語' WHERE id=?")
        .bind(note.summary.id.0.as_ref())
        .execute(&mut other)
        .await
        .unwrap();
    assert_eq!(
        read().receive().await.unwrap().items[0].title.as_ref(),
        "External 日本語"
    );
    runtime
        .create_note("Second".into())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(read().receive().await.unwrap().items.len(), 2);
    let page = runtime
        .library(
            LibraryQuery {
                offset: 200,
                ..LibraryQuery::default()
            },
            CancellationToken::new(),
        )
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert!(page.items.is_empty() && !page.has_more);
    other.close().await.unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn durable_roundtrip_opaque_json_and_conflicting_writes() {
    let directory = tempfile::tempdir().unwrap();
    let profile = Profile {
        database: directory.path().join("library.sqlite"),
    };
    let runtime = start(&profile).await;
    let session = runtime
        .create_note("日本語 😀".into())
        .unwrap()
        .receive()
        .await
        .unwrap();
    let first = runtime
        .library(LibraryQuery::default(), CancellationToken::new())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(first.items.len(), 1);
    let base = session.note.unwrap();
    let raw: Arc<str> = r#" { "type":"doc", "unknown": [1, 2], "content":[{"type":"custom","attrs":{"x":"😀"}}] } "#.into();
    let saved = runtime
        .save_document(SaveDocument {
            base: base.clone(),
            body: raw.clone(),
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(saved.body, raw);
    assert!(matches!(
        runtime
            .save_document(SaveDocument {
                base,
                body: r#"{"type":"doc"}"#.into()
            })
            .unwrap()
            .receive()
            .await,
        Err(ServiceError::Conflict)
    ));
    let renamed = runtime
        .rename_session(RenameSession {
            base: session.summary.clone(),
            title: "Updated".into(),
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert!(matches!(
        runtime
            .rename_session(RenameSession {
                base: session.summary,
                title: "Lost update".into()
            })
            .unwrap()
            .receive()
            .await,
        Err(ServiceError::Conflict)
    ));
    runtime.shutdown().await.unwrap();

    let runtime = start(&profile).await;
    let restored = runtime
        .open_session(renamed.summary.id, CancellationToken::new())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(restored.summary.title.as_ref(), "Updated");
    assert_eq!(restored.note.unwrap().body, raw);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn failures_preserve_original_and_deleted_documents_cannot_be_saved() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = start(&Profile {
        database: directory.path().join("library.sqlite"),
    })
    .await;
    let session = runtime
        .create_note("Preserve".into())
        .unwrap()
        .receive()
        .await
        .unwrap();
    let document = session.note.unwrap();
    assert!(
        runtime
            .save_document(SaveDocument {
                base: document.clone(),
                body: "malformed".into()
            })
            .unwrap()
            .receive()
            .await
            .is_err()
    );
    let loaded = runtime
        .open_session(session.summary.id.clone(), CancellationToken::new())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(loaded.note.unwrap().body, document.body);
    let id = document.id.clone();
    runtime
        .submit(move |services| async move {
            services
                .executor
                .execute(
                    "UPDATE session_documents SET deleted_at = 'deleted' WHERE id = ?".into(),
                    vec![json!(id)],
                )
                .await
                .map_err(|e| ServiceError::Failed(e.to_string().into()))?;
            Ok(())
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert!(matches!(
        runtime
            .save_document(SaveDocument {
                base: document,
                body: r#"{"type":"doc"}"#.into()
            })
            .unwrap()
            .receive()
            .await,
        Err(ServiceError::Conflict)
    ));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn bounded_queue_rejects_overflow_drains_writes_and_flushes_on_shutdown() {
    let directory = tempfile::tempdir().unwrap();
    let profile = Profile {
        database: directory.path().join("library.sqlite"),
    };
    let runtime = start(&profile).await;
    let flushed = Arc::new(AtomicBool::new(false));
    let flush_flag = flushed.clone();
    runtime
        .register_flush(Box::new(move || {
            Box::pin(async move {
                flush_flag.store(true, Ordering::Release);
                Ok(())
            })
        }))
        .unwrap()
        .receive()
        .await
        .unwrap();
    let (started_tx, started_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    let held = runtime
        .submit(move |_| async move {
            let _ = started_tx.send(());
            let _ = release_rx.await;
            Ok(())
        })
        .unwrap();
    started_rx.await.unwrap();
    let mut writes = Vec::new();
    for index in 0..QUEUE_CAPACITY {
        writes.push(
            runtime
                .create_note(format!("Queued {index}").into())
                .unwrap(),
        );
    }
    assert!(matches!(
        runtime.create_note("Overflow".into()),
        Err(ServiceError::Busy)
    ));
    let shutdown_runtime = runtime.clone();
    let shutdown = tokio::spawn(async move { shutdown_runtime.shutdown().await });
    release_tx.send(()).unwrap();
    held.receive().await.unwrap();
    drop(writes);
    shutdown.await.unwrap().unwrap();
    assert!(flushed.load(Ordering::Acquire));
    assert!(matches!(
        runtime.create_note("After shutdown".into()),
        Err(ServiceError::Closed)
    ));
    let runtime = start(&profile).await;
    let page = runtime
        .library(
            LibraryQuery {
                limit: 200,
                ..Default::default()
            },
            CancellationToken::new(),
        )
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(page.items.len(), QUEUE_CAPACITY);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn cancellation_and_generation_do_not_apply_stale_reads() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = start(&Profile {
        database: directory.path().join("library.sqlite"),
    })
    .await;
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        runtime
            .library(LibraryQuery::default(), cancel)
            .unwrap()
            .receive()
            .await,
        Err(ServiceError::Cancelled)
    ));
    let mut generation = Generation::default();
    let old = generation.advance();
    let current = generation.advance();
    assert_ne!(old, current);
    assert_eq!(generation, current);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn reactive_watch_initial_before_mutation_and_hard_unsubscribe() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = start(&Profile {
        database: directory.path().join("library.sqlite"),
    })
    .await;
    let mut watch = runtime.watch_library().unwrap().receive().await.unwrap();
    assert_eq!(watch.snapshots.borrow_and_update().sequence, 1);
    assert!(watch.snapshots.borrow().rows.is_empty());
    runtime
        .create_note("Reactive".into())
        .unwrap()
        .receive()
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), watch.snapshots.changed())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(watch.snapshots.borrow().rows.len(), 1);
    let receiver = watch.snapshots.clone();
    watch.unsubscribe().await.unwrap();
    let sequence = receiver.borrow().sequence;
    runtime
        .create_note("After unsubscribe".into())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(receiver.borrow().sequence, sequence);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn malformed_legacy_content_is_returned_without_replacement() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = start(&Profile {
        database: directory.path().join("new-profile/library.sqlite"),
    })
    .await;
    let session = runtime
        .create_note("Legacy".into())
        .unwrap()
        .receive()
        .await
        .unwrap();
    let id = session.note.unwrap().id;
    let raw = "{ invalid legacy text 日本語";
    runtime.submit(move |services| async move {
        services.executor.execute(
            "UPDATE session_documents SET body = ?, body_format = 'custom_format' WHERE id = ?".into(),
            vec![json!(raw), json!(id)],
        ).await.map_err(|e| ServiceError::Failed(e.to_string().into()))?;
        Ok(())
    }).unwrap().receive().await.unwrap();
    let opened = runtime
        .open_session(session.summary.id, CancellationToken::new())
        .unwrap()
        .receive()
        .await
        .unwrap();
    let document = opened.note.unwrap();
    assert_eq!(document.body.as_ref(), raw);
    assert_eq!(document.body_format.as_ref(), "custom_format");
    assert!(matches!(
        runtime
            .save_document(SaveDocument {
                base: document,
                body: r#"{"type":"doc"}"#.into(),
            })
            .unwrap()
            .receive()
            .await,
        Err(ServiceError::Unsupported(_))
    ));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn watch_admission_is_bounded_and_released_by_unsubscribe() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = start(&Profile {
        database: directory.path().join("library.sqlite"),
    })
    .await;
    let mut watches = Vec::new();
    for _ in 0..desktop_runtime::MAX_WATCHES {
        watches.push(runtime.watch_library().unwrap().receive().await.unwrap());
    }
    assert!(matches!(
        runtime.watch_library().unwrap().receive().await,
        Err(ServiceError::Busy)
    ));
    watches.pop().unwrap().unsubscribe().await.unwrap();
    runtime
        .watch_library()
        .unwrap()
        .receive()
        .await
        .unwrap()
        .unsubscribe()
        .await
        .unwrap();
    for watch in watches {
        watch.unsubscribe().await.unwrap();
    }
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn whole_library_orders_by_event_start_in_either_direction() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = start(&Profile {
        database: directory.path().join("order.sqlite"),
    })
    .await;
    runtime.submit(|services| async move {
        for sql in [
            "INSERT INTO events(id,tracking_id_event,calendar_id,started_at,ended_at) VALUES
                ('linked','t1','c1','2026-08-01T09:00:00Z','2026-08-01T10:00:00Z'),
                ('resynced','t2','c2','2026-09-20T09:00:00Z','2026-09-20T09:30:00Z')",
            "INSERT INTO sessions(id,title,created_at,event_id,event_json) VALUES
                ('plain','Plain','2026-09-01T00:00:00Z','',''),
                ('linked','Linked','2026-09-05T00:00:00Z','linked','{\"tracking_id\":\"t1\",\"calendar_id\":\"c1\",\"started_at\":\"2026-01-01T00:00:00Z\"}'),
                ('saved','Saved','2026-09-03T00:00:00Z','','{\"tracking_id\":\"gone\",\"calendar_id\":\"c9\",\"started_at\":\"2026-09-10T00:00:00Z\",\"ended_at\":\"2026-09-10T01:00:00Z\"}'),
                ('tracked','Tracked','2026-09-02T00:00:00Z','old-id','{\"tracking_id\":\"t2\",\"calendar_id\":\"c2\",\"started_at\":\"2026-01-01T00:00:00Z\"}'),
                ('broken','Broken','2026-08-15T00:00:00Z','','not json')",
        ] {
            services.executor.execute(sql.into(), vec![]).await.unwrap();
        }
        Ok(())
    }).unwrap().receive().await.unwrap();
    let ids = |oldest: bool, offset: u32, limit: u32| {
        let runtime = runtime.clone();
        async move {
            let page = runtime
                .library(
                    LibraryQuery {
                        offset,
                        limit,
                        oldest,
                        ..Default::default()
                    },
                    CancellationToken::new(),
                )
                .unwrap()
                .receive()
                .await
                .unwrap();
            let ids = page
                .items
                .iter()
                .map(|item| item.id.0.to_string())
                .collect::<Vec<_>>();
            (ids, page)
        }
    };
    let newest = ["tracked", "saved", "plain", "broken", "linked"];
    let (all, page) = ids(false, 0, u32::MAX).await;
    assert_eq!(all, newest);
    assert!(!page.has_more);
    let tracked = &page.items[0];
    assert_eq!(
        (tracked.event_start.as_ref(), tracked.event_end.as_ref()),
        ("2026-09-20T09:00:00Z", "2026-09-20T09:30:00Z")
    );
    assert_eq!(page.items[1].starts_at(), "2026-09-10T00:00:00Z");
    assert_eq!(page.items[2].starts_at(), "2026-09-01T00:00:00Z");
    let mut oldest = newest;
    oldest.reverse();
    assert_eq!(ids(true, 0, u32::MAX).await.0, oldest);
    let (cached, page) = ids(true, 1, 2).await;
    assert_eq!(cached, oldest[1..3]);
    assert!(page.has_more);
    let opened = runtime
        .open_session(
            desktop_runtime::SessionId("linked".into()),
            CancellationToken::new(),
        )
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(opened.summary.starts_at(), "2026-08-01T09:00:00Z");
    runtime.shutdown().await.unwrap();
}
