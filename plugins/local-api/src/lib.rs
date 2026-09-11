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
    async fn configured_markdown_export_filters_every_content_combination() {
        let pool = seeded_pool().await;
        let mut export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        let mut summary = export.meeting.note.clone().unwrap();
        summary.title = "Summary".to_string();
        summary.markdown = "Summary-only text".to_string();
        export.meeting.summaries.push(summary);
        export
            .meeting
            .action_items
            .push(anlg_agent_access::ActionItem {
                id: "action-1".to_string(),
                assignee_human_id: String::new(),
                status: "open".to_string(),
                text: "Action-only text".to_string(),
                due_at: String::new(),
                completed_at: None,
            });
        let directory =
            std::env::temp_dir().join(format!("anlg-md-options-{}", uuid::Uuid::new_v4()));
        for bits in 0..16 {
            let options = MarkdownExportOptions {
                include_memo: bits & 1 != 0,
                include_summary: bits & 2 != 0,
                include_transcript: bits & 4 != 0,
                include_action_items: bits & 8 != 0,
                filename: format!("selection-{bits}"),
                include_id_suffix: false,
            };
            let result =
                commands::write_markdown_export_with_options(&directory, &export, Some(&options));
            if bits == 0 {
                assert!(result.unwrap_err().contains("at least one"));
                assert!(!directory.exists());
                continue;
            }
            let path = result.unwrap();
            let markdown = std::fs::read_to_string(path).unwrap();
            assert!(markdown.starts_with("# Planning\n"));
            assert_eq!(markdown.contains("Launch decision"), options.include_memo);
            assert_eq!(
                markdown.contains("Summary-only text"),
                options.include_summary
            );
            assert_eq!(markdown.contains("hello world"), options.include_transcript);
            assert_eq!(
                markdown.contains("Action-only text"),
                options.include_action_items
            );
        }
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 15);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn configured_markdown_filenames_are_safe_and_support_patterns() {
        let pool = seeded_pool().await;
        let export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        for (name, expected) in [
            ("Recap.md", "Recap.md"),
            ("{date} {title} recap", "2026-07-13 Planning recap.md"),
            ("../outside/report", "_outside_report.md"),
            ("CON", "_CON.md"),
            ("...", "Untitled meeting.md"),
            ("  ", "2026-07-13 Planning.md"),
            ("recap.MD", "recap.md"),
            ("a\nb", "a_b.md"),
        ] {
            let options = MarkdownExportOptions {
                filename: name.to_string(),
                include_id_suffix: false,
                ..Default::default()
            };
            assert_eq!(
                commands::configured_markdown_filename(&export.meeting, &options),
                expected
            );
        }
        let options = MarkdownExportOptions {
            filename: "Recap".to_string(),
            ..Default::default()
        };
        assert_eq!(
            commands::configured_markdown_filename(&export.meeting, &options),
            "Recap [meeting-].md"
        );
        let options = MarkdownExportOptions {
            filename: "会".repeat(300),
            ..Default::default()
        };
        let name = commands::configured_markdown_filename(&export.meeting, &options);
        assert!(name.len() < 255);
        assert_eq!(std::path::Path::new(&name).components().count(), 1);
        let defaults: MarkdownExportOptions = serde_json::from_str("{}").unwrap();
        assert_eq!(defaults, MarkdownExportOptions::default());
    }

    #[tokio::test]
    async fn configured_actions_keep_separate_exports_for_the_same_meeting() {
        let pool = seeded_pool().await;
        let export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        let directory =
            std::env::temp_dir().join(format!("anlg-md-actions-{}", uuid::Uuid::new_v4()));
        let memo_options = MarkdownExportOptions {
            filename: "Memo".to_string(),
            include_transcript: false,
            ..Default::default()
        };
        let transcript_options = MarkdownExportOptions {
            filename: "Transcript".to_string(),
            include_memo: false,
            ..Default::default()
        };
        let memo =
            commands::write_markdown_export_with_options(&directory, &export, Some(&memo_options))
                .unwrap();
        let transcript = commands::write_markdown_export_with_options(
            &directory,
            &export,
            Some(&transcript_options),
        )
        .unwrap();
        assert!(memo.exists());
        assert!(transcript.exists());
        assert!(
            !std::fs::read_to_string(memo)
                .unwrap()
                .contains("hello world")
        );
        assert!(
            !std::fs::read_to_string(transcript)
                .unwrap()
                .contains("Launch decision")
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[tokio::test]
    async fn failed_markdown_writes_leave_no_partial_export_and_can_be_retried() {
        use std::io::Write;

        let pool = seeded_pool().await;
        let export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        let options = MarkdownExportOptions {
            filename: "Recap".to_string(),
            include_id_suffix: false,
            ..Default::default()
        };
        for replace_existing in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("Recap.md");
            if replace_existing {
                std::fs::write(&path, format!("{}\n", export.to_markdown())).unwrap();
            }
            let before = std::fs::read(&path).ok();
            let error = commands::persist_markdown_export(&path, replace_existing, |file| {
                file.write_all(b"# Partial")?;
                Err(std::io::Error::other("simulated write failure"))
            })
            .unwrap_err();
            assert_eq!(error.to_string(), "simulated write failure");
            assert_eq!(std::fs::read(&path).ok(), before);
            assert_eq!(
                std::fs::read_dir(directory.path()).unwrap().count(),
                usize::from(replace_existing)
            );
            commands::write_markdown_export_with_options(directory.path(), &export, Some(&options))
                .unwrap();
            assert!(
                std::fs::read_to_string(&path)
                    .unwrap()
                    .contains("hello world")
            );
        }
    }

    #[tokio::test]
    async fn simultaneous_markdown_exports_complete_without_false_collisions() {
        let pool = seeded_pool().await;
        let mut export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        export.meeting.note.as_mut().unwrap().markdown = "Memo content. ".repeat(20_000);
        let directory = tempfile::tempdir().unwrap();
        let barrier = std::sync::Barrier::new(8);
        let options = MarkdownExportOptions::default();
        std::thread::scope(|scope| {
            let handles = (0..8)
                .map(|index| {
                    let export = &export;
                    let directory = directory.path();
                    let barrier = &barrier;
                    let options = &options;
                    scope.spawn(move || {
                        barrier.wait();
                        for _ in 0..4 {
                            commands::write_markdown_export_with_options(
                                directory,
                                export,
                                (index % 2 == 0).then_some(options),
                            )
                            .unwrap();
                        }
                    })
                })
                .collect::<Vec<_>>();
            for handle in handles {
                handle.join().unwrap();
            }
        });
        let path = directory
            .path()
            .join(commands::markdown_export_filename(&export.meeting));
        let written = std::fs::read_to_string(path).unwrap();
        let content = written
            .strip_prefix("<!-- anarlog:legacy-markdown-export \"meeting-1\" -->\n\n")
            .unwrap_or(&written);
        assert_eq!(content, format!("{}\n", export.to_markdown()));
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn markdown_replacement_keeps_existing_permission_bits() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let pool = seeded_pool().await;
        let export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let options = MarkdownExportOptions::default();
        let path =
            commands::write_markdown_export_with_options(directory.path(), &export, Some(&options))
                .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
        let original = std::fs::metadata(&path).unwrap();
        commands::write_markdown_export_with_options(directory.path(), &export, Some(&options))
            .unwrap();
        let updated = std::fs::metadata(path).unwrap();
        assert_eq!(updated.permissions().mode() & 0o777, 0o640);
        assert_eq!(updated.uid(), original.uid());
        assert_eq!(updated.gid(), original.gid());
    }

    #[tokio::test]
    async fn legacy_cleanup_keeps_configured_and_unmarked_exports() {
        let pool = seeded_pool().await;
        let mut export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let legacy = commands::write_markdown_export(directory.path(), &export).unwrap();
        let options = MarkdownExportOptions {
            filename: "Memo".to_string(),
            ..Default::default()
        };
        let configured =
            commands::write_markdown_export_with_options(directory.path(), &export, Some(&options))
                .unwrap();
        let unmarked = directory.path().join("Older export [meeting-].md");
        std::fs::write(&unmarked, format!("{}\n", export.to_markdown())).unwrap();
        let mut other = export.clone();
        other.meeting.id = "meeting-2".to_string();
        other.meeting.title = "Another meeting".to_string();
        let same_prefix = commands::write_markdown_export(directory.path(), &other).unwrap();

        export.meeting.title = "Planning updated".to_string();
        commands::write_markdown_export(directory.path(), &export).unwrap();
        assert!(!legacy.exists());
        assert!(configured.exists());
        assert!(unmarked.exists());
        assert!(same_prefix.exists());
    }

    #[tokio::test]
    async fn configured_export_takes_ownership_of_a_shared_legacy_filename() {
        let pool = seeded_pool().await;
        let mut export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = commands::write_markdown_export(directory.path(), &export).unwrap();
        commands::write_markdown_export_with_options(
            directory.path(),
            &export,
            Some(&MarkdownExportOptions::default()),
        )
        .unwrap();
        assert!(
            !std::fs::read_to_string(&path)
                .unwrap()
                .starts_with("<!-- anarlog:legacy-markdown-export")
        );
        export.meeting.title = "Planning updated".to_string();
        commands::write_markdown_export(directory.path(), &export).unwrap();
        assert!(path.exists());
    }

    #[tokio::test]
    async fn configured_markdown_export_updates_its_own_file_but_rejects_collisions() {
        let pool = seeded_pool().await;
        let mut export = anlg_agent_access::get_meeting_export(&pool, "meeting-1".to_string())
            .await
            .unwrap();
        let directory =
            std::env::temp_dir().join(format!("anlg-md-collision-{}", uuid::Uuid::new_v4()));
        let options = MarkdownExportOptions {
            filename: "Recap".to_string(),
            include_id_suffix: false,
            ..Default::default()
        };
        let path =
            commands::write_markdown_export_with_options(&directory, &export, Some(&options))
                .unwrap();
        export.meeting.note.as_mut().unwrap().markdown = "Updated memo".to_string();
        assert_eq!(
            commands::write_markdown_export_with_options(&directory, &export, Some(&options))
                .unwrap(),
            path
        );
        let updated = std::fs::read_to_string(&path).unwrap();
        assert!(updated.contains("Updated memo"));
        export.meeting.id = "another-meeting".to_string();
        let error =
            commands::write_markdown_export_with_options(&directory, &export, Some(&options))
                .unwrap_err();
        assert!(error.contains("already exists"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), updated);
        std::fs::write(&path, "My unrelated notes").unwrap();
        assert!(
            commands::write_markdown_export_with_options(&directory, &export, Some(&options))
                .is_err()
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "My unrelated notes"
        );
        std::fs::remove_dir_all(directory).unwrap();
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
