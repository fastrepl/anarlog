use super::*;

const EIGHT_AM: i64 = 1_700_000_000_000;
const NINE_AM: i64 = EIGHT_AM + 3_600_000;

fn words(range: std::ops::Range<usize>) -> Vec<Value> {
    range
        .map(|index| json!({ "id": format!("w{index}"), "text": format!("word{index}"), "start_ms": index * 500, "end_ms": index * 500 + 400, "channel": 0 }))
        .collect()
}

fn words_json(items: &[Value]) -> String {
    Value::Array(items.to_vec()).to_string()
}

async fn seed_transcript(
    workspace_keys: &HashMap<String, anlg_e2ee::WorkspaceKeyring>,
    items: &[Value],
) -> (anlg_db_core::Db, anlg_db_core::Db) {
    let a = test_db().await;
    let b = test_db().await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
         VALUES ('session-1', 'workspace-a', 'user-a', 'Meeting')",
    )
    .execute(a.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO transcripts (id, workspace_id, owner_user_id, session_id, words_json)
         VALUES ('transcript-1', 'workspace-a', 'user-a', 'session-1', ?)",
    )
    .bind(words_json(items))
    .execute(a.pool())
    .await
    .unwrap();
    // Later edits use fixed times, so the seed must predate them.
    sqlx::query("UPDATE e2ee_dirty_rows SET dirtied_at_ms = ?")
        .bind(EIGHT_AM - 3_600_000)
        .execute(a.pool())
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(a.pool(), workspace_keys)
        .await
        .unwrap();
    copy_replica(a.pool(), b.pool()).await;
    apply_e2ee_replica_changes(b.pool(), workspace_keys)
        .await
        .unwrap();
    assert_eq!(read_words(&b).await, items);
    (a, b)
}

async fn read_words(db: &anlg_db_core::Db) -> Vec<Value> {
    let json: String =
        sqlx::query_scalar("SELECT words_json FROM transcripts WHERE id = 'transcript-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    serde_json::from_str::<Value>(&json)
        .unwrap()
        .as_array()
        .cloned()
        .unwrap()
}

async fn write_words(db: &anlg_db_core::Db, items: &[Value], edited_at_ms: i64) {
    sqlx::query("UPDATE transcripts SET words_json = ? WHERE id = 'transcript-1'")
        .bind(words_json(items))
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(
        "UPDATE e2ee_dirty_rows SET dirtied_at_ms = ?
         WHERE table_name = 'transcripts' AND row_id = 'transcript-1'",
    )
    .bind(edited_at_ms)
    .execute(db.pool())
    .await
    .unwrap();
}

async fn chunk_state_revisions(db: &anlg_db_core::Db) -> HashMap<String, i64> {
    sqlx::query_as::<_, (String, i64)>(
        "SELECT field_name, revision FROM e2ee_local_state
         WHERE table_name = 'transcripts' AND row_id = 'transcript-1' AND field_name LIKE 'words_json%'",
    )
    .fetch_all(db.pool())
    .await
    .unwrap()
    .into_iter()
    .collect()
}

async fn chunk_state_fields(db: &anlg_db_core::Db) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT field_name FROM e2ee_local_state
         WHERE table_name = 'transcripts' AND row_id = 'transcript-1' AND field_name LIKE 'words_json%'
         ORDER BY field_name",
    )
    .fetch_all(db.pool())
    .await
    .unwrap()
}

