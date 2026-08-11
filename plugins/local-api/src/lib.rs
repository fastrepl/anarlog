mod commands;
mod dispatch;
mod types;

pub use dispatch::{EVENT_MEETING_COMPLETED, EVENT_NOTE_ENHANCED, KNOWN_EVENTS};
pub use types::*;

const PLUGIN_NAME: &str = "local-api";

fn make_specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .plugin_name(PLUGIN_NAME)
        .events(tauri_specta::collect_events![])
        .commands(tauri_specta::collect_commands![
            commands::list_webhooks::<tauri::Wry>,
            commands::create_webhook::<tauri::Wry>,
            commands::delete_webhook::<tauri::Wry>,
            commands::set_webhook_active::<tauri::Wry>,
            commands::test_webhook::<tauri::Wry>,
            commands::dispatch_event::<tauri::Wry>,
            commands::export_meeting_markdown::<tauri::Wry>,
            commands::get_cloud_snapshot::<tauri::Wry>,
            commands::list_cloud_snapshot_ids::<tauri::Wry>,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);
            Ok(())
        })
        .build()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn export_types() {
        const OUTPUT_FILE: &str = "./js/bindings.gen.ts";

        make_specta_builder()
            .export(
                specta_typescript::Typescript::default()
                    .formatter(specta_typescript::formatter::prettier)
                    .bigint(specta_typescript::BigIntExportBehavior::Number),
                OUTPUT_FILE,
            )
            .unwrap();

        let content = std::fs::read_to_string(OUTPUT_FILE).unwrap();
        std::fs::write(OUTPUT_FILE, format!("// @ts-nocheck\n{content}")).unwrap();
    }

    async fn seeded_pool() -> sqlx::SqlitePool {
        let db = anlg_db_core::Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, title, started_at, series_id) \
             VALUES ('meeting-1', 'Planning', '2026-07-13', 'series-1')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO session_documents (id, session_id, kind, body_format, body, title) \
             VALUES ('note-1', 'meeting-1', 'note', 'markdown', 'Launch decision', 'Notes')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO transcripts (id, session_id, started_at_ms, words_json) \
             VALUES ('transcript-1', 'meeting-1', 0, '[{\"text\":\"hello\"},{\"text\":\"world\"}]')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db.pool().clone()
    }

    #[tokio::test]
    async fn markdown_export_writes_stable_file_into_directory() {
        let pool = seeded_pool().await;
        let export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();

        assert_eq!(
            commands::markdown_export_filename(&export.meeting),
            "2026-07-13 Planning [meeting-].md"
        );

        let untitled = anlg_agent_access::Meeting {
            title: "  ".to_string(),
            started_at: String::new(),
            created_at: "bad".to_string(),
            ..export.meeting.clone()
        };
        assert_eq!(
            commands::markdown_export_filename(&untitled),
            "Untitled meeting [meeting-].md"
        );

        let hostile = anlg_agent_access::Meeting {
            title: "a/b:c*d?".to_string(),
            ..export.meeting.clone()
        };
        assert_eq!(
            commands::markdown_export_filename(&hostile),
            "2026-07-13 a_b_c_d_ [meeting-].md"
        );

        let directory =
            std::env::temp_dir().join(format!("anlg-md-export-{}", uuid::Uuid::new_v4()));
        let path = commands::write_markdown_export(&directory, &export).unwrap();
        assert_eq!(
            path.file_name().unwrap().to_string_lossy(),
            "2026-07-13 Planning [meeting-].md"
        );
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("# Planning"));
        assert!(written.contains("hello world"));

        let other_meeting_file = directory.join("2026-07-13 Other [meeting2].md");
        std::fs::write(&other_meeting_file, "other").unwrap();
        let mut retitled = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        retitled.meeting.title = "Planning follow-up".to_string();
        let renamed = commands::write_markdown_export(&directory, &retitled).unwrap();
        assert_eq!(
            renamed.file_name().unwrap().to_string_lossy(),
            "2026-07-13 Planning follow-up [meeting-].md"
        );
        assert!(!path.exists(), "stale export under the old title remains");
        assert!(other_meeting_file.exists());
        std::fs::remove_dir_all(&directory).ok();
    }

    #[tokio::test]
    async fn note_enhanced_reexports_markdown_and_records_the_run() {
        let pool = seeded_pool().await;
        let directory = std::env::temp_dir().join(format!("anlg-md-auto-{}", uuid::Uuid::new_v4()));
        for (id, value) in [
            (
                "automation_markdown_export_enabled",
                serde_json::json!(true),
            ),
            (
                "automation_markdown_export_directory",
                serde_json::json!(directory.to_string_lossy()),
            ),
        ] {
            sqlx::query("INSERT INTO app_settings (id, value_json) VALUES (?, ?)")
                .bind(id)
                .bind(value.to_string())
                .execute(&pool)
                .await
                .unwrap();
        }

        commands::run_markdown_export_automation(&pool, "meeting-1").await;

        let exported = directory.join("2026-07-13 Planning [meeting-].md");
        assert!(exported.exists());
        let last_run: String = sqlx::query_scalar(
            "SELECT value_json FROM app_settings \
             WHERE id = 'automation_markdown_export_last_run'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        // Stored the way the desktop settings layer writes string settings:
        // a JSON-encoded string containing the record JSON.
        let record: String = serde_json::from_str(&last_run).unwrap();
        let record: serde_json::Value = serde_json::from_str(&record).unwrap();
        assert_eq!(record["status"], "success");
        assert_eq!(record["detail"], exported.to_string_lossy().into_owned());
        assert!(record["at"].as_str().is_some_and(|at| at.ends_with('Z')));
        std::fs::remove_dir_all(&directory).ok();
    }

    #[tokio::test]
    async fn note_enhanced_export_skips_silently_without_configuration() {
        let pool = seeded_pool().await;

        commands::run_markdown_export_automation(&pool, "meeting-1").await;

        let row: Option<String> = sqlx::query_scalar(
            "SELECT value_json FROM app_settings \
             WHERE id = 'automation_markdown_export_last_run'",
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert!(row.is_none());
    }

    #[tokio::test]
    async fn oversized_cloud_snapshot_keeps_text_and_drops_word_payloads() {
        let pool = seeded_pool().await;
        let mut export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        export.transcripts[0].words =
            vec![serde_json::json!({ "text": "x".repeat(2 * 1024 * 1024) })];
        export.transcripts[0].speaker_hints = vec![serde_json::json!({ "name": "x".repeat(1024) })];

        let snapshot = commands::prepare_cloud_snapshot(export).unwrap();

        assert_eq!(snapshot["id"], "meeting-1");
        assert!(snapshot.get("meeting").is_none());
        assert_eq!(snapshot["transcripts"][0]["text"], "hello world");
        assert_eq!(snapshot["transcripts"][0]["words"], serde_json::json!([]));
        assert_eq!(
            snapshot["transcripts"][0]["speaker_hints"],
            serde_json::json!([])
        );
    }
}
