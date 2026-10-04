use sqlx::{Row, Sqlite, Transaction};

pub(crate) const CAPTURE_USAGE_SESSION_PREDICATE: &str = "CASE WHEN json_valid(value_json) THEN
       json_extract(value_json,'$.sessionId')=?1
       AND id='capture_usage:'||?1||':'||json_extract(value_json,'$.transcriptId')
     ELSE 0 END";

pub(crate) fn capture_stop_session_predicate() -> String {
    format!(
        "id IN (SELECT 'capture_stop:'||?1||':'||id FROM transcripts WHERE session_id=?1
          UNION SELECT 'capture_stop:'||?1||':'||json_extract(value_json,'$.transcriptId')
          FROM app_settings WHERE ({CAPTURE_USAGE_SESSION_PREDICATE}))"
    )
}

/// Local preservation also includes IDs retained by child/history rows after a
/// parent disappears. Keyset pagination never requires a live session row.
pub async fn list_local_hold_sessions(
    tx: &mut Transaction<'_, Sqlite>,
    after: &str,
    limit: u32,
) -> Result<Vec<String>, sqlx::Error> {
    let tables = local_hold_tables(tx).await?;
    let mut sources = vec!["SELECT id FROM sessions".to_string()];
    for table in &tables {
        let name = quote_hold_identifier(&table.name);
        for column in ["session_id", "source_session_id"] {
            if table.columns.iter().any(|value| value == column) {
                let column = quote_hold_identifier(column);
                sources.push(format!("SELECT {column} AS id FROM {name}"));
            }
        }
    }
    sources.push("SELECT row_id AS id FROM e2ee_local_state WHERE table_name='sessions'".into());
    sources.push("SELECT substr(id,length('capture_lifecycle_pending:')+1) AS id FROM app_settings WHERE id GLOB 'capture_lifecycle_pending:*'".into());
    sources.push("SELECT substr(id,length('capture_audio_saved:')+1) AS id FROM app_settings WHERE id GLOB 'capture_audio_saved:*'".into());
    sources.push("SELECT json_extract(value_json,'$.sessionId') AS id FROM app_settings WHERE CASE WHEN json_valid(value_json) THEN id='capture_usage:'||json_extract(value_json,'$.sessionId')||':'||json_extract(value_json,'$.transcriptId') ELSE 0 END".into());
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT DISTINCT id FROM ({}) WHERE id<>'' AND id>? ORDER BY id COLLATE BINARY LIMIT ?",
        sources.join(" UNION ")
    )))
    .bind(after)
    .bind(limit.clamp(1, 200))
    .fetch_all(&mut **tx)
    .await
}

pub struct LocalHoldTable {
    pub name: String,
    pub columns: Vec<String>,
    pub predicate: String,
    pub key: String,
}

pub fn quote_hold_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

/// Enumerate meeting-owned data, including revisions and live partials. No
/// global settings, account sessions, provider tokens or unrelated meetings.
/// Names come from SQLite; identifiers are quoted, and values remain bound.
pub async fn local_hold_tables(
    tx: &mut Transaction<'_, Sqlite>,
) -> Result<Vec<LocalHoldTable>, sqlx::Error> {
    let definitions = sqlx::query(
        "SELECT name,sql FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    ).fetch_all(&mut **tx).await?;
    let mut tables = Vec::new();
    let mut replicas = Vec::new();
    for definition in definitions {
        let name: String = definition.get("name");
        let ddl: String = definition.get("sql");
        let quoted = quote_hold_identifier(&name);
        let info = sqlx::query(sqlx::AssertSqlSafe(format!("PRAGMA table_info({quoted})")))
            .fetch_all(&mut **tx)
            .await?;
        let columns: Vec<String> = info.iter().map(|row| row.get("name")).collect();
        let key = if ddl.to_ascii_uppercase().contains("WITHOUT ROWID") {
            let mut primary: Vec<_> = info
                .iter()
                .filter_map(|row| {
                    let order: i64 = row.get("pk");
                    (order > 0)
                        .then(|| (order, quote_hold_identifier(&row.get::<String, _>("name"))))
                })
                .collect();
            primary.sort_by_key(|(order, _)| *order);
            format!(
                "json_array({})",
                primary
                    .into_iter()
                    .map(|(_, name)| name)
                    .collect::<Vec<_>>()
                    .join(",")
            )
        } else {
            "CAST(_rowid_ AS TEXT)".into()
        };
        let has = |column: &str| columns.iter().any(|value| value == column);
        let mut related = Vec::new();
        if has("session_id") {
            related.push("session_id=?1");
        }
        if has("source_session_id") {
            related.push("source_session_id=?1");
        }
        if has("transcript_id") {
            related.push("transcript_id IN (SELECT id FROM transcripts WHERE session_id=?1)");
        }
        if has("document_id") {
            related.push("document_id IN (SELECT id FROM session_documents WHERE session_id=?1)");
        }
        let predicate = if name == "sessions" {
            "id=?1".into()
        } else if name == "app_settings" {
            format!(
                "({CAPTURE_USAGE_SESSION_PREDICATE}) OR ({})
                 OR id IN ('capture_lifecycle_pending:'||?1,'capture_audio_saved:'||?1,
                           'auto_enhance_pending:'||?1,'capture_incomplete:'||?1||':audio-recovery',
                           'capture_incomplete:'||?1||':audio-cleanup')
                 OR id IN (SELECT 'capture_incomplete:'||?1||':'||id FROM transcripts WHERE session_id=?1)",
                capture_stop_session_predicate()
            )
        } else if !related.is_empty() {
            related.join(" OR ")
        } else if name == "events" {
            "id IN (SELECT event_id FROM sessions WHERE id=?1)".into()
        } else if name == "entity_mentions" {
            "((source_type='session' AND source_id=?1) OR (target_type='session' AND target_id=?1))"
                .into()
        } else if name.starts_with("e2ee_")
            && (has("record_id") || has("table_name") || name == "e2ee_records")
        {
            replicas.push(LocalHoldTable {
                name,
                columns,
                predicate: String::new(),
                key,
            });
            continue;
        } else {
            continue;
        };
        tables.push(LocalHoldTable {
            name,
            columns,
            predicate,
            key,
        });
    }
    // Retained encrypted revisions are linked through their domain identity.
    // Keep only this meeting's replicas; no workspace-wide encryption keys.
    let domain = tables
        .iter()
        .filter(|table| table.columns.iter().any(|column| column == "id"))
        .map(|table| {
            format!(
                "(table_name='{}' AND row_id IN (SELECT id FROM {} WHERE ({})))",
                table.name.replace(char::from(39), "''"),
                quote_hold_identifier(&table.name),
                table.predicate
            )
        })
        .collect::<Vec<_>>()
        .join(" OR ");
    for mut table in replicas {
        let has = |column: &str| table.columns.iter().any(|value| value == column);
        let replica_domain = format!("({domain}) OR (table_name='sessions' AND row_id=?1)");
        table.predicate = if has("table_name") && has("row_id") {
            replica_domain
        } else if has("workspace_id") && (has("record_id") || table.name == "e2ee_records") {
            let record = if table.name == "e2ee_records" {
                "id"
            } else {
                "record_id"
            };
            format!(
                "(workspace_id,{record}) IN (SELECT workspace_id,record_id FROM e2ee_local_state WHERE ({replica_domain}))"
            )
        } else {
            continue;
        };
        tables.push(table);
    }
    tables.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(tables)
}