#[tokio::test]
async fn initial_transcript_chunks_must_hydrate_before_local_edits_are_encrypted() {
    for missing_field in ["words_json#n", "words_json#1"] {
        for rotate_key in [false, true] {
            let mut workspace_keys = keys("workspace-a");
            let key = workspace_keys["workspace-a"].active().clone();
            let items = words(0..600);
            let (a, _) = seed_transcript(&workspace_keys, &items).await;
            let b = test_db().await;
            copy_replica(a.pool(), b.pool()).await;
            sqlx::query("DELETE FROM e2ee_records WHERE id = ?")
                .bind(key.blind_field_id("transcripts", "transcript-1", missing_field))
                .execute(b.pool())
                .await
                .unwrap();
            if rotate_key {
                let recovery = RecoveryKey::generate().unwrap();
                let mut keyring: anlg_e2ee::WorkspaceKeyring =
                    recovery.workspace_key("workspace-a").unwrap().into();
                keyring.insert_retired(key.clone());
                workspace_keys.insert("workspace-a".to_string(), keyring);
            }
            let applied = apply_e2ee_replica_changes(b.pool(), &workspace_keys)
                .await
                .unwrap();
            assert!(applied.incomplete_chunk_columns > 0);
            assert!(read_words(&b).await.is_empty());
            sqlx::query(
                "UPDATE transcripts SET created_at = 'local edit' WHERE id = 'transcript-1'",
            )
            .execute(b.pool())
            .await
            .unwrap();
            let before: Vec<(String, String)> =
                sqlx::query_as("SELECT id, payload FROM e2ee_records ORDER BY id")
                    .fetch_all(b.pool())
                    .await
                    .unwrap();
            let encrypted = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                encrypt_e2ee_replica_changes(b.pool(), &workspace_keys),
            )
            .await
            .expect("encryption spun on an incomplete transcript")
            .unwrap();
            assert!(encrypted.remaining_replica_changes);
            assert_eq!(encrypted.encrypted_fields, 0);
            let after: Vec<(String, String)> =
                sqlx::query_as("SELECT id, payload FROM e2ee_records ORDER BY id")
                    .fetch_all(b.pool())
                    .await
                    .unwrap();
            assert_eq!(before, after);
            copy_replica(a.pool(), b.pool()).await;
            apply_e2ee_replica_changes(b.pool(), &workspace_keys)
                .await
                .unwrap();
            assert_eq!(read_words(&b).await, items);
            let encrypted = encrypt_e2ee_replica_changes(b.pool(), &workspace_keys)
                .await
                .unwrap();
            assert!(!encrypted.remaining_replica_changes);
            assert!(encrypted.encrypted_fields > 0);
            assert_eq!(
                sqlx::query_scalar::<_, String>(
                    "SELECT created_at FROM transcripts WHERE id = 'transcript-1'"
                )
                .fetch_one(b.pool())
                .await
                .unwrap(),
                "local edit"
            );
        }
    }
}

