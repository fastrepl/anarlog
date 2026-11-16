use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use desktop_runtime::{
    CancellationToken, Profile, RequestGate, RuntimeHandle, RuntimeState, ServiceError,
    ShutdownPhase,
};
use serde_json::json;
use tokio::sync::oneshot;

async fn open(profile: &Profile) -> RuntimeHandle {
    let (runtime, ready) = RuntimeHandle::start(profile.clone()).unwrap();
    ready.receive().await.unwrap();
    runtime
}

#[tokio::test]
async fn startup_lease_prevents_concurrent_migrations_and_releases_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    let profile = Profile {
        database: dir.path().join("library.sqlite"),
    };
    let first = open(&profile).await;
    let (second, ready) = RuntimeHandle::start(profile.clone()).unwrap();
    assert!(ready.receive().await.is_err());
    assert!(second.shutdown().await.is_err());
    first.shutdown().await.unwrap();
    open(&profile).await.shutdown().await.unwrap();
    let marker = dir.path().join("library.sqlite.reset-requested");
    std::fs::write(&marker, "keep backup").unwrap();
    let (blocked, ready) = RuntimeHandle::start(profile.clone()).unwrap();
    assert!(blocked.submit(|_| async { Ok(()) }).is_err());
    assert!(blocked.service(|_| async { Ok(()) }).is_err());
    assert!(matches!(
        ready.receive().await,
        Err(ServiceError::Unsupported(_))
    ));
    assert!(blocked.shutdown().await.is_err());
    assert_eq!(std::fs::read_to_string(marker).unwrap(), "keep backup");
}

#[tokio::test]
async fn incompatible_schema_and_cloudsync_are_rejected_before_bootstrap() {
    for sql in [
        "CREATE TABLE cloudsync_table_settings (id TEXT)",
        "UPDATE _anlg_schema_compat SET min_supported_version = 99999999999999 WHERE id = 0",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let profile = Profile {
            database: dir.path().join("library.sqlite"),
        };
        let runtime = open(&profile).await;
        runtime
            .submit(move |services| async move {
                services
                    .executor
                    .execute(sql.into(), vec![])
                    .await
                    .map_err(|e| ServiceError::Failed(e.to_string().into()))?;
                Ok(())
            })
            .unwrap()
            .receive()
            .await
            .unwrap();
        runtime.shutdown().await.unwrap();
        let (blocked, ready) = RuntimeHandle::start(profile).unwrap();
        assert!(ready.receive().await.is_err(), "{sql}");
        assert!(blocked.shutdown().await.is_err());
    }
}

#[tokio::test]
async fn service_jobs_do_not_block_database_and_finish_before_flush() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = open(&Profile {
        database: dir.path().join("library.sqlite"),
    })
    .await;
    let (started, ready) = oneshot::channel();
    let service = runtime
        .service(move |services| async move {
            let _ = started.send(());
            services.shutdown_requested.cancelled().await;
            services
                .executor
                .execute(
                    "INSERT INTO sessions (id,title,kind) VALUES ('tail','Tail','note')".into(),
                    vec![],
                )
                .await
                .map_err(|e| ServiceError::Failed(e.to_string().into()))?;
            Ok(())
        })
        .unwrap();
    ready.await.unwrap();
    tokio::time::timeout(
        Duration::from_secs(5),
        runtime.create_note("Independent".into()).unwrap().receive(),
    )
    .await
    .unwrap()
    .unwrap();
    let check = runtime
        .submit(|services| async move { Ok(services.executor) })
        .unwrap()
        .receive()
        .await
        .unwrap();
    runtime
        .register_flush(Box::new(move || {
            Box::pin(async move {
                let rows = check
                    .execute("SELECT id FROM sessions WHERE id='tail'".into(), vec![])
                    .await
                    .map_err(|e| ServiceError::Failed(e.to_string().into()))?;
                assert_eq!(rows.len(), 1);
                Ok(())
            })
        }))
        .unwrap()
        .receive()
        .await
        .unwrap();
    runtime.shutdown().await.unwrap();
    service.receive().await.unwrap();
}

