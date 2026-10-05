//! Randomised multi-device sync. Devices edit a shared note while apart,
//! publish through a model of the witness (newest revision per record wins),
//! and pull each other's records. After everyone syncs until quiet, every
//! device must hold the same note, the title must be the latest edit by edit
//! time, and no device may still have work to do.

use super::*;
use std::collections::BTreeMap;

const DEVICES: usize = 3;
const ROUNDS: usize = 8;
const SEEDS: u64 = 6;
const BASE_TIME_MS: i64 = 1_700_000_000_000;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*: enough randomness for a schedule, fully deterministic.
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }
}

struct Server {
    records: BTreeMap<String, (String, u64, String, String)>,
}

impl Server {
    fn new() -> Self {
        Self {
            records: BTreeMap::new(),
        }
    }

    async fn publish(&mut self, db: &anlg_db_core::Db, keyring: &anlg_e2ee::WorkspaceKeyring) {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, payload FROM e2ee_records WHERE workspace_id = 'workspace-a'",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        for (id, payload) in rows {
            if payload.is_empty() {
                continue;
            }
            let Ok(field) = keyring.open_field("workspace-a", &id, &payload) else {
                continue;
            };
            let hash = anlg_e2ee::payload_hash(&payload);
            let newer = match self.records.get(&id) {
                None => true,
                Some((_, revision, writer, current_hash)) => {
                    (field.revision, field.writer_id.as_str(), hash.as_str())
                        > (*revision, writer.as_str(), current_hash.as_str())
                }
            };
            if newer {
                self.records
                    .insert(id, (payload, field.revision, field.writer_id, hash));
            }
        }
    }

    async fn pull(&self, db: &anlg_db_core::Db) {
        for (id, (payload, ..)) in &self.records {
            sqlx::query(
                "INSERT INTO e2ee_records (id, workspace_id, payload) VALUES (?, 'workspace-a', ?)
                 ON CONFLICT(id) DO UPDATE SET payload = excluded.payload
                 WHERE e2ee_records.payload <> excluded.payload",
            )
            .bind(id)
            .bind(payload)
            .execute(db.pool())
            .await
            .unwrap();
        }
    }
}

fn paragraph(text: &str) -> Value {
    json!({ "type": "paragraph", "content": [{ "type": "text", "text": text }] })
}

fn body_doc(paragraphs: &[String]) -> String {
    json!({ "type": "doc", "content": paragraphs.iter().map(|text| paragraph(text)).collect::<Vec<_>>() })
        .to_string()
}

fn paragraphs(body: &str) -> Vec<String> {
    let doc: Value = serde_json::from_str(body).unwrap();
    doc["content"]
        .as_array()
        .unwrap()
        .iter()
        .map(|block| {
            block["content"][0]["text"]
                .as_str()
                .unwrap_or("")
                .to_string()
        })
        .collect()
}

async fn note(db: &anlg_db_core::Db) -> (String, String) {
    sqlx::query_as(
        "SELECT session.title, note.body
         FROM sessions AS session
         JOIN session_documents AS note ON note.id = session.id
         WHERE session.id = 'session-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap()
}

// The note and its session share the row id, so stamp only the table edited.
async fn stamp(db: &anlg_db_core::Db, table: &str, edited_at_ms: i64) {
    sqlx::query(
        "UPDATE e2ee_dirty_rows SET dirtied_at_ms = ?
         WHERE row_id = 'session-1' AND table_name = ?",
    )
    .bind(edited_at_ms)
    .bind(table)
    .execute(db.pool())
    .await
    .unwrap();
}

async fn has_work(db: &anlg_db_core::Db) -> bool {
    let dirty: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_dirty_rows")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM e2ee_replica_pending")
        .fetch_one(db.pool())
        .await
        .unwrap();
    dirty > 0 || pending > 0
}