#[tokio::test]
async fn incomplete_transcript_pages_do_not_starve_later_local_edits() {
    let workspace_keys = keys("workspace-a");
    let key = workspace_keys["workspace-a"].active();
    let a = test_db().await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
         VALUES ('session-1', 'workspace-a', 'user-a', 'Meeting')",
    )
    .execute(a.pool())
    .await
    .unwrap();
    for index in 0..(E2EE_ENCRYPT_ROW_LIMIT * 2) {
        sqlx::query(
            "INSERT INTO transcripts (id, workspace_id, owner_user_id, session_id, words_json)
             VALUES (?, 'workspace-a', 'user-a', 'session-1', ?)",
        )
        .bind(format!("transcript-{index:03}"))
        .bind(words_json(&words(0..1)))
        .execute(a.pool())
        .await
        .unwrap();
    }
    encrypt_e2ee_replica_changes(a.pool(), &workspace_keys)
        .await
        .unwrap();

    for bounded in [true, false] {
        let b = test_db().await;
        copy_replica(a.pool(), b.pool()).await;
        for _ in 0..E2EE_ENCRYPT_ROW_LIMIT {
            if !apply_e2ee_replica_changes(b.pool(), &workspace_keys)
                .await
                .unwrap()
                .remaining_replica_changes
            {
                break;
            }
        }
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM transcripts")
                .fetch_one(b.pool())
                .await
                .unwrap(),
            E2EE_ENCRYPT_ROW_LIMIT * 2
        );
        for index in 0..(E2EE_ENCRYPT_ROW_LIMIT * 2) {
            sqlx::query("DELETE FROM e2ee_records WHERE id = ?")
                .bind(key.blind_field_id(
                    "transcripts",
                    &format!("transcript-{index:03}"),
                    "words_json#0",
                ))
                .execute(b.pool())
                .await
                .unwrap();
        }
        // Model more initially partial rows than a single hydration page can load.
        sqlx::query("DELETE FROM e2ee_local_state WHERE table_name = 'transcripts' AND field_name LIKE 'words_json%'")
        .execute(b.pool())
        .await
        .unwrap();
        sqlx::query("UPDATE transcripts SET words_json = '[]', created_at = 'local edit'")
            .execute(b.pool())
            .await
            .unwrap();
        for index in 0..3 {
            sqlx::query(
                "INSERT INTO transcripts (id, workspace_id, owner_user_id, session_id, words_json)
             VALUES (?, 'workspace-a', 'user-a', 'session-1', ?)",
            )
            .bind(format!("transcript-z{index}"))
            .bind(words_json(&words(0..2)))
            .execute(b.pool())
            .await
            .unwrap();
        }

        let stats = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            if bounded {
                encrypt_e2ee_replica_changes_bounded(b.pool(), &workspace_keys, 1).await
            } else {
                encrypt_e2ee_replica_changes(b.pool(), &workspace_keys).await
            }
        })
        .await
        .expect("encryption spun on deferred transcript pages")
        .unwrap();
        assert!(
            stats.encrypted_fields > 0,
            "later local edit was starved: {stats:?}"
        );
        assert!(stats.remaining_replica_changes);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM e2ee_dirty_rows")
                .fetch_one(b.pool())
                .await
                .unwrap(),
            E2EE_ENCRYPT_ROW_LIMIT * 2 + if bounded { 2 } else { 0 }
        );
        let restored = test_db().await;
        copy_replica(b.pool(), restored.pool()).await;
        copy_replica(a.pool(), restored.pool()).await;
        for _ in 0..E2EE_ENCRYPT_ROW_LIMIT {
            if !apply_e2ee_replica_changes(restored.pool(), &workspace_keys)
                .await
                .unwrap()
                .remaining_replica_changes
            {
                break;
            }
        }
        let value: String =
            sqlx::query_scalar("SELECT words_json FROM transcripts WHERE id = 'transcript-z0'")
                .fetch_one(restored.pool())
                .await
                .unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<Value>>(&value).unwrap(),
            words(0..2)
        );
    }
}

#[tokio::test]
async fn transcripts_sync_as_chunks_and_an_append_reseals_only_the_tail() {
    let workspace_keys = keys("workspace-a");
    let items = words(0..600);
    let (a, b) = seed_transcript(&workspace_keys, &items).await;

    let fields = chunk_state_fields(&a).await;
    assert_eq!(
        fields,
        vec![
            "words_json#0",
            "words_json#1",
            "words_json#2",
            "words_json#n"
        ]
    );
    assert_eq!(chunk_state_fields(&b).await, fields);

    let revisions_before = chunk_state_revisions(&a).await;
    let appended = words(0..610);
    write_words(&a, &appended, NINE_AM).await;
    let stats = encrypt_e2ee_replica_changes(a.pool(), &workspace_keys)
        .await
        .unwrap();
    // The last chunk, the row manifest, and bookkeeping columns; the earlier
    // chunks and the count are untouched.
    assert!(stats.encrypted_fields <= 4, "{stats:?}");
    let revisions_after = chunk_state_revisions(&a).await;
    assert_eq!(
        revisions_after["words_json#0"],
        revisions_before["words_json#0"]
    );
    assert_eq!(
        revisions_after["words_json#1"],
        revisions_before["words_json#1"]
    );
    assert_eq!(
        revisions_after["words_json#n"],
        revisions_before["words_json#n"]
    );
    assert!(revisions_after["words_json#2"] > revisions_before["words_json#2"]);

    copy_replica(a.pool(), b.pool()).await;
    apply_e2ee_replica_changes(b.pool(), &workspace_keys)
        .await
        .unwrap();
    assert_eq!(read_words(&b).await, appended);
}

