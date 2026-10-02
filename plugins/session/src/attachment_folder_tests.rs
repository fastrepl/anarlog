use anlg_db_core::Db;
use sqlx::{Row, SqlitePool};

use crate::attachments::{
    CatalogNoteAttachmentRequest, CatalogSessionAudioRequest, SessionAudioRequest,
    SetAttachmentCloudSyncEnabledRequest, catalog_note_attachment, catalog_session_audio,
    mark_session_audio_absent, mark_session_audio_transcription_complete,
    set_attachment_cloud_sync_enabled, tombstone_session_audio,
};
use crate::folder_catalog::{
    DeleteFolderCatalogRequest, EnsureFolderCatalogRequest, RenameFolderCatalogRequest,
    UpdateFolderIconRequest, UpdateFolderInstructionsRequest, UpdateFolderWorkspaceRequest,
    delete_folder_catalog, ensure_folder_catalog, rename_folder_catalog, update_folder_icon,
    update_folder_instructions, update_folder_workspace,
};
use crate::folder_materials::{
    CatalogFolderMaterialRequest, TombstoneFolderMaterialRequest, catalog_folder_material,
    tombstone_folder_material,
};

const SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

async fn test_db() -> Db {
    let db = Db::connect_memory_plain().await.unwrap();
    anlg_db_app::prepare_schema(&db).await.unwrap();
    db
}

async fn insert_session(pool: &SqlitePool, id: &str) {
    sqlx::query(
        "INSERT INTO sessions (id, owner_user_id, workspace_id) VALUES (?, 'user-1', 'workspace-1')",
    )
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
}

async fn insert_session_in_folder(pool: &SqlitePool, id: &str, workspace: &str, folder: &str) {
    sqlx::query(
        "INSERT INTO sessions (id, owner_user_id, workspace_id, folder_path)
         VALUES (?, 'user-1', ?, ?)",
    )
    .bind(id)
    .bind(workspace)
    .bind(folder)
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_note_attachment_row(
    pool: &SqlitePool,
    session_id: &str,
    relative_path: &str,
    sha256: &str,
    cloud_object_key: &str,
) {
    sqlx::query(
        "INSERT INTO session_attachments (
          id, workspace_id, session_id, filename, relative_path, content_type,
          size_bytes, sha256, cloud_object_key
        ) VALUES (?, 'workspace-1', ?, 'old.png', ?, 'image/png', 10, ?, ?)",
    )
    .bind(format!("att-{relative_path}"))
    .bind(session_id)
    .bind(relative_path)
    .bind(sha256)
    .bind(cloud_object_key)
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_transfer_job(
    pool: &SqlitePool,
    id: &str,
    attachment_id: &str,
    direction: &str,
    phase: &str,
) {
    sqlx::query(
        "INSERT INTO attachment_transfer_jobs (
          id, attachment_id, session_id, workspace_id, direction,
          expected_sha256, expected_size_bytes, object_key, phase
        ) VALUES (?, ?, 'session-1', 'workspace-1', ?, ?, 10, 'object-key-1', ?)",
    )
    .bind(id)
    .bind(attachment_id)
    .bind(direction)
    .bind(if direction == "download" {
        SHA_B
    } else {
        SHA_A
    })
    .bind(phase)
    .execute(pool)
    .await
    .unwrap();
}

fn note_request() -> CatalogNoteAttachmentRequest {
    CatalogNoteAttachmentRequest {
        session_id: "session-1".to_string(),
        attachment_id: "att-1".to_string(),
        filename: "doc.pdf".to_string(),
        content_type: "application/pdf".to_string(),
        size_bytes: 42,
        sha256: SHA_A.to_string(),
    }
}