async fn sync(
    db: &anlg_db_core::Db,
    server: &mut Server,
    workspace_keys: &HashMap<String, anlg_e2ee::WorkspaceKeyring>,
) {
    let keyring = &workspace_keys["workspace-a"];
    server.pull(db).await;
    apply_e2ee_replica_changes(db.pool(), workspace_keys)
        .await
        .unwrap();
    encrypt_e2ee_replica_changes(db.pool(), workspace_keys)
        .await
        .unwrap();
    server.publish(db, keyring).await;
}

#[tokio::test]
async fn devices_converge_and_the_latest_title_edit_wins() {
    for seed in 1..=SEEDS {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let workspace_keys = keys("workspace-a");
        let mut server = Server::new();
        let mut devices = Vec::with_capacity(DEVICES);
        for _ in 0..DEVICES {
            devices.push(test_db().await);
        }

        let initial = (0..4).map(|index| format!("p{index}")).collect::<Vec<_>>();
        sqlx::query(
            "INSERT INTO sessions (id, workspace_id, owner_user_id, title)
             VALUES ('session-1', 'workspace-a', 'user-a', 'Base')",
        )
        .execute(devices[0].pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_documents (id, workspace_id, session_id, kind, body_format, body)
             VALUES ('session-1', 'workspace-a', 'session-1', 'note', 'prosemirror_json', ?)",
        )
        .bind(body_doc(&initial))
        .execute(devices[0].pool())
        .await
        .unwrap();
        stamp(&devices[0], "sessions", BASE_TIME_MS).await;
        stamp(&devices[0], "session_documents", BASE_TIME_MS).await;
        sync(&devices[0], &mut server, &workspace_keys).await;
        for device in &devices[1..] {
            sync(device, &mut server, &workspace_keys).await;
        }

        let mut latest_title: (i64, String) = (BASE_TIME_MS, "Base".to_string());
        for round in 0..ROUNDS {
            for (index, device) in devices.iter().enumerate() {
                if !rng.chance(70) {
                    continue;
                }
                let edited_at_ms = BASE_TIME_MS + ((round * DEVICES + index) as i64 + 1) * 60_000;
                let edited_table = match rng.below(3) {
                    0 => {
                        let title = format!("title-d{index}-r{round}");
                        sqlx::query("UPDATE sessions SET title = ? WHERE id = 'session-1'")
                            .bind(&title)
                            .execute(device.pool())
                            .await
                            .unwrap();
                        if edited_at_ms > latest_title.0 {
                            latest_title = (edited_at_ms, title);
                        }
                        "sessions"
                    }
                    1 => {
                        let (_, body) = note(device).await;
                        let mut current = paragraphs(&body);
                        let slot = rng.below(current.len());
                        current[slot] = format!("p{slot}-d{index}-r{round}");
                        sqlx::query("UPDATE session_documents SET body = ? WHERE id = 'session-1'")
                            .bind(body_doc(&current))
                            .execute(device.pool())
                            .await
                            .unwrap();
                        "session_documents"
                    }
                    _ => {
                        let (_, body) = note(device).await;
                        let mut current = paragraphs(&body);
                        current.push(format!("added-d{index}-r{round}"));
                        sqlx::query("UPDATE session_documents SET body = ? WHERE id = 'session-1'")
                            .bind(body_doc(&current))
                            .execute(device.pool())
                            .await
                            .unwrap();
                        "session_documents"
                    }
                };
                stamp(device, edited_table, edited_at_ms).await;
                if rng.chance(50) {
                    sync(device, &mut server, &workspace_keys).await;
                }
            }
        }

        let mut quiet = false;
        for _ in 0..12 {
            for device in &devices {
                sync(device, &mut server, &workspace_keys).await;
            }
            let mut any_work = false;
            for device in &devices {
                any_work |= has_work(device).await;
            }
            if !any_work {
                quiet = true;
                break;
            }
        }
        assert!(quiet, "seed {seed}: devices never settled");

        let mut notes = Vec::new();
        for device in &devices {
            notes.push(note(device).await);
        }
        for (index, current) in notes.iter().enumerate() {
            assert_eq!(
                current, &notes[0],
                "seed {seed}: device {index} diverged from device 0"
            );
        }
        assert_eq!(
            notes[0].0, latest_title.1,
            "seed {seed}: the latest title edit did not win"
        );
        let final_paragraphs = paragraphs(&notes[0].1);
        assert!(
            final_paragraphs.len() >= initial.len(),
            "seed {seed}: paragraphs vanished"
        );
    }
}