#[tokio::test]
async fn edits_to_different_chunks_merge_on_both_devices() {
    let workspace_keys = keys("workspace-a");
    let items = words(0..300);
    let (a, b) = seed_transcript(&workspace_keys, &items).await;

    // Phone corrects a word in the first chunk while offline; desktop keeps
    // recording and appends to the second chunk.
    let mut phone = items.clone();
    phone[3]["text"] = json!("corrected");
    write_words(&b, &phone, EIGHT_AM).await;
    let mut desktop = items.clone();
    desktop.extend(words(300..320));
    write_words(&a, &desktop, NINE_AM).await;
    encrypt_e2ee_replica_changes(a.pool(), &workspace_keys)
        .await
        .unwrap();

    copy_replica(a.pool(), b.pool()).await;
    let stats = apply_e2ee_replica_changes(b.pool(), &workspace_keys)
        .await
        .unwrap();
    assert_eq!(stats.recorded_conflicts, 0);
    let mut expected = desktop.clone();
    expected[3]["text"] = json!("corrected");
    assert_eq!(read_words(&b).await, expected);

    encrypt_e2ee_replica_changes(b.pool(), &workspace_keys)
        .await
        .unwrap();
    copy_replica(b.pool(), a.pool()).await;
    apply_e2ee_replica_changes(a.pool(), &workspace_keys)
        .await
        .unwrap();
    assert_eq!(read_words(&a).await, expected);
}

#[tokio::test]
async fn shrinking_a_transcript_drops_trailing_chunks_everywhere() {
    let workspace_keys = keys("workspace-a");
    let items = words(0..600);
    let (a, b) = seed_transcript(&workspace_keys, &items).await;

    let trimmed = words(0..100);
    write_words(&a, &trimmed, NINE_AM).await;
    encrypt_e2ee_replica_changes(a.pool(), &workspace_keys)
        .await
        .unwrap();
    copy_replica(a.pool(), b.pool()).await;
    apply_e2ee_replica_changes(b.pool(), &workspace_keys)
        .await
        .unwrap();

    assert_eq!(read_words(&b).await, trimmed);
    assert_eq!(
        chunk_state_fields(&b).await,
        vec!["words_json#0", "words_json#n"]
    );
}

#[tokio::test]
async fn a_legacy_whole_column_record_still_applies() {
    let workspace_keys = keys("workspace-a");
    let key = &workspace_keys["workspace-a"];
    let db = test_db().await;
    sqlx::query(
        "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
         VALUES ('session-1', 'workspace-a', 'user-a', 'Meeting')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let items = words(0..5);
    for (field, value) in [
        (ROW_MANIFEST_FIELD, json!(true)),
        ("session_id", json!("session-1")),
        ("words_json", Value::String(words_json(&items))),
    ] {
        let sealed = key
            .seal_field(
                "workspace-a",
                "transcripts",
                "transcript-1",
                field,
                "ffffffffffffffffffffffffffffffff",
                1,
                false,
                value,
            )
            .unwrap();
        sqlx::query(
            "INSERT INTO e2ee_records (id, workspace_id, payload) VALUES (?, 'workspace-a', ?)",
        )
        .bind(&sealed.record_id)
        .bind(&sealed.payload)
        .execute(db.pool())
        .await
        .unwrap();
    }

    apply_e2ee_replica_changes(db.pool(), &workspace_keys)
        .await
        .unwrap();

    assert_eq!(read_words(&db).await, items);
}
