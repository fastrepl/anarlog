use std::time::{Duration, SystemTime, UNIX_EPOCH};

use db_app::{
    claim_cloudsync_workspace, cloudsync_table_registry, ensure_cloudsync_workspace_binding,
    prepare_schema,
};
use hypr_db_core::{
    CloudsyncAuth, CloudsyncRuntimeConfig, CloudsyncRuntimeError, Db, DbOpenOptions, DbStorage,
};
use sqlx::{AssertSqlSafe, SqlitePool};

const SYNC_TIMEOUT: Duration = Duration::from_secs(90);
const SYNC_ATTEMPTS: usize = 3;
const POLICY_SYNC_TIMEOUT: Duration = Duration::from_secs(30);
const STALE_SNAPSHOT_SYNC_ATTEMPTS: usize = 2;
const SYNCED_TABLES: [&str; 8] = [
    "organizations",
    "humans",
    "sessions",
    "session_documents",
    "transcripts",
    "session_participants",
    "action_items",
    "session_attachments",
];

fn cloudsync_config(auth: CloudsyncAuth, wait_ms: i64, max_retries: i64) -> CloudsyncRuntimeConfig {
    CloudsyncRuntimeConfig {
        connection_string: std::env::var("ANARLOG_CLOUDSYNC_DATABASE_ID")
            .expect("ANARLOG_CLOUDSYNC_DATABASE_ID must be set"),
        auth,
        tables: cloudsync_table_registry().to_vec(),
        sync_interval_ms: 86_400_000,
        wait_ms: Some(wait_ms),
        max_retries: Some(max_retries),
    }
}

async fn setup_db_with_network_options(
    auth: CloudsyncAuth,
    workspace_id: Option<&str>,
    wait_ms: i64,
    max_retries: i64,
) -> Db {
    let db = Db::open(DbOpenOptions {
        storage: DbStorage::Memory,
        cloudsync_enabled: true,
        journal_mode_wal: true,
        foreign_keys: true,
        max_connections: Some(1),
    })
    .await
    .unwrap();

    prepare_schema(&db).await.unwrap();
    if let Some(workspace_id) = workspace_id {
        claim_cloudsync_workspace(db.pool(), workspace_id)
            .await
            .unwrap();
    }
    db.cloudsync_configure(cloudsync_config(auth, wait_ms, max_retries))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(15), db.cloudsync_start())
        .await
        .expect("cloudsync start timed out")
        .unwrap();
    db
}

async fn setup_db(auth: CloudsyncAuth, workspace_id: Option<&str>) -> Db {
    setup_db_with_network_options(auth, workspace_id, 5_000, 3).await
}

async fn setup_policy_db(auth: CloudsyncAuth, workspace_id: &str) -> Db {
    setup_db_with_network_options(auth, Some(workspace_id), 2_500, 1).await
}

async fn sync_ok(db: &Db, label: &str) {
    let mut result = None;
    let mut last_failure = None;
    for attempt in 1..=SYNC_ATTEMPTS {
        match tokio::time::timeout(SYNC_TIMEOUT, db.cloudsync_trigger_sync()).await {
            Ok(Ok(value)) => {
                result = Some(value);
                break;
            }
            Ok(Err(error)) => last_failure = Some(error.to_string()),
            Err(_) => last_failure = Some("timed out".to_string()),
        }
        if attempt < SYNC_ATTEMPTS {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
    let result = result.unwrap_or_else(|| {
        panic!(
            "{label} failed after {SYNC_ATTEMPTS} attempts: {}",
            last_failure.as_deref().unwrap_or("unknown error")
        )
    });
    if let Some(send) = result.send {
        assert_eq!(send.status, "synced", "{label} send did not complete");
        assert!(send.last_failure.is_none(), "{label} send failed");
    }
    if let Some(receive) = result.receive {
        assert!(receive.complete, "{label} receive did not complete");
        assert!(receive.error.is_none(), "{label} receive failed");
        assert!(
            receive.last_failure.is_none(),
            "{label} receive did not complete"
        );
    }
    let status = db.cloudsync_status().await.unwrap();
    assert!(
        status.last_error.is_none(),
        "{label} recorded a sync error: {:?}",
        status.last_error
    );
}

#[derive(Debug)]
struct SyncedFixture {
    organization: String,
    human: String,
    session: String,
    document: String,
    transcript: String,
    participant: String,
    action_item: String,
    attachment: String,
}

impl SyncedFixture {
    fn rows(&self) -> [(&'static str, &str); 8] {
        [
            ("organizations", &self.organization),
            ("humans", &self.human),
            ("sessions", &self.session),
            ("session_documents", &self.document),
            ("transcripts", &self.transcript),
            ("session_participants", &self.participant),
            ("action_items", &self.action_item),
            ("session_attachments", &self.attachment),
        ]
    }

    fn rows_child_first(&self) -> [(&'static str, &str); 8] {
        [
            ("session_attachments", &self.attachment),
            ("action_items", &self.action_item),
            ("session_participants", &self.participant),
            ("transcripts", &self.transcript),
            ("session_documents", &self.document),
            ("sessions", &self.session),
            ("humans", &self.human),
            ("organizations", &self.organization),
        ]
    }
}

async fn insert_synced_fixture(
    pool: &SqlitePool,
    workspace_id: &str,
    marker: &str,
) -> SyncedFixture {
    let (
        organization,
        human,
        session,
        document,
        transcript,
        participant,
        action_item,
        attachment,
    ): (String, String, String, String, String, String, String, String) = sqlx::query_as(
        "SELECT cloudsync_uuid(), cloudsync_uuid(), cloudsync_uuid(), cloudsync_uuid(), \
                cloudsync_uuid(), cloudsync_uuid(), cloudsync_uuid(), cloudsync_uuid()",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let fixture = SyncedFixture {
        organization,
        human,
        session,
        document,
        transcript,
        participant,
        action_item,
        attachment,
    };
    let mut transaction = pool.begin().await.unwrap();

    sqlx::query(
        "INSERT INTO organizations (id, workspace_id, owner_user_id, name) VALUES (?, ?, ?, ?)",
    )
    .bind(&fixture.organization)
    .bind(workspace_id)
    .bind(workspace_id)
    .bind(marker)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO humans (id, workspace_id, owner_user_id, organization_id, name) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&fixture.human)
    .bind(workspace_id)
    .bind(workspace_id)
    .bind(&fixture.organization)
    .bind(marker)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title) VALUES (?, ?, ?, ?)",
    )
    .bind(&fixture.session)
    .bind(workspace_id)
    .bind(workspace_id)
    .bind(marker)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO session_documents (id, workspace_id, session_id, title, body) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&fixture.document)
    .bind(workspace_id)
    .bind(&fixture.session)
    .bind(marker)
    .bind(marker)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO transcripts (id, workspace_id, owner_user_id, session_id, memo) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&fixture.transcript)
    .bind(workspace_id)
    .bind(workspace_id)
    .bind(&fixture.session)
    .bind(marker)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO session_participants \
             (id, workspace_id, owner_user_id, session_id, human_id, display_name) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&fixture.participant)
    .bind(workspace_id)
    .bind(workspace_id)
    .bind(&fixture.session)
    .bind(&fixture.human)
    .bind(marker)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO action_items \
             (id, workspace_id, session_id, assignee_human_id, text, created_by, updated_by) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&fixture.action_item)
    .bind(workspace_id)
    .bind(&fixture.session)
    .bind(&fixture.human)
    .bind(marker)
    .bind(workspace_id)
    .bind(workspace_id)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO session_attachments (id, workspace_id, session_id, filename) \
         VALUES (?, ?, ?, ?)",
    )
    .bind(&fixture.attachment)
    .bind(workspace_id)
    .bind(&fixture.session)
    .bind(marker)
    .execute(&mut *transaction)
    .await
    .unwrap();

    transaction.commit().await.unwrap();
    fixture
}

