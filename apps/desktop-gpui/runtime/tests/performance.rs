use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anlg_db_execute::TransactionStatement;
use desktop_runtime::{
    CancellationToken, LibraryQuery, Profile, RuntimeHandle, SaveDocument, ServiceError,
};
use serde_json::{Value, json};

const SEED: u64 = 320;
const TIMESTAMP: &str = "2026-01-01T00:00:00.000Z";
const LONG_DOCUMENT_COUNT: usize = 10;

fn fingerprint(hash: &mut u64, text: &str) {
    for byte in text.bytes() {
        *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
}

async fn fixture(runtime: &RuntimeHandle, count: usize, document_bytes: usize) -> String {
    let mut hash = 0xcbf29ce484222325;
    for start in (0..count).step_by(64) {
        let mut statements = Vec::new();
        for index in start..(start + 64).min(count) {
            let id = format!("fixture-{SEED}-{index:08}");
            let title = format!("Fixture {index:08} 日本語");
            let text_bytes = if index < LONG_DOCUMENT_COUNT {
                document_bytes
            } else {
                256
            };
            let body = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"x".repeat(text_bytes)}]}]}).to_string();
            let params = vec![json!(id), json!(title), json!(TIMESTAMP)];
            fingerprint(&mut hash, &Value::Array(params.clone()).to_string());
            statements.push(TransactionStatement {
                sql: "INSERT INTO sessions (id,title,kind,created_at,updated_at) VALUES (?,?,'note',?,?)".into(),
                params: vec![params[0].clone(), params[1].clone(), params[2].clone(), params[2].clone()],
                expected_rows_affected: Some(1),
            });
            let document_params = vec![
                json!(id),
                json!(id),
                json!(body),
                json!(TIMESTAMP),
                json!(TIMESTAMP),
            ];
            fingerprint(
                &mut hash,
                &Value::Array(document_params.clone()).to_string(),
            );
            statements.push(TransactionStatement {
                sql: "INSERT INTO session_documents (id,session_id,body,created_at,updated_at) VALUES (?,?,?,?,?)".into(),
                params: document_params,
                expected_rows_affected: Some(1),
            });
        }
        runtime
            .submit(move |services| async move {
                services
                    .executor
                    .execute_transaction(statements)
                    .await
                    .map_err(|e| ServiceError::Failed(e.to_string().into()))?;
                Ok(())
            })
            .unwrap()
            .receive()
            .await
            .unwrap();
    }
    format!("fnv1a64:{hash:016x}")
}