async fn job_rows(pool: &SqlitePool) -> Vec<sqlx::sqlite::SqliteRow> {
    sqlx::query(
        "SELECT id, attachment_id, direction, phase, object_key FROM attachment_transfer_jobs ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn catalog_note_attachment_inserts_row_and_local_state() {
    let db = test_db().await;
    insert_session(db.pool(), "session-1").await;

    catalog_note_attachment(db.pool(), note_request())
        .await
        .unwrap();

    let row = sqlx::query(
        "SELECT filename, relative_path, sha256, source_type, source_id, workspace_id
         FROM session_attachments WHERE relative_path = 'attachments/att-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>("filename"), "doc.pdf");
    assert_eq!(row.get::<String, _>("source_type"), "note_upload");
    assert_eq!(row.get::<String, _>("source_id"), "att-1");

    let local: String = sqlx::query_scalar(
        "SELECT availability FROM attachment_local_state
         WHERE relative_path = 'attachments/att-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(local, "present");

    assert!(
        job_rows(db.pool()).await.is_empty(),
        "no upload job without cloud_sync_enabled"
    );
}

#[tokio::test]
async fn catalog_note_attachment_enqueues_upload_when_sync_enabled() {
    let db = test_db().await;
    insert_session(db.pool(), "session-1").await;
    sqlx::query(
        "INSERT INTO session_attachments (
          id, workspace_id, session_id, filename, relative_path, content_type,
          size_bytes, sha256, cloud_sync_enabled
        ) VALUES ('att-old', 'workspace-1', 'session-1', 'old.pdf', 'attachments/att-1',
                  'application/pdf', 10, ?, 1)",
    )
    .bind(SHA_A)
    .execute(db.pool())
    .await
    .unwrap();

    catalog_note_attachment(db.pool(), note_request())
        .await
        .unwrap();

    let jobs = job_rows(db.pool()).await;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].get::<String, _>("direction"), "upload");
}

#[tokio::test]
async fn catalog_note_attachment_recatalog_new_sha_enqueues_delete_and_clears_key() {
    let db = test_db().await;
    insert_session(db.pool(), "session-1").await;
    insert_note_attachment_row(
        db.pool(),
        "session-1",
        "attachments/att-1",
        SHA_B,
        "object-key-1",
    )
    .await;

    catalog_note_attachment(db.pool(), note_request())
        .await
        .unwrap();

    let jobs = job_rows(db.pool()).await;
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].get::<String, _>("direction"), "delete");
    assert_eq!(jobs[0].get::<String, _>("object_key"), "object-key-1");

    let row = sqlx::query(
        "SELECT cloud_object_key, storage_kind, sha256, deleted_at
         FROM session_attachments WHERE relative_path = 'attachments/att-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>("cloud_object_key"), "");
    assert_eq!(row.get::<String, _>("storage_kind"), "local_file");
    assert_eq!(row.get::<String, _>("sha256"), SHA_A);
    assert!(row.get::<Option<String>, _>("deleted_at").is_none());
}

#[tokio::test]
async fn catalog_note_attachment_deleted_session_errors_and_writes_nothing() {
    let db = test_db().await;
    insert_session(db.pool(), "session-1").await;
    sqlx::query("UPDATE sessions SET deleted_at = 'gone' WHERE id = 'session-1'")
        .execute(db.pool())
        .await
        .unwrap();

    let error = catalog_note_attachment(db.pool(), note_request())
        .await
        .unwrap_err();
    assert_eq!(error, "transaction statement 3 affected 0 rows; expected 1");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM session_attachments")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert!(job_rows(db.pool()).await.is_empty());
}

