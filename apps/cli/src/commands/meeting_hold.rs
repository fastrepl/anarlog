use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::{Component, Path},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;

use crate::{Error, Result, output};

fn failure(error: impl std::fmt::Display) -> Error {
    Error::operation("preserve local meeting", error.to_string())
}

/// The recorder can retain files after every corresponding SQLite row is gone.
/// Read only its sessions tree; never follow links or silently truncate inventory.
fn recording_locations(db: &anlg_db_core::Db) -> Result<BTreeMap<String, Vec<String>>> {
    let options = db.pool().connect_options();
    let base = options
        .get_filename()
        .parent()
        .ok_or_else(|| failure("storage unavailable"))?;
    let base = if base.as_os_str().is_empty() {
        Path::new(".")
    } else {
        base
    };
    let root = base.canonicalize().map_err(failure)?.join("sessions");
    match std::fs::symlink_metadata(&root) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(failure(error)),
        Ok(metadata) if metadata.is_dir() && !metadata.is_symlink() => {}
        _ => return Err(failure("linked or invalid recorder sessions directory")),
    }
    let started = std::time::Instant::now();
    let mut stack = vec![(root.clone(), 0)];
    let mut locations: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut directories = 0;
    let mut entries = 0;
    while let Some((directory, depth)) = stack.pop() {
        directories += 1;
        if directories > 10_000
            || depth > 32
            || started.elapsed() >= std::time::Duration::from_secs(2)
        {
            return Err(failure(
                "recording inventory exceeds traversal bounds; preservation awaits retry",
            ));
        }
        let mut has_recording = false;
        for entry in std::fs::read_dir(&directory).map_err(failure)? {
            entries += 1;
            if entries > 100_000 || started.elapsed() >= std::time::Duration::from_secs(2) {
                return Err(failure(
                    "recording inventory exceeds traversal bounds; preservation awaits retry",
                ));
            }
            let entry = entry.map_err(failure)?;
            let kind = entry.file_type().map_err(failure)?;
            if kind.is_symlink() {
                return Err(failure("linked recorder path; preservation awaits review"));
            }
            let name = entry.file_name();
            let name = name
                .to_str()
                .ok_or_else(|| failure("unrepresentable recorder filename"))?;
            if kind.is_dir() {
                if matches!(name, "audio-recovery" | "attachments") {
                    has_recording = true;
                } else {
                    stack.push((entry.path(), depth + 1));
                }
            } else if kind.is_file() {
                let extension = Path::new(name).extension().and_then(|value| value.to_str());
                has_recording |= matches!(
                    name,
                    "_meta.json"
                        | "_memo.md"
                        | "transcript.json"
                        | "audio.mp3.tmp"
                        | "audio.wav.tmp"
                ) || extension == Some("md")
                    || ((name.starts_with("audio.") || name.starts_with("audio_"))
                        && matches!(
                            extension,
                            Some("wav" | "mp3" | "ogg" | "opus" | "m4a" | "flac")
                        ));
            }
        }
        if has_recording {
            let id = directory
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| failure("invalid recording identity"))?;
            let path = directory.strip_prefix(&root).map_err(failure)?;
            if path.as_os_str().is_empty() || id.len() > 500 {
                return Err(failure("recording files have no valid meeting directory"));
            }
            let mut parts = Vec::new();
            for part in path.components() {
                let Component::Normal(value) = part else {
                    return Err(failure("invalid recording path"));
                };
                parts.push(
                    value
                        .to_str()
                        .ok_or_else(|| failure("unrepresentable recording path"))?,
                );
            }
            let path = parts.join("/");
            if path.len() > 4096 {
                return Err(failure("recording path exceeds preservation bounds"));
            }
            locations.entry(id.into()).or_default().push(path);
        }
    }
    for paths in locations.values_mut() {
        paths.sort();
        if paths.len() > 128 || paths.iter().map(String::len).sum::<usize>() > 32 * 1024 {
            return Err(failure("recording locations exceed preservation bounds"));
        }
    }
    Ok(locations)
}

pub async fn list(
    db: &anlg_db_core::Db,
    after: &str,
    limit: u32,
    database_only: bool,
) -> Result<()> {
    let locations = if database_only {
        BTreeMap::new()
    } else {
        recording_locations(db)?
    };
    let mut tx = db.pool().begin().await.map_err(failure)?;
    let database_ids = anlg_db_app::list_local_hold_sessions(&mut tx, after, limit)
        .await
        .map_err(failure)?;
    tx.rollback().await.map_err(failure)?;
    let ids: Vec<_> = database_ids
        .into_iter()
        .chain(locations.into_keys().filter(|id| id.as_str() > after))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(limit.clamp(1, 200) as usize)
        .collect();
    output::emit(&output::json(
        "meetings.hold-list",
        &json!({"ids":ids}),
        None,
    )?);
    Ok(())
}