#[tokio::test]
async fn dropping_shutdown_waiter_does_not_interrupt_drain_and_all_waiters_agree() {
    let dir = tempfile::tempdir().unwrap();
    let profile = Profile {
        database: dir.path().join("library.sqlite"),
    };
    let runtime = open(&profile).await;
    let (started, ready) = oneshot::channel();
    let (release, held) = oneshot::channel();
    runtime
        .submit(move |_| async move {
            let _ = started.send(());
            let _ = held.await;
            Ok(())
        })
        .unwrap();
    ready.await.unwrap();
    drop(runtime.create_note("Must persist".into()).unwrap());
    let mut waiting = Box::pin(runtime.shutdown());
    assert!(futures::poll!(&mut waiting).is_pending());
    drop(waiting);
    assert!(matches!(
        runtime.create_note("Too late".into()),
        Err(ServiceError::Closed)
    ));
    release.send(()).unwrap();
    let (a, b) = tokio::join!(runtime.shutdown(), runtime.shutdown());
    a.unwrap();
    b.unwrap();
    assert!(matches!(
        *runtime.state().borrow(),
        RuntimeState::Closed(Ok(()))
    ));
    let runtime = open(&profile).await;
    let library = runtime
        .library(Default::default(), CancellationToken::new())
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert_eq!(library.items.len(), 1);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn phase_failures_drain_all_participants_and_panics_do_not_kill_queue() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = open(&Profile {
        database: dir.path().join("library.sqlite"),
    })
    .await;
    let failed = runtime
        .submit::<(), _, _>(|_| async {
            panic!("operation panic");
        })
        .unwrap()
        .receive()
        .await;
    assert!(failed.is_err());
    runtime
        .create_note("After panic".into())
        .unwrap()
        .receive()
        .await
        .unwrap();
    let order = Arc::new(Mutex::new(Vec::new()));
    for phase in [
        ShutdownPhase::StopServices,
        ShutdownPhase::ApplicationState,
        ShutdownPhase::PendingDeletions,
    ] {
        let order = order.clone();
        runtime
            .register_shutdown(
                phase,
                Box::new(move || {
                    Box::pin(async move {
                        order.lock().unwrap().push(phase);
                        if phase == ShutdownPhase::ApplicationState {
                            return Err(ServiceError::Conflict);
                        }
                        Ok(())
                    })
                }),
            )
            .unwrap()
            .receive()
            .await
            .unwrap();
    }
    assert!(runtime.shutdown().await.is_err());
    assert!(runtime.shutdown().await.is_err());
    assert_eq!(
        *order.lock().unwrap(),
        vec![
            ShutdownPhase::PendingDeletions,
            ShutdownPhase::ApplicationState,
            ShutdownPhase::StopServices,
        ]
    );
    assert_eq!(runtime.metrics().queued, 0);
    assert_eq!(runtime.metrics().failures, 1);
}

#[tokio::test]
async fn watch_drop_and_shutdown_are_delivery_barriers_and_document_watch_is_scoped() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = open(&Profile {
        database: dir.path().join("library.sqlite"),
    })
    .await;
    let session = runtime
        .create_note("Watch".into())
        .unwrap()
        .receive()
        .await
        .unwrap();
    let mut watch = runtime
        .watch_document(session.summary.id.clone())
        .unwrap()
        .receive()
        .await
        .unwrap();
    watch.snapshots.borrow_and_update();
    let before = watch.snapshots.borrow().sequence;
    runtime
        .rename_session(desktop_runtime::RenameSession {
            base: session.summary,
            title: "Metadata".into(),
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(80), watch.snapshots.changed())
            .await
            .is_err()
    );
    assert_eq!(watch.snapshots.borrow().sequence, before);
    runtime
        .save_document(desktop_runtime::SaveDocument {
            base: session.note.unwrap(),
            body: json!({"type":"doc","content":[]}).to_string().into(),
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), watch.snapshots.changed())
        .await
        .unwrap()
        .unwrap();
    let mut receiver = watch.snapshots.clone();
    drop(watch);
    while receiver.changed().await.is_ok() {}
    let held = runtime.watch_library().unwrap().receive().await.unwrap();
    runtime.shutdown().await.unwrap();
    assert!(held.snapshots.has_changed().is_err());
    held.unsubscribe().await.unwrap();
}