async fn delete_synced_fixture(pool: &SqlitePool, fixture: &SyncedFixture) {
    let mut transaction = pool.begin().await.unwrap();
    for (table, id) in fixture.rows_child_first() {
        sqlx::query(AssertSqlSafe(format!("DELETE FROM {table} WHERE id = ?")))
            .bind(id)
            .execute(&mut *transaction)
            .await
            .unwrap();
    }
    transaction.commit().await.unwrap();
}

async fn row_count(pool: &SqlitePool, table: &str, id: &str, workspace_id: &str) -> i64 {
    sqlx::query_scalar(AssertSqlSafe(format!(
        "SELECT COUNT(*) FROM {table} WHERE id = ? AND workspace_id = ?"
    )))
    .bind(id)
    .bind(workspace_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn row_count_by_id(pool: &SqlitePool, table: &str, id: &str) -> i64 {
    sqlx::query_scalar(AssertSqlSafe(format!(
        "SELECT COUNT(*) FROM {table} WHERE id = ?"
    )))
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn assert_fixture_visibility(
    pool: &SqlitePool,
    workspace_id: &str,
    own: &SyncedFixture,
    foreign_workspace_id: &str,
    foreign: &SyncedFixture,
) {
    for (table, id) in own.rows() {
        assert_eq!(
            row_count(pool, table, id, workspace_id).await,
            1,
            "{table} did not download its own workspace row"
        );
    }
    for (table, id) in foreign.rows() {
        assert_eq!(
            row_count(pool, table, id, foreign_workspace_id).await,
            0,
            "{table} leaked a foreign workspace row"
        );
    }
}

async fn insert_foreign_row(
    pool: &SqlitePool,
    table: &str,
    foreign_workspace_id: &str,
    target: &SyncedFixture,
    marker: &str,
) -> String {
    let id: String = sqlx::query_scalar("SELECT cloudsync_uuid()")
        .fetch_one(pool)
        .await
        .unwrap();

    match table {
        "organizations" => {
            sqlx::query(
                "INSERT INTO organizations (id, workspace_id, owner_user_id, name) \
                 VALUES (?, ?, ?, ?)",
            )
            .bind(&id)
            .bind(foreign_workspace_id)
            .bind(foreign_workspace_id)
            .bind(marker)
            .execute(pool)
            .await
            .unwrap();
        }
        "humans" => {
            sqlx::query(
                "INSERT INTO humans (id, workspace_id, owner_user_id, organization_id, name) \
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&id)
            .bind(foreign_workspace_id)
            .bind(foreign_workspace_id)
            .bind(&target.organization)
            .bind(marker)
            .execute(pool)
            .await
            .unwrap();
        }
        "sessions" => {
            sqlx::query(
                "INSERT INTO sessions (id, workspace_id, owner_user_id, title) \
                 VALUES (?, ?, ?, ?)",
            )
            .bind(&id)
            .bind(foreign_workspace_id)
            .bind(foreign_workspace_id)
            .bind(marker)
            .execute(pool)
            .await
            .unwrap();
        }
        "session_documents" => {
            sqlx::query(
                "INSERT INTO session_documents (id, workspace_id, session_id, title) \
                 VALUES (?, ?, ?, ?)",
            )
            .bind(&id)
            .bind(foreign_workspace_id)
            .bind(&target.session)
            .bind(marker)
            .execute(pool)
            .await
            .unwrap();
        }
        "transcripts" => {
            sqlx::query(
                "INSERT INTO transcripts (id, workspace_id, owner_user_id, session_id, memo) \
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&id)
            .bind(foreign_workspace_id)
            .bind(foreign_workspace_id)
            .bind(&target.session)
            .bind(marker)
            .execute(pool)
            .await
            .unwrap();
        }
        "session_participants" => {
            sqlx::query(
                "INSERT INTO session_participants \
                     (id, workspace_id, owner_user_id, session_id, human_id, display_name) \
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(&id)
            .bind(foreign_workspace_id)
            .bind(foreign_workspace_id)
            .bind(&target.session)
            .bind(&target.human)
            .bind(marker)
            .execute(pool)
            .await
            .unwrap();
        }
        "action_items" => {
            sqlx::query(
                "INSERT INTO action_items \
                     (id, workspace_id, session_id, assignee_human_id, text, created_by, updated_by) \
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&id)
            .bind(foreign_workspace_id)
            .bind(&target.session)
            .bind(&target.human)
            .bind(marker)
            .bind(foreign_workspace_id)
            .bind(foreign_workspace_id)
            .execute(pool)
            .await
            .unwrap();
        }
        "session_attachments" => {
            sqlx::query(
                "INSERT INTO session_attachments (id, workspace_id, session_id, filename) \
                 VALUES (?, ?, ?, ?)",
            )
            .bind(&id)
            .bind(foreign_workspace_id)
            .bind(&target.session)
            .bind(marker)
            .execute(pool)
            .await
            .unwrap();
        }
        _ => panic!("unsupported synced table: {table}"),
    }

    id
}

fn fixture_value_column(table: &str) -> &'static str {
    match table {
        "organizations" | "humans" => "name",
        "sessions" | "session_documents" => "title",
        "transcripts" => "memo",
        "session_participants" => "display_name",
        "action_items" => "text",
        "session_attachments" => "filename",
        _ => panic!("unsupported synced table: {table}"),
    }
}

async fn setup_snapshot_policy_db(
    token: &str,
    local_workspace: &str,
    row_workspace: &str,
    table: &str,
    id: &str,
) -> Db {
    let db = setup_db(
        CloudsyncAuth::Token {
            token: token.to_string(),
        },
        Some(local_workspace),
    )
    .await;
    db.cloudsync_network_reset_sync_version()
        .await
        .unwrap_or_else(|error| panic!("{table} snapshot fixture reset failed: {error}"));
    for attempt in 1..=STALE_SNAPSHOT_SYNC_ATTEMPTS {
        sync_ok(
            &db,
            &format!("{table} snapshot fixture download attempt {attempt}"),
        )
        .await;
        if row_count(db.pool(), table, id, row_workspace).await == 1 {
            break;
        }
    }
    let downloaded_rows = row_count(db.pool(), table, id, row_workspace).await;
    let status = db.cloudsync_status().await.unwrap();
    assert_eq!(
        downloaded_rows, 1,
        "{table} snapshot fixture was not downloaded after a reset and \
         {STALE_SNAPSHOT_SYNC_ATTEMPTS} bounded attempts; last sync: {:?}; last error: {:?}",
        status.last_sync, status.last_error,
    );
    if status.has_unsent_changes != Some(false) {
        sync_ok(&db, &format!("{table} snapshot fixture upload drain")).await;
    }
    assert_eq!(
        db.cloudsync_status().await.unwrap().has_unsent_changes,
        Some(false),
        "{table} snapshot client had unrelated pending changes"
    );

    db
}

async fn setup_stale_policy_db(
    token_b: &str,
    token_a: &str,
    workspace_b: &str,
    table: &str,
    id: &str,
) -> Db {
    let db = setup_snapshot_policy_db(token_b, workspace_b, workspace_b, table, id).await;

    // Keep the downloaded B-owned row, but switch only the network identity to A.
    tokio::time::timeout(Duration::from_secs(15), db.cloudsync_stop())
        .await
        .unwrap_or_else(|_| panic!("{table} B-authenticated client stop timed out"))
        .unwrap();
    db.cloudsync_configure(cloudsync_config(
        CloudsyncAuth::Token {
            token: token_a.to_string(),
        },
        2_500,
        1,
    ))
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(15), db.cloudsync_start())
        .await
        .unwrap_or_else(|_| panic!("{table} A-authenticated client start timed out"))
        .unwrap();
    assert_eq!(
        db.cloudsync_status().await.unwrap().has_unsent_changes,
        Some(false),
        "{table} reauthentication introduced unrelated pending changes"
    );
    db
}

async fn update_stale_foreign_row(pool: &SqlitePool, table: &str, id: &str, marker: &str) {
    let column = fixture_value_column(table);
    let result = sqlx::query(AssertSqlSafe(format!(
        "UPDATE {table} SET {column} = ? WHERE id = ?"
    )))
    .bind(marker)
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
    assert_eq!(result.rows_affected(), 1, "{table} stale UPDATE missed");
}

async fn delete_stale_foreign_row(pool: &SqlitePool, table: &str, id: &str) {
    let result = sqlx::query(AssertSqlSafe(format!("DELETE FROM {table} WHERE id = ?")))
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    assert_eq!(result.rows_affected(), 1, "{table} stale DELETE missed");
}

async fn assert_policy_change_pending(db: &Db, operation: &str) {
    assert_eq!(
        db.cloudsync_status().await.unwrap().has_unsent_changes,
        Some(true),
        "{operation} was not tracked as a pending CloudSync change"
    );
}

async fn fixture_state_violations(
    pool: &SqlitePool,
    workspace_id: &str,
    fixture: &SyncedFixture,
    expected_value: &str,
) -> Vec<String> {
    let mut violations = Vec::new();
    for (table, id) in fixture.rows() {
        let column = fixture_value_column(table);
        let row: Option<(String, String)> = sqlx::query_as(AssertSqlSafe(format!(
            "SELECT workspace_id, {column} FROM {table} WHERE id = ?"
        )))
        .bind(id)
        .fetch_optional(pool)
        .await
        .unwrap();
        match row {
            None => violations.push(format!("{table} was deleted")),
            Some((actual_workspace_id, _)) if actual_workspace_id != workspace_id => {
                violations.push(format!("{table} changed workspace"));
            }
            Some((_, actual_value)) if actual_value != expected_value => {
                violations.push(format!("{table} was updated"));
            }
            Some(_) => {}
        }
    }
    violations
}

fn is_rls_policy_denial(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    let mentions_rls = message.contains("row-level security")
        || message.contains("row level security")
        || message
            .split(|character: char| !character.is_ascii_alphanumeric())
            .any(|part| part == "rls");
    let mentions_denial = [
        "denied",
        "deny",
        "rejected",
        "forbidden",
        "violation",
        "failed",
        "failure",
        "not allowed",
        "not authorized",
    ]
    .iter()
    .any(|needle| message.contains(needle));

    mentions_rls && mentions_denial
}

async fn expect_policy_sync_completed_or_denied(db: &Db, table: &str) -> Result<(), String> {
    let outcome = tokio::time::timeout(POLICY_SYNC_TIMEOUT, db.cloudsync_trigger_sync())
        .await
        .unwrap_or_else(|_| panic!("{table} foreign-write sync timed out"));
    let mut evidence = Vec::new();
    let mut clean_send = false;
    let mut clean_receive = true;
    match outcome {
        Ok(result) => {
            if let Some(send) = result.send {
                clean_send =
                    send.status.eq_ignore_ascii_case("synced") && send.last_failure.is_none();
                evidence.push(format!("send status: {}", send.status));
                if let Some(last_failure) = send.last_failure {
                    evidence.push(format!("send failure: {last_failure}"));
                }
            }
            if let Some(receive) = result.receive {
                if let Some(error) = receive.error {
                    clean_receive = false;
                    evidence.push(format!("receive error: {error}"));
                }
                if let Some(last_failure) = receive.last_failure {
                    clean_receive = false;
                    evidence.push(format!("receive failure: {last_failure}"));
                }
            }
        }
        Err(error) => evidence.push(error.to_string()),
    }
    let status = db.cloudsync_status().await.unwrap();
    let runtime_clean = status.last_error.is_none();
    if let Some(error) = &status.last_error {
        evidence.push(format!("runtime error: {error}"));
    }

    let explicit_denial = evidence.iter().any(|entry| is_rls_policy_denial(entry));

    // SQLite Cloud can silently discard an RLS-blocked send while reporting it as synced.
    // The final B-authenticated state checks prove whether the mutation reached the server.
    if explicit_denial || (clean_send && clean_receive && runtime_clean) {
        Ok(())
    } else {
        Err(format!(
            "{table} neither completed a clean policy send nor returned an RLS denial; sync evidence: {}",
            evidence.join(" | ")
        ))
    }
}

#[test]
fn rls_policy_denial_matcher_rejects_generic_failures() {
    assert!(is_rls_policy_denial("RLS policy denied INSERT on sessions"));
    assert!(is_rls_policy_denial(
        "row-level security policy check failed"
    ));
    assert!(!is_rls_policy_denial(
        "401 database_auth_failed: database credentials were rejected: Invalid APIKEY"
    ));
    assert!(!is_rls_policy_denial("connection timed out"));
    assert!(!is_rls_policy_denial("access token expired"));
    assert!(!is_rls_policy_denial("network retry policy failed"));
}

#[test]
fn policy_fixture_covers_every_enabled_table() {
    let mut enabled_tables: Vec<&str> = cloudsync_table_registry()
        .iter()
        .filter(|table| table.enabled)
        .map(|table| table.table_name.as_str())
        .collect();
    enabled_tables.sort_unstable();
    let mut covered_tables = SYNCED_TABLES;
    covered_tables.sort_unstable();

    assert_eq!(enabled_tables, covered_tables);
}

#[tokio::test]
#[ignore = "external verification only; requires the anarlog-dev SQLite Cloud credentials"]
async fn core_session_syncs_between_two_clients() {
    let marker = format!(
        "anarlog-cloudsync-e2e-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let auth = || CloudsyncAuth::ApiKey {
        api_key: std::env::var("ANARLOG_CLOUDSYNC_API_KEY")
            .expect("ANARLOG_CLOUDSYNC_API_KEY must be set"),
    };
    let db_a = setup_db(auth(), None).await;
    let workspace_id = ensure_cloudsync_workspace_binding(db_a.pool())
        .await
        .unwrap();

    sqlx::query("INSERT INTO sessions (id, workspace_id, title) VALUES (cloudsync_uuid(), ?, ?)")
        .bind(&workspace_id)
        .bind(&marker)
        .execute(db_a.pool())
        .await
        .unwrap();
    tokio::time::timeout(SYNC_TIMEOUT, db_a.cloudsync_trigger_sync())
        .await
        .expect("first client sync timed out")
        .unwrap();

    let db_b = setup_db(auth(), None).await;
    for _ in 0..2 {
        tokio::time::timeout(SYNC_TIMEOUT, db_b.cloudsync_trigger_sync())
            .await
            .expect("second client sync timed out")
            .unwrap();
    }

    let title: Option<String> =
        sqlx::query_scalar("SELECT title FROM sessions WHERE title = ? LIMIT 1")
            .bind(&marker)
            .fetch_optional(db_b.pool())
            .await
            .unwrap();

    assert_eq!(title.as_deref(), Some(marker.as_str()));

    sqlx::query("DELETE FROM sessions WHERE title = ?")
        .bind(&marker)
        .execute(db_a.pool())
        .await
        .unwrap();
    tokio::time::timeout(SYNC_TIMEOUT, db_a.cloudsync_trigger_sync())
        .await
        .expect("cleanup sync timed out")
        .unwrap();

    tokio::time::timeout(Duration::from_secs(15), db_a.cloudsync_stop())
        .await
        .expect("first client stop timed out")
        .unwrap();
    tokio::time::timeout(Duration::from_secs(15), db_b.cloudsync_stop())
        .await
        .expect("second client stop timed out")
        .unwrap();
}

#[tokio::test]
#[ignore = "external smoke test only; creates four dev devices and requires two short-lived access tokens"]
async fn access_tokens_sync_two_clients_and_isolate_workspace() {
    let workspace_a = std::env::var("ANARLOG_CLOUDSYNC_WORKSPACE_A")
        .expect("ANARLOG_CLOUDSYNC_WORKSPACE_A must be set");
    let workspace_b = std::env::var("ANARLOG_CLOUDSYNC_WORKSPACE_B")
        .expect("ANARLOG_CLOUDSYNC_WORKSPACE_B must be set");
    assert_ne!(workspace_a, workspace_b);

    let token_a =
        std::env::var("ANARLOG_CLOUDSYNC_TOKEN_A").expect("ANARLOG_CLOUDSYNC_TOKEN_A must be set");
    let token_b =
        std::env::var("ANARLOG_CLOUDSYNC_TOKEN_B").expect("ANARLOG_CLOUDSYNC_TOKEN_B must be set");
    let auth_a = || CloudsyncAuth::Token {
        token: token_a.clone(),
    };
    let db_a1 = setup_db(auth_a(), Some(&workspace_a)).await;
    let db_a2 = setup_db(auth_a(), Some(&workspace_a)).await;
    let db_b = setup_db(
        CloudsyncAuth::Token {
            token: token_b.clone(),
        },
        Some(&workspace_b),
    )
    .await;

    let marker = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let session_a: String = sqlx::query_scalar("SELECT cloudsync_uuid()")
        .fetch_one(db_a1.pool())
        .await
        .unwrap();
    let title_a = format!("anarlog-smoke-a-{marker}");
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title) VALUES (?, ?, ?, ?)",
    )
    .bind(&session_a)
    .bind(&workspace_a)
    .bind(&workspace_a)
    .bind(&title_a)
    .execute(db_a1.pool())
    .await
    .unwrap();
    sync_ok(&db_a1, "workspace A client 1 upload").await;
    sync_ok(&db_a2, "workspace A client 2 download").await;
    sync_ok(&db_a2, "workspace A client 2 download retry").await;
    let a1_uploaded = sqlx::query_scalar::<_, String>("SELECT title FROM sessions WHERE id = ?")
        .bind(&session_a)
        .fetch_optional(db_a2.pool())
        .await
        .unwrap()
        .as_deref()
        == Some(title_a.as_str());

    let updated_title_a = format!("anarlog-smoke-a-updated-{marker}");
    sqlx::query("UPDATE sessions SET title = ? WHERE id = ?")
        .bind(&updated_title_a)
        .bind(&session_a)
        .execute(db_a2.pool())
        .await
        .unwrap();
    sync_ok(&db_a2, "workspace A client 2 update").await;
    sync_ok(&db_a1, "workspace A client 1 update download").await;
    sync_ok(&db_a1, "workspace A client 1 update download retry").await;
    let a2_uploaded = sqlx::query_scalar::<_, String>("SELECT title FROM sessions WHERE id = ?")
        .bind(&session_a)
        .fetch_optional(db_a1.pool())
        .await
        .unwrap()
        .as_deref()
        == Some(updated_title_a.as_str());

    let session_b: String = sqlx::query_scalar("SELECT cloudsync_uuid()")
        .fetch_one(db_b.pool())
        .await
        .unwrap();
    let title_b = format!("anarlog-smoke-b-{marker}");
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title) VALUES (?, ?, ?, ?)",
    )
    .bind(&session_b)
    .bind(&workspace_b)
    .bind(&workspace_b)
    .bind(&title_b)
    .execute(db_b.pool())
    .await
    .unwrap();
    sync_ok(&db_b, "workspace B upload").await;

    let db_b_verifier = setup_db(
        CloudsyncAuth::Token {
            token: token_b.clone(),
        },
        Some(&workspace_b),
    )
    .await;
    sync_ok(&db_b_verifier, "workspace B verifier download").await;
    sync_ok(&db_b_verifier, "workspace B verifier download retry").await;
    let b_uploaded = sqlx::query_scalar::<_, String>("SELECT title FROM sessions WHERE id = ?")
        .bind(&session_b)
        .fetch_optional(db_b_verifier.pool())
        .await
        .unwrap()
        .as_deref()
        == Some(title_b.as_str());

    db_a2
        .cloudsync_network_reset_sync_version()
        .await
        .expect("workspace A isolation sync reset failed");
    sync_ok(&db_a2, "workspace A isolation download").await;
    sync_ok(&db_a2, "workspace A isolation download retry").await;
    let b_hidden_from_a = row_count_by_id(db_a2.pool(), "sessions", &session_b).await == 0;

    let pending_session: String = sqlx::query_scalar("SELECT cloudsync_uuid()")
        .fetch_one(db_a1.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title) VALUES (?, ?, ?, ?)",
    )
    .bind(&pending_session)
    .bind(&workspace_a)
    .bind(&workspace_a)
    .bind(format!("anarlog-smoke-pending-{marker}"))
    .execute(db_a1.pool())
    .await
    .unwrap();
    let unsent_logout_rejected = matches!(
        db_a1.cloudsync_logout(false).await,
        Err(CloudsyncRuntimeError::UnsentChanges)
    );
    sync_ok(&db_a1, "workspace A pending upload").await;
    sqlx::query("DELETE FROM sessions WHERE id = ? OR id = ?")
        .bind(&session_a)
        .bind(&pending_session)
        .execute(db_a1.pool())
        .await
        .unwrap();
    sync_ok(&db_a1, "workspace A cleanup").await;
    db_a2
        .cloudsync_network_reset_sync_version()
        .await
        .expect("workspace A cleanup sync reset failed");
    sync_ok(&db_a2, "workspace A cleanup verification").await;
    sync_ok(&db_a2, "workspace A cleanup verification retry").await;
    let a_cleanup_counts = (
        row_count_by_id(db_a2.pool(), "sessions", &session_a).await,
        row_count_by_id(db_a2.pool(), "sessions", &pending_session).await,
    );
    let a1_logout =
        match tokio::time::timeout(Duration::from_secs(15), db_a1.cloudsync_logout(false)).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(error.to_string()),
            Err(_) => Err("timed out".to_string()),
        };
    if a1_logout.is_err() {
        let _ = tokio::time::timeout(Duration::from_secs(15), db_a1.cloudsync_logout(true)).await;
    }

    let foreign_session: String = sqlx::query_scalar("SELECT cloudsync_uuid()")
        .fetch_one(db_a2.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title) VALUES (?, ?, ?, ?)",
    )
    .bind(&foreign_session)
    .bind(&workspace_b)
    .bind(&workspace_b)
    .bind(format!("anarlog-smoke-foreign-{marker}"))
    .execute(db_a2.pool())
    .await
    .unwrap();
    let foreign_sync = expect_policy_sync_completed_or_denied(&db_a2, "sessions").await;
    db_b_verifier
        .cloudsync_network_reset_sync_version()
        .await
        .expect("workspace B foreign-write sync reset failed");
    sync_ok(&db_b_verifier, "workspace B foreign-write check").await;
    sync_ok(&db_b_verifier, "workspace B foreign-write check retry").await;
    let foreign_write_blocked =
        row_count_by_id(db_b_verifier.pool(), "sessions", &foreign_session).await == 0;

    sqlx::query("DELETE FROM sessions WHERE id = ? OR id = ?")
        .bind(&session_b)
        .bind(&foreign_session)
        .execute(db_b_verifier.pool())
        .await
        .unwrap();
    sync_ok(&db_b_verifier, "workspace B cleanup").await;
    db_b.cloudsync_network_reset_sync_version()
        .await
        .expect("workspace B cleanup sync reset failed");
    sync_ok(&db_b, "workspace B cleanup verification").await;
    sync_ok(&db_b, "workspace B cleanup verification retry").await;
    let b_cleanup_counts = (
        row_count_by_id(db_b.pool(), "sessions", &session_b).await,
        row_count_by_id(db_b.pool(), "sessions", &foreign_session).await,
    );

    let b_logout =
        match tokio::time::timeout(Duration::from_secs(15), db_b.cloudsync_logout(false)).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(error.to_string()),
            Err(_) => Err("timed out".to_string()),
        };
    let b_verifier_logout = match tokio::time::timeout(
        Duration::from_secs(15),
        db_b_verifier.cloudsync_logout(false),
    )
    .await
    {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(error.to_string()),
        Err(_) => Err("timed out".to_string()),
    };
    let a2_logout =
        match tokio::time::timeout(Duration::from_secs(15), db_a2.cloudsync_logout(true)).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(error.to_string()),
            Err(_) => Err("timed out".to_string()),
        };

    assert!(a1_uploaded, "workspace A client 1 upload was not visible");
    assert!(a2_uploaded, "workspace A client 2 update was not visible");
    assert!(b_uploaded, "workspace B upload was not visible remotely");
    assert!(b_hidden_from_a, "workspace A downloaded workspace B data");
    assert!(
        unsent_logout_rejected,
        "logout did not reject workspace A's unsent change"
    );
    assert_eq!(
        a_cleanup_counts,
        (0, 0),
        "workspace A cleanup was not visible"
    );
    assert!(
        foreign_sync.is_ok(),
        "workspace A foreign write had an unexpected sync result: {:?}",
        foreign_sync.err()
    );
    assert!(foreign_write_blocked, "workspace A wrote into workspace B");
    assert_eq!(
        b_cleanup_counts,
        (0, 0),
        "workspace B cleanup was not visible"
    );
    assert!(
        a1_logout.is_ok(),
        "workspace A client 1 logout failed: {a1_logout:?}"
    );
    assert!(
        a2_logout.is_ok(),
        "workspace A client 2 logout failed: {a2_logout:?}"
    );
    assert!(
        b_logout.is_ok(),
        "workspace B client logout failed: {b_logout:?}"
    );
    assert!(
        b_verifier_logout.is_ok(),
        "workspace B verifier logout failed: {b_verifier_logout:?}"
    );
}

