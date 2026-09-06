use super::super::*;

#[cfg(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "windows",
    target_os = "ios"
))]
async fn assert_interrupts_stalled_native_request_once() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (accepted_tx, accepted_rx) = std::sync::mpsc::sync_channel(1);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        accepted_tx.send(()).unwrap();
        let _stream = stream;
        let _ = release_rx.recv_timeout(Duration::from_secs(5));
    });

    let db = Arc::new(Db::connect_memory().await.unwrap());
    sqlx::query(
        "CREATE TABLE items (
            id TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL DEFAULT ''
        )",
    )
    .execute(db.pool())
    .await
    .unwrap();
    db.cloudsync_init("items", None, None).await.unwrap();
    {
        let mut connection = db.cloudsync_connection.lock().await;
        *connection = Some(db.pool.acquire().await.unwrap());
        sqlx::query("SELECT cloudsync_network_init_custom(?, ?)")
            .bind(endpoint)
            .bind("interrupt-test")
            .fetch_optional(&mut **connection.as_mut().unwrap())
            .await
            .unwrap();
    }

    let request_db = Arc::clone(&db);
    let request = tokio::spawn(async move {
        let _sync_operation = request_db.cloudsync_sync_operation.lock().await;
        let mut connection = request_db.cloudsync_connection.lock().await;
        super::super::super::ops::interruptible_network_receive_changes(
            connection.as_mut().unwrap(),
            &request_db.cloudsync_interrupt,
        )
        .await
    });
    tokio::task::spawn_blocking(move || {
        accepted_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("native CloudSync request did not reach the blackhole server");
    })
    .await
    .unwrap();

    let interrupted_at = std::time::Instant::now();
    while !request.is_finished() {
        db.cloudsync_interrupt_sync();
        assert!(
            interrupted_at.elapsed() < Duration::from_secs(2),
            "native CloudSync request did not honor sqlite3_interrupt"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let result = request.await.unwrap();
    assert!(result.is_err(), "interrupted native request succeeded");
    assert!(interrupted_at.elapsed() < Duration::from_secs(2));
    tokio::time::timeout(
        Duration::from_millis(100),
        db.cloudsync_wait_for_sync_idle(),
    )
    .await
    .expect("sync operation released before its SQLite worker became idle");

    {
        let mut connection = db.cloudsync_connection.lock().await;
        let value: i64 = sqlx::query_scalar("SELECT 1")
            .fetch_one(&mut **connection.as_mut().unwrap())
            .await
            .unwrap();
        assert_eq!(value, 1);
        let worker_idle = connection.as_mut().unwrap().lock_handle().await.unwrap();
        drop(worker_idle);
    }

    let _ = release_tx.send(());
    server.join().unwrap();
    db.cloudsync_close_connection().await.unwrap();
}

#[cfg(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "windows",
    target_os = "ios"
))]
#[tokio::test]
async fn native_cloudsync_interrupt_drains_blackhole_http_and_reuses_connection() {
    for _ in 0..3 {
        assert_interrupts_stalled_native_request_once().await;
    }
}