#[tokio::test]
#[ignore = "isolated service microbenchmark; run explicitly in release mode with ANARLOG_BUILD_SHA"]
async fn isolated_runtime_distribution() {
    assert!(
        !std::hint::black_box(cfg!(debug_assertions)),
        "Use cargo test --release"
    );
    let sha = std::env::var("ANARLOG_BUILD_SHA")
        .expect("Set ANARLOG_BUILD_SHA to the measured source commit");
    let count: usize = std::env::var("ANARLOG_FIXTURE_ROWS")
        .unwrap_or_else(|_| "1000".into())
        .parse()
        .unwrap();
    assert!([1000, 10000, 50000].contains(&count));
    let document_bytes: usize = std::env::var("ANARLOG_FIXTURE_DOCUMENT_BYTES")
        .unwrap_or_else(|_| "256".into())
        .parse()
        .unwrap();
    assert!(document_bytes <= 10 * 1024 * 1024);
    let directory = tempfile::tempdir().unwrap();
    let profile = Profile {
        database: directory.path().join("isolated.sqlite"),
    };
    let started = Instant::now();
    let (runtime, ready) = RuntimeHandle::start(profile.clone()).unwrap();
    ready.receive().await.unwrap();
    let fresh_start_ns = started.elapsed().as_nanos();
    let fixture_hash = fixture(&runtime, count, document_bytes).await;
    let schema = runtime
        .submit(|services| async move {
            services
                .executor
                .execute(
                    "SELECT max(version) AS version FROM _sqlx_migrations".into(),
                    vec![],
                )
                .await
                .map_err(|e| ServiceError::Failed(e.to_string().into()))
        })
        .unwrap()
        .receive()
        .await
        .unwrap();
    runtime.shutdown().await.unwrap();
    let started = Instant::now();
    let (runtime, ready) = RuntimeHandle::start(profile).unwrap();
    ready.receive().await.unwrap();
    let warm_start_ns = started.elapsed().as_nanos();
    let plans = runtime.submit(|services| async move {
        let original = services.executor.execute(
            "EXPLAIN QUERY PLAN SELECT id,title,kind,status,created_at,updated_at,started_at,ended_at,series_id FROM sessions WHERE deleted_at IS NULL ORDER BY COALESCE(NULLIF(started_at,''),created_at) DESC,created_at DESC,id DESC LIMIT 101 OFFSET 0".into(), vec![],
        ).await.map_err(|e| ServiceError::Failed(e.to_string().into()))?;
        let optimized = services.executor.execute(
            "EXPLAIN QUERY PLAN WITH page AS MATERIALIZED (SELECT id,COALESCE(NULLIF(started_at,''),created_at) AS sort_at,created_at FROM sessions WHERE deleted_at IS NULL ORDER BY sort_at DESC,created_at DESC,id DESC LIMIT 101 OFFSET 0) SELECT s.id,s.title,s.kind,s.status,s.created_at,s.updated_at,s.started_at,s.ended_at,s.series_id FROM page CROSS JOIN sessions s ON s.id=page.id ORDER BY page.sort_at DESC,page.created_at DESC,page.id DESC".into(), vec![],
        ).await.map_err(|e| ServiceError::Failed(e.to_string().into()))?;
        Ok(json!({"original": original, "optimized": optimized}))
    }).unwrap().receive().await.unwrap();
    let mut library_ns = Vec::new();
    let mut original_ns = Vec::new();
    let mut uncached_ns = Vec::new();
    for index in 0..30 {
        let started = Instant::now();
        let result = runtime
            .library(
                LibraryQuery {
                    offset: (index * 31 % count) as u32,
                    limit: 100,
                    search: "".into(),
                },
                CancellationToken::new(),
            )
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert!(!result.items.is_empty());
        library_ns.push(started.elapsed().as_nanos());
        let started = Instant::now();
        let original = runtime.read(CancellationToken::new(), move |services| async move {
            sqlx::query_as::<_, anlg_db_app::SessionListItem>(
                "SELECT id,title,kind,status,created_at,updated_at,started_at,ended_at,series_id FROM sessions WHERE deleted_at IS NULL ORDER BY COALESCE(NULLIF(started_at,''),created_at) DESC,created_at DESC,id DESC LIMIT 101 OFFSET ?",
            ).bind((index * 31 % count) as i64).fetch_all(services.db.pool()).await
                .map_err(|e| ServiceError::Failed(e.to_string().into()))
        }).unwrap().receive().await.unwrap();
        original_ns.push(started.elapsed().as_nanos());
        let started = Instant::now();
        let uncached = runtime
            .read(CancellationToken::new(), move |services| async move {
                anlg_db_app::list_sessions(
                    services.db.pool(),
                    anlg_db_app::ListSessions {
                        query: None,
                        series_id: None,
                        folder_path: None,
                        limit: 101,
                        offset: (index * 31 % count) as u32,
                    },
                )
                .await
                .map_err(|e| ServiceError::Failed(e.to_string().into()))
            })
            .unwrap()
            .receive()
            .await
            .unwrap();
        uncached_ns.push(started.elapsed().as_nanos());
        assert_eq!(
            uncached.iter().map(|row| &row.id).collect::<Vec<_>>(),
            original.iter().map(|row| &row.id).collect::<Vec<_>>()
        );
        assert_eq!(
            result
                .items
                .iter()
                .map(|item| item.id.0.as_ref())
                .collect::<Vec<_>>(),
            original
                .iter()
                .take(100)
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>()
        );
    }
    let id = format!("fixture-{SEED}-00000000");
    let note = runtime
        .open_session(id.clone().into(), CancellationToken::new())
        .unwrap()
        .receive()
        .await
        .unwrap();
    let mut base = note.note.unwrap();
    let mut watch = runtime
        .watch_document(id.into())
        .unwrap()
        .receive()
        .await
        .unwrap();
    watch.snapshots.borrow_and_update();
    let mut save_ns = Vec::new();
    let mut delivery_ns = Vec::new();
    for index in 0..20 {
        let body: Arc<str> = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":format!("change-{index}:{}", "x".repeat(document_bytes))}]}]}).to_string().into();
        let started = Instant::now();
        base = runtime
            .save_document(SaveDocument { base, body })
            .unwrap()
            .receive()
            .await
            .unwrap();
        save_ns.push(started.elapsed().as_nanos());
        tokio::time::timeout(Duration::from_secs(5), watch.snapshots.changed())
            .await
            .unwrap()
            .unwrap();
        delivery_ns.push(started.elapsed().as_nanos());
        watch.snapshots.borrow_and_update();
    }
    watch.unsubscribe().await.unwrap();
    let started = Instant::now();
    runtime.shutdown().await.unwrap();
    println!(
        "{}",
        json!({
            "kind": "service_microbenchmark", "build_sha": sha, "release": true,
            "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
            "fixture": {"seed": SEED, "sessions": count, "documents": count,
                "long_document_text_bytes": document_bytes, "long_document_count": LONG_DOCUMENT_COUNT,
                "other_document_text_bytes": 256,
                "logical_content_hash": fixture_hash, "hash_scope": "ordered session input parameters and exact document JSON",
                "schema": schema, "timestamp": TIMESTAMP},
            "config": {"queue_capacity": 64, "service_capacity": 16, "service_concurrency":4, "watch_cap": 32, "pool_size": desktop_runtime::DATABASE_POOL_SIZE, "cache_observer_connections":1, "cache_pages":8, "cache_bytes":8388608, "tokio_workers": 2, "page_size":100},
            "fresh_start_ns":fresh_start_ns, "warm_start_ns":warm_start_ns,
            "library_ns":library_ns, "save_ns":save_ns, "enqueue_to_snapshot_ns":delivery_ns,
            "original_library_ns":original_ns, "uncached_library_ns":uncached_ns, "query_plans":plans,
            "shutdown_ns":started.elapsed().as_nanos(), "runtime":runtime.metrics(),
            "limitations":"Warm OS cache; no cache reset; no GUI/provider/audio; paired cached runtime, original SQL, uncached page SQL on identical data in that order; runtime samples mix cache misses and hits; FNV is not cryptographic; document construction/serialization precedes enqueue timer"
        })
    );
}