#[tokio::test]
#[ignore = "external verification only; requires two short-lived anarlog-dev access tokens"]
async fn access_tokens_isolate_two_workspaces() {
    let workspace_a = std::env::var("ANARLOG_CLOUDSYNC_WORKSPACE_A")
        .expect("ANARLOG_CLOUDSYNC_WORKSPACE_A must be set");
    let workspace_b = std::env::var("ANARLOG_CLOUDSYNC_WORKSPACE_B")
        .expect("ANARLOG_CLOUDSYNC_WORKSPACE_B must be set");
    assert_ne!(workspace_a, workspace_b);

    let token_a =
        std::env::var("ANARLOG_CLOUDSYNC_TOKEN_A").expect("ANARLOG_CLOUDSYNC_TOKEN_A must be set");
    let token_b =
        std::env::var("ANARLOG_CLOUDSYNC_TOKEN_B").expect("ANARLOG_CLOUDSYNC_TOKEN_B must be set");
    let db_a = setup_db(
        CloudsyncAuth::Token {
            token: token_a.clone(),
        },
        Some(&workspace_a),
    )
    .await;
    let db_b = setup_db(
        CloudsyncAuth::Token {
            token: token_b.clone(),
        },
        Some(&workspace_b),
    )
    .await;

    let marker = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let fixture_a_value = format!("anarlog-tenant-a-{marker}");
    let fixture_b_value = format!("anarlog-tenant-b-{marker}");
    let fixture_a = insert_synced_fixture(db_a.pool(), &workspace_a, &fixture_a_value).await;
    sync_ok(&db_a, "workspace A upload").await;
    let fixture_b = insert_synced_fixture(db_b.pool(), &workspace_b, &fixture_b_value).await;
    sync_ok(&db_b, "workspace B upload").await;

    let db_a_fresh = setup_db(
        CloudsyncAuth::Token {
            token: token_a.clone(),
        },
        Some(&workspace_a),
    )
    .await;
    let db_b_fresh = setup_db(
        CloudsyncAuth::Token {
            token: token_b.clone(),
        },
        Some(&workspace_b),
    )
    .await;
    for (db, label) in [
        (&db_a_fresh, "fresh workspace A download"),
        (&db_b_fresh, "fresh workspace B download"),
    ] {
        sync_ok(db, label).await;
        sync_ok(db, label).await;
    }

    assert_fixture_visibility(
        db_a_fresh.pool(),
        &workspace_a,
        &fixture_a,
        &workspace_b,
        &fixture_b,
    )
    .await;
    assert_fixture_visibility(
        db_b_fresh.pool(),
        &workspace_b,
        &fixture_b,
        &workspace_a,
        &fixture_a,
    )
    .await;

    let mut foreign_rows = Vec::with_capacity(SYNCED_TABLES.len());
    let mut policy_failures = Vec::new();
    for table in SYNCED_TABLES {
        let attacker = setup_policy_db(
            CloudsyncAuth::Token {
                token: token_a.clone(),
            },
            &workspace_a,
        )
        .await;
        let foreign_id = insert_foreign_row(
            attacker.pool(),
            table,
            &workspace_b,
            &fixture_b,
            &format!("anarlog-foreign-{table}-{marker}"),
        )
        .await;
        if let Err(error) = expect_policy_sync_completed_or_denied(&attacker, table).await {
            policy_failures.push(error);
        }
        foreign_rows.push((table, foreign_id));
        tokio::time::timeout(Duration::from_secs(15), attacker.cloudsync_stop())
            .await
            .unwrap_or_else(|_| panic!("{table} policy client stop timed out"))
            .unwrap();
    }

    for (table, id) in fixture_b.rows() {
        let attacker = setup_stale_policy_db(&token_b, &token_a, &workspace_b, table, id).await;
        update_stale_foreign_row(
            attacker.pool(),
            table,
            id,
            &format!("anarlog-foreign-update-{table}-{marker}"),
        )
        .await;
        assert_policy_change_pending(&attacker, &format!("{table} UPDATE")).await;
        if let Err(error) =
            expect_policy_sync_completed_or_denied(&attacker, &format!("{table} UPDATE")).await
        {
            policy_failures.push(error);
        }
        tokio::time::timeout(Duration::from_secs(15), attacker.cloudsync_stop())
            .await
            .unwrap_or_else(|_| panic!("{table} UPDATE policy client stop timed out"))
            .unwrap();
    }

    for (table, id) in fixture_b.rows_child_first() {
        let attacker = setup_stale_policy_db(&token_b, &token_a, &workspace_b, table, id).await;
        delete_stale_foreign_row(attacker.pool(), table, id).await;
        assert_policy_change_pending(&attacker, &format!("{table} DELETE")).await;
        if let Err(error) =
            expect_policy_sync_completed_or_denied(&attacker, &format!("{table} DELETE")).await
        {
            policy_failures.push(error);
        }
        tokio::time::timeout(Duration::from_secs(15), attacker.cloudsync_stop())
            .await
            .unwrap_or_else(|_| panic!("{table} DELETE policy client stop timed out"))
            .unwrap();
    }

    let db_b_verifier = setup_db(
        CloudsyncAuth::Token {
            token: token_b.clone(),
        },
        Some(&workspace_b),
    )
    .await;
    db_b_verifier
        .cloudsync_network_reset_sync_version()
        .await
        .expect("workspace B verifier sync reset failed");
    sync_ok(&db_b_verifier, "foreign-write full snapshot check").await;
    sync_ok(&db_b_verifier, "foreign-write full snapshot check").await;
    let fixture_b_violations = fixture_state_violations(
        db_b_verifier.pool(),
        &workspace_b,
        &fixture_b,
        &fixture_b_value,
    )
    .await;
    let mut leaked_tables = Vec::new();
    for (table, id) in &foreign_rows {
        if row_count(db_b_verifier.pool(), table, id, &workspace_b).await != 0 {
            leaked_tables.push(*table);
        }
    }
    if !leaked_tables.is_empty() {
        let mut transaction = db_b_verifier.pool().begin().await.unwrap();
        for (table, id) in foreign_rows.iter().rev() {
            sqlx::query(AssertSqlSafe(format!("DELETE FROM {table} WHERE id = ?")))
                .bind(id)
                .execute(&mut *transaction)
                .await
                .unwrap();
        }
        transaction.commit().await.unwrap();
        sync_ok(&db_b_verifier, "leaked foreign-write cleanup").await;
    }

    delete_synced_fixture(db_a.pool(), &fixture_a).await;
    delete_synced_fixture(db_b.pool(), &fixture_b).await;
    sync_ok(&db_a, "workspace A cleanup").await;
    sync_ok(&db_b, "workspace B cleanup").await;

    for db in [&db_a, &db_b, &db_a_fresh, &db_b_fresh, &db_b_verifier] {
        tokio::time::timeout(Duration::from_secs(15), db.cloudsync_stop())
            .await
            .expect("tenant client stop timed out")
            .unwrap();
    }

    assert!(policy_failures.is_empty(), "{}", policy_failures.join("\n"));
    assert!(
        leaked_tables.is_empty(),
        "workspace A wrote into workspace B tables: {}",
        leaked_tables.join(", ")
    );
    assert!(
        fixture_b_violations.is_empty(),
        "workspace A mutated workspace B rows: {}",
        fixture_b_violations.join(", ")
    );
}