#[tokio::test]
async fn concurrent_default_summaries_converge_without_losing_either_body() {
    let a = test_db().await;
    let b = test_db().await;
    let workspace_keys = keys("workspace-a");
    let desktop_body = r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Desktop decisions"}]}]}"#;
    let mobile_body = "## Mobile decisions";
    for (device, kind, template, format, body, edited_at) in [
        (
            &a,
            "template_output",
            "template-1",
            "prosemirror_json",
            desktop_body,
            1_800_000_000_000_i64,
        ),
        (
            &b,
            "summary",
            "",
            "markdown",
            mobile_body,
            1_800_000_002_000_i64,
        ),
    ] {
        sqlx::query("INSERT INTO sessions (id, workspace_id) VALUES ('meeting', 'workspace-a')")
            .execute(device.pool())
            .await
            .unwrap();
        sqlx::query("INSERT INTO session_documents (id, workspace_id, session_id, kind, template_id, body_format, body)
            VALUES ('summary:meeting', 'workspace-a', 'meeting', ?, ?, ?, ?)")
            .bind(kind).bind(template).bind(format).bind(body).execute(device.pool()).await.unwrap();
        sqlx::query("UPDATE e2ee_dirty_rows SET dirtied_at_ms = ?")
            .bind(edited_at)
            .execute(device.pool())
            .await
            .unwrap();
        encrypt_e2ee_replica_changes(device.pool(), &workspace_keys)
            .await
            .unwrap();
    }
    let mut server = Server::new();
    for device in [&a, &b] {
        server.publish(device, &workspace_keys["workspace-a"]).await;
    }
    for _ in 0..8 {
        for device in [&a, &b] {
            sync(device, &mut server, &workspace_keys).await;
        }
        if !has_work(&a).await && !has_work(&b).await {
            break;
        }
    }
    assert!(!has_work(&a).await && !has_work(&b).await);
    let mut converged = None;
    let mut retained_desktop = false;
    let mut retained_mobile = false;
    for device in [&a, &b] {
        let documents: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT id, body, body_format FROM session_documents WHERE deleted_at IS NULL",
        )
        .fetch_all(device.pool())
        .await
        .unwrap();
        assert_eq!(documents.len(), 1);
        assert_eq!(documents[0].0, "summary:meeting");
        let body = &documents[0].1;
        assert!(body == desktop_body || body == mobile_body);
        assert_eq!(
            documents[0].2,
            if body == desktop_body {
                "prosemirror_json"
            } else {
                "markdown"
            }
        );
        if let Some(previous) = &converged {
            assert_eq!(&documents, previous);
        }
        converged = Some(documents);
        let versions: Vec<String> = sqlx::query_scalar("SELECT body FROM session_documents
            UNION ALL SELECT body FROM session_document_versions WHERE document_id = 'summary:meeting'
            UNION ALL SELECT json_extract(value_json, '$') FROM e2ee_field_conflicts
              WHERE table_name = 'session_documents' AND row_id = 'summary:meeting' AND field_name = 'body'")
            .fetch_all(device.pool()).await.unwrap();
        retained_desktop |= versions.iter().any(|body| body == desktop_body);
        retained_mobile |= versions.iter().any(|body| body == mobile_body);
    }
    assert!(retained_desktop && retained_mobile);
}