#[tokio::test]
async fn same_resource_generations_cancel_old_requests_and_reject_old_results() {
    let mut gate = RequestGate::default();
    let (old, cancel) = gate.begin();
    let (current, next) = gate.begin();
    assert!(cancel.is_cancelled());
    assert!(!gate.is_current(old));
    assert!(gate.is_current(current));
    drop(gate);
    assert!(next.is_cancelled());
}

#[tokio::test]
async fn oversize_watch_terminates_only_its_subscription_and_preserves_visible_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = open(&Profile {
        database: dir.path().join("library.sqlite"),
    })
    .await;
    let bad = runtime.watch_query(
        "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x < 1001) SELECT x FROM n".into(), vec![],
    ).unwrap().receive().await;
    assert!(bad.is_err());
    let mut watch = runtime
        .watch_query("SELECT title FROM sessions".into(), vec![])
        .unwrap()
        .receive()
        .await
        .unwrap();
    watch.snapshots.borrow_and_update();
    let title = "x".repeat(desktop_runtime::MAX_DOCUMENT_BYTES);
    runtime
        .submit(move |services| async move {
            services
                .executor
                .execute(
                    "INSERT INTO sessions(id,title) VALUES('oversized',?)".into(),
                    vec![json!(title)],
                )
                .await
                .map_err(|e| ServiceError::Failed(e.to_string().into()))?;
            Ok(())
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), watch.snapshots.changed())
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        watch.terminal_error(),
        Some(ServiceError::Unsupported(_))
    ));
    assert!(watch.snapshots.borrow().rows.is_empty());
    runtime
        .submit(|services| async move {
            services
                .executor
                .execute("UPDATE sessions SET title='small'".into(), vec![])
                .await
                .map_err(|e| ServiceError::Failed(e.to_string().into()))?;
            Ok(())
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    let recovered = runtime.watch_library().unwrap().receive().await.unwrap();
    assert_eq!(recovered.snapshots.borrow().rows.len(), 1);
    recovered.unsubscribe().await.unwrap();
    watch.unsubscribe().await.unwrap();
    runtime.shutdown().await.unwrap();
    assert!(runtime.metrics().snapshot_count > 0);
    assert!(runtime.metrics().snapshot_bytes > 0);
}

#[tokio::test]
async fn service_backpressure_is_independent_and_accepted_jobs_drain_after_shutdown() {
    let dir = tempfile::tempdir().unwrap();
    let runtime = open(&Profile {
        database: dir.path().join("library.sqlite"),
    })
    .await;
    let mut replies = Vec::new();
    for _ in 0..4 {
        let (started, ready) = oneshot::channel();
        replies.push(
            runtime
                .service(move |services| async move {
                    let _ = started.send(());
                    services.shutdown_requested.cancelled().await;
                    Ok(())
                })
                .unwrap(),
        );
        ready.await.unwrap();
    }
    for _ in 0..desktop_runtime::SERVICE_CAPACITY {
        replies.push(runtime.service(|_| async { Ok(()) }).unwrap());
    }
    assert!(matches!(
        runtime.service(|_| async { Ok(()) }),
        Err(ServiceError::Busy)
    ));
    runtime
        .create_note("Database still writable".into())
        .unwrap()
        .receive()
        .await
        .unwrap();
    runtime.shutdown().await.unwrap();
    for reply in replies {
        reply.receive().await.unwrap();
    }
    assert_eq!(runtime.metrics().queued, 0);
    assert!(runtime.metrics().busy >= 1);
}