#[tokio::test]
#[ignore = "external verification only; requires ANARLOG_CLOUDSYNC_DATABASE_ID, ANARLOG_CLOUDSYNC_WORKSPACE_A/B/C, ANARLOG_CLOUDSYNC_TOKEN_A_WITH_SHARED_C, ANARLOG_CLOUDSYNC_TOKEN_A_WITHOUT_SHARED_C, ANARLOG_CLOUDSYNC_TOKEN_B, and ANARLOG_CLOUDSYNC_TOKEN_C_OWNER"]
async fn access_token_workspace_attributes_allow_shared_reads_but_not_writes() {
    let workspace_a = std::env::var("ANARLOG_CLOUDSYNC_WORKSPACE_A")
        .expect("ANARLOG_CLOUDSYNC_WORKSPACE_A must be set");
    let workspace_b = std::env::var("ANARLOG_CLOUDSYNC_WORKSPACE_B")
        .expect("ANARLOG_CLOUDSYNC_WORKSPACE_B must be set");
    let workspace_c = std::env::var("ANARLOG_CLOUDSYNC_WORKSPACE_C")
        .expect("ANARLOG_CLOUDSYNC_WORKSPACE_C must be set");
    assert_ne!(workspace_a, workspace_b);
    assert_ne!(workspace_a, workspace_c);
    assert_ne!(workspace_b, workspace_c);

    let token_a_with_shared_c = std::env::var("ANARLOG_CLOUDSYNC_TOKEN_A_WITH_SHARED_C")
        .expect("ANARLOG_CLOUDSYNC_TOKEN_A_WITH_SHARED_C must be set");
    let token_a_without_shared_c = std::env::var("ANARLOG_CLOUDSYNC_TOKEN_A_WITHOUT_SHARED_C")
        .expect("ANARLOG_CLOUDSYNC_TOKEN_A_WITHOUT_SHARED_C must be set");
    let token_b =
        std::env::var("ANARLOG_CLOUDSYNC_TOKEN_B").expect("ANARLOG_CLOUDSYNC_TOKEN_B must be set");
    let token_c_owner = std::env::var("ANARLOG_CLOUDSYNC_TOKEN_C_OWNER")
        .expect("ANARLOG_CLOUDSYNC_TOKEN_C_OWNER must be set");

    let db_a_owner = setup_db(
        CloudsyncAuth::Token {
            token: token_a_without_shared_c.clone(),
        },
        Some(&workspace_a),
    )
    .await;
    let db_b_owner = setup_db(
        CloudsyncAuth::Token {
            token: token_b.clone(),
        },
        Some(&workspace_b),
    )
    .await;
    let db_c_owner = setup_db(
        CloudsyncAuth::Token {
            token: token_c_owner.clone(),
        },
        Some(&workspace_c),
    )
    .await;

    let marker = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let session_a: String = sqlx::query_scalar("SELECT cloudsync_uuid()")
        .fetch_one(db_a_owner.pool())
        .await
        .unwrap();
    let session_b: String = sqlx::query_scalar("SELECT cloudsync_uuid()")
        .fetch_one(db_b_owner.pool())
        .await
        .unwrap();
    let session_c: String = sqlx::query_scalar("SELECT cloudsync_uuid()")
        .fetch_one(db_c_owner.pool())
        .await
        .unwrap();
    let foreign_insert_c: String = sqlx::query_scalar("SELECT cloudsync_uuid()")
        .fetch_one(db_a_owner.pool())
        .await
        .unwrap();
    let title_a = format!("anarlog-workspace-attributes-a-{marker}");
    let title_b = format!("anarlog-workspace-attributes-b-{marker}");
    let title_c = format!("anarlog-workspace-attributes-c-{marker}");

    for (db, id, workspace_id, title) in [
        (&db_a_owner, &session_a, &workspace_a, &title_a),
        (&db_b_owner, &session_b, &workspace_b, &title_b),
        (&db_c_owner, &session_c, &workspace_c, &title_c),
    ] {
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title) VALUES (?, ?, ?, ?)",
        )
        .bind(id)
        .bind(workspace_id)
        .bind(workspace_id)
        .bind(title)
        .execute(db.pool())
        .await
        .unwrap();
    }
    sync_ok(&db_a_owner, "workspace attribute A fixture upload").await;
    sync_ok(&db_b_owner, "workspace attribute B fixture upload").await;
    sync_ok(&db_c_owner, "workspace attribute C fixture upload").await;

    let db_a_shared = setup_db(
        CloudsyncAuth::Token {
            token: token_a_with_shared_c.clone(),
        },
        Some(&workspace_a),
    )
    .await;
    let db_b_reader = setup_db(
        CloudsyncAuth::Token {
            token: token_b.clone(),
        },
        Some(&workspace_b),
    )
    .await;
    for (db, label) in [
        (&db_a_shared, "workspace A shared-read snapshot"),
        (&db_b_reader, "workspace B isolation snapshot"),
    ] {
        sync_ok(db, label).await;
        sync_ok(db, label).await;
    }

    let a_reads_personal =
        row_count(db_a_shared.pool(), "sessions", &session_a, &workspace_a).await == 1;
    let a_reads_shared_c =
        row_count(db_a_shared.pool(), "sessions", &session_c, &workspace_c).await == 1;
    let a_cannot_read_b = row_count_by_id(db_a_shared.pool(), "sessions", &session_b).await == 0;
    let b_reads_personal =
        row_count(db_b_reader.pool(), "sessions", &session_b, &workspace_b).await == 1;
    let b_cannot_read_c = row_count_by_id(db_b_reader.pool(), "sessions", &session_c).await == 0;

    let insert_attacker = setup_policy_db(
        CloudsyncAuth::Token {
            token: token_a_with_shared_c.clone(),
        },
        &workspace_a,
    )
    .await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title) VALUES (?, ?, ?, ?)",
    )
    .bind(&foreign_insert_c)
    .bind(&workspace_c)
    .bind(&workspace_c)
    .bind(format!(
        "anarlog-workspace-attributes-foreign-insert-{marker}"
    ))
    .execute(insert_attacker.pool())
    .await
    .unwrap();
    let insert_policy =
        expect_policy_sync_completed_or_denied(&insert_attacker, "shared workspace C INSERT").await;

    let update_attacker = setup_snapshot_policy_db(
        &token_a_with_shared_c,
        &workspace_a,
        &workspace_c,
        "sessions",
        &session_c,
    )
    .await;
    let update_rows = sqlx::query("UPDATE sessions SET title = ? WHERE id = ?")
        .bind(format!(
            "anarlog-workspace-attributes-foreign-update-{marker}"
        ))
        .bind(&session_c)
        .execute(update_attacker.pool())
        .await
        .unwrap()
        .rows_affected();
    let update_policy = if update_rows == 1 {
        expect_policy_sync_completed_or_denied(&update_attacker, "shared workspace C UPDATE").await
    } else {
        Err("shared workspace C was not available for the UPDATE attempt".to_string())
    };

    let delete_attacker = setup_snapshot_policy_db(
        &token_a_with_shared_c,
        &workspace_a,
        &workspace_c,
        "sessions",
        &session_c,
    )
    .await;
    let delete_rows = sqlx::query("DELETE FROM sessions WHERE id = ?")
        .bind(&session_c)
        .execute(delete_attacker.pool())
        .await
        .unwrap()
        .rows_affected();
    let delete_policy = if delete_rows == 1 {
        expect_policy_sync_completed_or_denied(&delete_attacker, "shared workspace C DELETE").await
    } else {
        Err("shared workspace C was not available for the DELETE attempt".to_string())
    };

    let db_c_verifier = setup_db(
        CloudsyncAuth::Token {
            token: token_c_owner.clone(),
        },
        Some(&workspace_c),
    )
    .await;
    db_c_verifier
        .cloudsync_network_reset_sync_version()
        .await
        .expect("workspace C verification sync reset failed");
    sync_ok(
        &db_c_verifier,
        "shared workspace C write-policy verification",
    )
    .await;
    sync_ok(
        &db_c_verifier,
        "shared workspace C write-policy verification retry",
    )
    .await;
    let verified_title_c: Option<String> =
        sqlx::query_scalar("SELECT title FROM sessions WHERE id = ?")
            .bind(&session_c)
            .fetch_optional(db_c_verifier.pool())
            .await
            .unwrap();
    let foreign_insert_count =
        row_count_by_id(db_c_verifier.pool(), "sessions", &foreign_insert_c).await;

    db_a_shared
        .cloudsync_reconfigure(cloudsync_config(
            CloudsyncAuth::Token {
                token: token_a_without_shared_c.clone(),
            },
            5_000,
            3,
        ))
        .await
        .expect("same-client workspace A token reconfiguration failed");
    if db_a_shared
        .cloudsync_status()
        .await
        .expect("same-client workspace A status failed")
        .has_unsent_changes
        .unwrap_or(true)
    {
        sync_ok(&db_a_shared, "same-client workspace A pre-revocation drain").await;
    }
    db_a_shared
        .cloudsync_logout(false)
        .await
        .expect("same-client workspace A replica logout failed");
    let revoked_a_was_purged = row_count_by_id(db_a_shared.pool(), "sessions", &session_a).await
        == 0
        && row_count_by_id(db_a_shared.pool(), "sessions", &session_c).await == 0;

    db_a_shared
        .cloudsync_configure(cloudsync_config(
            CloudsyncAuth::Token {
                token: token_a_without_shared_c.clone(),
            },
            5_000,
            3,
        ))
        .await
        .expect("same-client workspace A post-logout configuration failed");
    db_a_shared
        .cloudsync_start()
        .await
        .expect("same-client workspace A post-logout start failed");
    db_a_shared
        .cloudsync_network_reset_sync_version()
        .await
        .expect("same-client workspace A token sync reset failed");
    sync_ok(
        &db_a_shared,
        "same-client workspace A token without C snapshot",
    )
    .await;
    sync_ok(
        &db_a_shared,
        "same-client workspace A token without C snapshot retry",
    )
    .await;
    let revoked_a_reads_personal =
        row_count(db_a_shared.pool(), "sessions", &session_a, &workspace_a).await == 1;
    let revoked_a_cannot_read_c =
        row_count_by_id(db_a_shared.pool(), "sessions", &session_c).await == 0;

    sqlx::query("DELETE FROM sessions WHERE id = ?")
        .bind(&session_a)
        .execute(db_a_owner.pool())
        .await
        .unwrap();
    sqlx::query("DELETE FROM sessions WHERE id = ?")
        .bind(&session_b)
        .execute(db_b_owner.pool())
        .await
        .unwrap();
    sqlx::query("DELETE FROM sessions WHERE id = ? OR id = ?")
        .bind(&session_c)
        .bind(&foreign_insert_c)
        .execute(db_c_verifier.pool())
        .await
        .unwrap();
    sync_ok(&db_a_owner, "workspace attribute A fixture cleanup").await;
    sync_ok(&db_b_owner, "workspace attribute B fixture cleanup").await;
    sync_ok(&db_c_verifier, "workspace attribute C fixture cleanup").await;

    for (db, label) in [
        (&db_a_owner, "workspace A owner"),
        (&db_b_owner, "workspace B owner"),
        (&db_c_owner, "workspace C owner"),
        (&db_a_shared, "workspace A shared reader"),
        (&db_b_reader, "workspace B reader"),
        (&insert_attacker, "workspace C insert attacker"),
        (&update_attacker, "workspace C update attacker"),
        (&delete_attacker, "workspace C delete attacker"),
        (&db_c_verifier, "workspace C verifier"),
    ] {
        tokio::time::timeout(Duration::from_secs(15), db.cloudsync_stop())
            .await
            .unwrap_or_else(|_| panic!("{label} stop timed out"))
            .unwrap();
    }

    assert!(
        a_reads_personal,
        "workspace A could not read its personal row"
    );
    assert!(
        a_reads_shared_c,
        "workspace A could not read shared workspace C"
    );
    assert!(a_cannot_read_b, "workspace A read unrelated workspace B");
    assert!(
        b_reads_personal,
        "workspace B could not read its personal row"
    );
    assert!(b_cannot_read_c, "workspace B read shared workspace C");
    assert!(
        insert_policy.is_ok(),
        "workspace A shared INSERT returned an unexpected sync result: {:?}",
        insert_policy.err()
    );
    assert!(
        update_policy.is_ok(),
        "workspace A shared UPDATE returned an unexpected sync result: {:?}",
        update_policy.err()
    );
    assert!(
        delete_policy.is_ok(),
        "workspace A shared DELETE returned an unexpected sync result: {:?}",
        delete_policy.err()
    );
    assert_eq!(
        verified_title_c.as_deref(),
        Some(title_c.as_str()),
        "workspace A updated or deleted workspace C's row"
    );
    assert_eq!(
        foreign_insert_count, 0,
        "workspace A inserted a row into workspace C"
    );
    assert!(
        revoked_a_was_purged,
        "same-client workspace A logout did not purge its prior replica"
    );
    assert!(
        revoked_a_reads_personal,
        "same-client workspace A token could not restore its personal row"
    );
    assert!(
        revoked_a_cannot_read_c,
        "same-client workspace A token without C restored workspace C"
    );
}