#[tokio::test]
async fn catalog_session_audio_inserts_processing_transcript() {
    let db = test_db().await;
    insert_session(db.pool(), "session-1").await;

    catalog_session_audio(
        db.pool(),
        CatalogSessionAudioRequest {
            session_id: "session-1".to_string(),
            filename: "audio.wav".to_string(),
            content_type: "audio/wav".to_string(),
            size_bytes: 100,
            sha256: SHA_A.to_string(),
        },
    )
    .await
    .unwrap();

    let row = sqlx::query(
        "SELECT source_type, source_id, metadata_json
         FROM session_attachments WHERE id = 'session-audio:session-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>("source_type"), "session_audio");
    assert_eq!(
        row.get::<String, _>("metadata_json"),
        "{\"transcript_status\":\"processing\"}"
    );

    let local: String = sqlx::query_scalar(
        "SELECT availability FROM attachment_local_state
         WHERE attachment_id = 'session-audio:session-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(local, "present");
}

#[tokio::test]
async fn catalog_session_audio_missing_session_errors() {
    let db = test_db().await;
    let error = catalog_session_audio(
        db.pool(),
        CatalogSessionAudioRequest {
            session_id: "session-1".to_string(),
            filename: "audio.wav".to_string(),
            content_type: "audio/wav".to_string(),
            size_bytes: 100,
            sha256: SHA_A.to_string(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(error, "transaction statement 3 affected 0 rows; expected 1");
    assert!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM session_attachments")
            .fetch_one(db.pool())
            .await
            .unwrap()
            == 0
    );
}

#[tokio::test]
async fn mark_session_audio_transcription_complete_sets_status() {
    let db = test_db().await;
    insert_session(db.pool(), "session-1").await;
    catalog_session_audio(
        db.pool(),
        CatalogSessionAudioRequest {
            session_id: "session-1".to_string(),
            filename: "audio.wav".to_string(),
            content_type: "audio/wav".to_string(),
            size_bytes: 100,
            sha256: SHA_A.to_string(),
        },
    )
    .await
    .unwrap();

    mark_session_audio_transcription_complete(
        db.pool(),
        SessionAudioRequest {
            session_id: "session-1".to_string(),
        },
    )
    .await
    .unwrap();

    let metadata: String = sqlx::query_scalar(
        "SELECT metadata_json FROM session_attachments
         WHERE id = 'session-audio:session-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(metadata, "{\"transcript_status\":\"complete\"}");
}

async fn seed_synced_attachment(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO session_attachments (
          id, workspace_id, session_id, filename, relative_path, sha256,
          size_bytes, cloud_object_key
        ) VALUES ('att-1', 'workspace-1', 'session-1', 'doc.pdf', 'attachments/att-1', ?, 10, 'object-key-1')",
    )
    .bind(SHA_A)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn set_attachment_cloud_sync_enabled_enqueues_download_and_completes_delete() {
    let db = test_db().await;
    insert_session(db.pool(), "session-1").await;
    seed_synced_attachment(db.pool()).await;
    insert_transfer_job(db.pool(), "job-del", "att-1", "delete", "queued").await;

    set_attachment_cloud_sync_enabled(
        db.pool(),
        SetAttachmentCloudSyncEnabledRequest {
            session_id: "session-1".to_string(),
            attachment_id: "att-1".to_string(),
            enabled: true,
        },
    )
    .await
    .unwrap();

    let flag: i64 =
        sqlx::query_scalar("SELECT cloud_sync_enabled FROM session_attachments WHERE id = 'att-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(flag, 1);

    let jobs = job_rows(db.pool()).await;
    let old = jobs
        .iter()
        .find(|row| row.get::<String, _>("id") == "job-del")
        .unwrap();
    assert_eq!(old.get::<String, _>("phase"), "completed");
    let new_jobs: Vec<String> = jobs
        .iter()
        .filter(|row| row.get::<String, _>("id") != "job-del")
        .map(|row| row.get::<String, _>("direction"))
        .collect();
    assert_eq!(new_jobs, vec!["download".to_string()]);
}

#[tokio::test]
async fn set_attachment_cloud_sync_disabled_enqueues_download_delete_and_completes_jobs() {
    let db = test_db().await;
    insert_session(db.pool(), "session-1").await;
    seed_synced_attachment(db.pool()).await;
    sqlx::query("UPDATE session_attachments SET cloud_sync_enabled = 1 WHERE id = 'att-1'")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO attachment_local_state (
          attachment_id, session_id, relative_path, availability
        ) VALUES ('att-1', 'session-1', 'attachments/att-1', 'present')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    insert_transfer_job(db.pool(), "job-up", "att-1", "upload", "queued").await;
    insert_transfer_job(db.pool(), "job-down", "att-1", "download", "retry_wait").await;

    set_attachment_cloud_sync_enabled(
        db.pool(),
        SetAttachmentCloudSyncEnabledRequest {
            session_id: "session-1".to_string(),
            attachment_id: "att-1".to_string(),
            enabled: false,
        },
    )
    .await
    .unwrap();

    let flag: i64 =
        sqlx::query_scalar("SELECT cloud_sync_enabled FROM session_attachments WHERE id = 'att-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(flag, 0);

    let jobs = job_rows(db.pool()).await;
    let phases: Vec<(String, String, String)> = jobs
        .iter()
        .map(|row| {
            (
                row.get::<String, _>("id"),
                row.get::<String, _>("direction"),
                row.get::<String, _>("phase"),
            )
        })
        .collect();
    assert!(
        phases
            .iter()
            .any(|(id, _, phase)| id == "job-up" && phase == "completed")
    );
    assert!(
        phases
            .iter()
            .any(|(id, _, phase)| id == "job-down" && phase == "completed")
    );
    let fresh: Vec<String> = jobs
        .iter()
        .filter(|row| !["job-up", "job-down"].contains(&row.get::<String, _>("id").as_str()))
        .map(|row| row.get::<String, _>("direction"))
        .collect();
    assert!(
        fresh.contains(&"delete".to_string()),
        "local present attachment enqueues delete job"
    );
}

#[tokio::test]
async fn set_attachment_cloud_sync_missing_attachment_commits_then_errors() {
    let db = test_db().await;
    insert_session(db.pool(), "session-1").await;
    seed_synced_attachment(db.pool()).await;
    sqlx::query("UPDATE session_attachments SET deleted_at = 'gone' WHERE id = 'att-1'")
        .execute(db.pool())
        .await
        .unwrap();
    insert_transfer_job(db.pool(), "job-del", "att-1", "delete", "queued").await;

    let error = set_attachment_cloud_sync_enabled(
        db.pool(),
        SetAttachmentCloudSyncEnabledRequest {
            session_id: "session-1".to_string(),
            attachment_id: "att-1".to_string(),
            enabled: true,
        },
    )
    .await
    .unwrap_err();
    assert_eq!(error, "attachment is unavailable");

    let phase: String =
        sqlx::query_scalar("SELECT phase FROM attachment_transfer_jobs WHERE id = 'job-del'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(phase, "completed", "committed before the error is raised");
}

#[tokio::test]
async fn tombstone_session_audio_and_mark_absent() {
    let db = test_db().await;
    insert_session(db.pool(), "session-1").await;
    catalog_session_audio(
        db.pool(),
        CatalogSessionAudioRequest {
            session_id: "session-1".to_string(),
            filename: "audio.wav".to_string(),
            content_type: "audio/wav".to_string(),
            size_bytes: 100,
            sha256: SHA_A.to_string(),
        },
    )
    .await
    .unwrap();

    tombstone_session_audio(
        db.pool(),
        SessionAudioRequest {
            session_id: "session-1".to_string(),
        },
    )
    .await
    .unwrap();
    let deleted: Option<String> = sqlx::query_scalar(
        "SELECT deleted_at FROM session_attachments
         WHERE id = 'session-audio:session-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(deleted.is_some());

    mark_session_audio_absent(
        db.pool(),
        SessionAudioRequest {
            session_id: "session-1".to_string(),
        },
    )
    .await
    .unwrap();
    let availability: String = sqlx::query_scalar(
        "SELECT availability FROM attachment_local_state
         WHERE attachment_id = 'session-audio:session-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(availability, "absent");
}

fn material_request() -> CatalogFolderMaterialRequest {
    CatalogFolderMaterialRequest {
        folder_path: "docs".to_string(),
        attachment_id: "mat-1".to_string(),
        filename: "syllabus.pdf".to_string(),
        content_type: "application/pdf".to_string(),
        size_bytes: 99,
        sha256: SHA_A.to_string(),
    }
}

#[tokio::test]
async fn catalog_folder_material_inserts_and_revives() {
    let db = test_db().await;

    catalog_folder_material(db.pool(), material_request())
        .await
        .unwrap();
    let row = sqlx::query(
        "SELECT relative_path, source_type, source_id FROM folder_attachments
         WHERE folder_path = 'docs'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>("relative_path"), "materials/mat-1");
    assert_eq!(row.get::<String, _>("source_type"), "folder_material");

    sqlx::query("UPDATE folder_attachments SET deleted_at = 'gone'")
        .execute(db.pool())
        .await
        .unwrap();
    catalog_folder_material(
        db.pool(),
        CatalogFolderMaterialRequest {
            filename: "v2.pdf".to_string(),
            ..material_request()
        },
    )
    .await
    .unwrap();
    let row = sqlx::query(
        "SELECT filename, deleted_at FROM folder_attachments WHERE folder_path = 'docs'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>("filename"), "v2.pdf");
    assert!(row.get::<Option<String>, _>("deleted_at").is_none());
}

#[tokio::test]
async fn tombstone_folder_material_missing_errors() {
    let db = test_db().await;
    let error = tombstone_folder_material(
        db.pool(),
        TombstoneFolderMaterialRequest {
            folder_path: "docs".to_string(),
            attachment_id: "mat-1".to_string(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(error, "folder material is unavailable");
}

#[tokio::test]
async fn ensure_folder_catalog_revives_and_uses_session_workspace() {
    let db = test_db().await;
    insert_session_in_folder(db.pool(), "session-1", "workspace-7", "docs").await;
    sqlx::query("INSERT INTO folders (id, path, deleted_at) VALUES ('f-1', 'docs', 'gone')")
        .execute(db.pool())
        .await
        .unwrap();

    ensure_folder_catalog(
        db.pool(),
        EnsureFolderCatalogRequest {
            paths: vec!["docs".to_string()],
        },
    )
    .await
    .unwrap();

    let row = sqlx::query("SELECT id, workspace_id, deleted_at FROM folders WHERE path = 'docs'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("id"), "f-1");
    assert!(row.get::<Option<String>, _>("deleted_at").is_none());

    sqlx::query("DELETE FROM folders")
        .execute(db.pool())
        .await
        .unwrap();
    ensure_folder_catalog(
        db.pool(),
        EnsureFolderCatalogRequest {
            paths: vec!["docs".to_string()],
        },
    )
    .await
    .unwrap();
    let workspace: String =
        sqlx::query_scalar("SELECT workspace_id FROM folders WHERE path = 'docs'")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(workspace, "workspace-7");
}

#[tokio::test]
async fn rename_folder_catalog_rewrites_exact_and_nested_paths() {
    let db = test_db().await;
    sqlx::query("INSERT INTO folders (id, path) VALUES ('f-1', 'a')")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO folder_attachments (id, folder_path, relative_path)
         VALUES ('fa-1', 'a/sub', 'materials/x'), ('fa-2', 'a\\win', 'materials/y')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, owner_user_id, folder_path)
         VALUES ('s-1', 'u', 'a'), ('s-2', 'u', 'a/deep'), ('s-3', 'u', 'a\\win/deep')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    rename_folder_catalog(
        db.pool(),
        RenameFolderCatalogRequest {
            old_path: "a".to_string(),
            new_path: "b".to_string(),
        },
    )
    .await
    .unwrap();

    let path: String = sqlx::query_scalar("SELECT path FROM folders WHERE id = 'f-1'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(path, "b");

    let mut material_paths: Vec<String> =
        sqlx::query_scalar("SELECT folder_path FROM folder_attachments ORDER BY id")
            .fetch_all(db.pool())
            .await
            .unwrap();
    material_paths.sort();
    assert_eq!(
        material_paths,
        vec!["b/sub".to_string(), "b\\win".to_string()]
    );

    let mut session_paths: Vec<String> =
        sqlx::query_scalar("SELECT folder_path FROM sessions ORDER BY id")
            .fetch_all(db.pool())
            .await
            .unwrap();
    session_paths.sort();
    assert_eq!(
        session_paths,
        vec![
            "b".to_string(),
            "b/deep".to_string(),
            "b\\win/deep".to_string()
        ]
    );
}

#[tokio::test]
async fn delete_folder_catalog_tombstones_nested_and_clears_sessions() {
    let db = test_db().await;
    sqlx::query(
        "INSERT INTO folders (id, path) VALUES ('f-1', 'a'), ('f-2', 'a/sub'), ('f-3', 'other')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO folder_attachments (id, folder_path, relative_path)
         VALUES ('fa-1', 'a/sub', 'materials/x'), ('fa-2', 'other', 'materials/y')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, owner_user_id, folder_path)
         VALUES ('s-1', 'u', 'a'), ('s-2', 'u', 'a\\win'), ('s-3', 'u', 'other')",
    )
    .execute(db.pool())
    .await
    .unwrap();

    delete_folder_catalog(
        db.pool(),
        DeleteFolderCatalogRequest {
            path: "a".to_string(),
        },
    )
    .await
    .unwrap();

    let live_folders: Vec<String> =
        sqlx::query_scalar("SELECT id FROM folders WHERE deleted_at IS NULL ORDER BY id")
            .fetch_all(db.pool())
            .await
            .unwrap();
    assert_eq!(live_folders, vec!["f-3".to_string()]);

    let live_materials: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM folder_attachments WHERE deleted_at IS NULL ORDER BY id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(live_materials, vec!["fa-2".to_string()]);

    let session_paths: Vec<String> =
        sqlx::query_scalar("SELECT folder_path FROM sessions ORDER BY id")
            .fetch_all(db.pool())
            .await
            .unwrap();
    assert_eq!(
        session_paths,
        vec!["".to_string(), "".to_string(), "other".to_string()]
    );
}

#[tokio::test]
async fn update_folder_icon_creates_folder_row() {
    let db = test_db().await;
    update_folder_icon(
        db.pool(),
        UpdateFolderIconRequest {
            path: "docs".to_string(),
            icon_json: "{\"type\":\"icon\",\"value\":\"star\"}".to_string(),
        },
    )
    .await
    .unwrap();
    let icon: String = sqlx::query_scalar("SELECT icon_json FROM folders WHERE path = 'docs'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(icon, "{\"type\":\"icon\",\"value\":\"star\"}");
}

#[tokio::test]
async fn update_folder_instructions_and_workspace() {
    let db = test_db().await;
    sqlx::query("INSERT INTO folders (id, path) VALUES ('f-1', 'docs')")
        .execute(db.pool())
        .await
        .unwrap();

    update_folder_instructions(
        db.pool(),
        UpdateFolderInstructionsRequest {
            path: "docs".to_string(),
            instructions: "Be brief".to_string(),
        },
    )
    .await
    .unwrap();
    update_folder_workspace(
        db.pool(),
        UpdateFolderWorkspaceRequest {
            path: "docs".to_string(),
            workspace_id: "workspace-9".to_string(),
        },
    )
    .await
    .unwrap();

    let row = sqlx::query("SELECT instructions, workspace_id FROM folders WHERE id = 'f-1'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("instructions"), "Be brief");
    assert_eq!(row.get::<String, _>("workspace_id"), "workspace-9");
}