/// NDJSON preserves raw SQLite cells in bounded chunks in one read transaction.
/// Opaque JSON/text is never normalized; even a single large note stays bounded.
pub async fn export(db: &anlg_db_core::Db, id: &str, database_only: bool) -> Result<()> {
    let locations = if database_only {
        Vec::new()
    } else {
        recording_locations(db)?.remove(id).unwrap_or_default()
    };
    let mut tx = db.pool().begin().await.map_err(failure)?;
    let tables = anlg_db_app::local_hold_tables(&mut tx)
        .await
        .map_err(failure)?;
    let mut exists = false;
    for table in &tables {
        let name = anlg_db_app::quote_hold_identifier(&table.name);
        if sqlx::query_scalar::<_, bool>(sqlx::AssertSqlSafe(format!(
            "SELECT EXISTS(SELECT 1 FROM {name} WHERE ({}))",
            table.predicate
        )))
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(failure)?
        {
            exists = true;
            break;
        }
    }
    if !exists && locations.is_empty() {
        return Err(Error::NotFound("local held meeting".into()));
    }
    let mut stream = Stream {
        output: std::io::BufWriter::new(std::io::stdout()),
        hash: Sha256::new(),
        frames: 0,
    };
    stream.frame(json!({"type":"header","schema_version":"1","command":"meetings.hold-export","meeting_id":id,"recording_locations":locations,"recording_files_separate":database_only}))?;
    for table in tables {
        let quoted = anlg_db_app::quote_hold_identifier(&table.name);
        let key = &table.key;
        let mut after: Option<String> = None;
        loop {
            let rowid: Option<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT {key} FROM {quoted} WHERE ({}) AND (?2 IS NULL OR {key}>?2 COLLATE BINARY) ORDER BY {key} COLLATE BINARY LIMIT 1", table.predicate
            ))).bind(id).bind(after).fetch_optional(&mut *tx).await.map_err(failure)?;
            let Some(rowid) = rowid else { break };
            for column in &table.columns {
                let name = anlg_db_app::quote_hold_identifier(column);
                // SQLite's extended formatter round-trips REAL values; ordinary
                // CAST can lose precision. Other storage classes remain exact.
                let bytes = format!(
                    "CASE WHEN typeof({name})='real' THEN CAST(printf('%!.26g',{name}) AS BLOB) ELSE CAST({name} AS BLOB) END"
                );
                let info = sqlx::query(sqlx::AssertSqlSafe(format!("SELECT typeof({name}) AS storage, COALESCE(length({bytes}),0) AS total FROM {quoted} WHERE {key}=?")))
                    .bind(&rowid).fetch_one(&mut *tx).await.map_err(failure)?;
                let storage: String = info.get("storage");
                let total: i64 = info.get("total");
                let mut offset = 0;
                loop {
                    let data: Option<Vec<u8>> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                        "SELECT substr({bytes},?,65536) FROM {quoted} WHERE {key}=?"
                    )))
                    .bind(offset + 1)
                    .bind(&rowid)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(failure)?;
                    let data = data.unwrap_or_default();
                    stream.frame(json!({"type":"cell","table":table.name,"row":rowid,"column":column,"storage":storage,"offset":offset,"total":total,"base64":STANDARD.encode(&data)}))?;
                    offset += data.len() as i64;
                    if offset >= total {
                        break;
                    }
                    if data.is_empty() {
                        return Err(failure("incomplete SQLite cell"));
                    }
                }
            }
            stream.frame(json!({"type":"row-end","table":table.name,"row":rowid}))?;
            after = Some(rowid);
        }
    }
    tx.rollback().await.map_err(failure)?;
    let digest: String = stream
        .hash
        .clone()
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let frames = stream.frames;
    stream.frame(json!({"type":"end","sha256":digest,"frames":frames}))?;
    stream.output.flush().map_err(failure)
}

struct Stream<W: Write> {
    output: W,
    hash: Sha256,
    frames: u64,
}
impl<W: Write> Stream<W> {
    fn frame(&mut self, mut frame: Value) -> Result<()> {
        frame["sequence"] = self.frames.into();
        let mut bytes = serde_json::to_vec(&frame).map_err(failure)?;
        bytes.push(b'\n');
        self.output.write_all(&bytes).map_err(failure)?;
        self.hash.update(&bytes);
        self.frames += 1;
        Ok(())
    }
}
