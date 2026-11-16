use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

use desktop_runtime::{Profile, RuntimeHandle};
use futures::executor::block_on;

use super::*;

fn row(key: &str, value: Value, rank: u64) -> Value {
    json!({ "id": key, "value_json": value.to_string(), "source_rank": rank })
}

#[test]
fn direct_synced_legacy_defaults_and_corruption_follow_shipping_precedence() {
    let rows = [
        row(
            LEGACY_SETTINGS,
            json!({"general": {"theme": "dark", "saveAudioAfterMeeting": false}, "language": {"ai_language": ""}}),
            0,
        ),
        row(
            LEGACY_MAIN,
            json!({"export_directory": "/legacy", "spoken_languages": "en, ja, "}),
            0,
        ),
        row("theme", json!("light"), 0),
        row("theme", json!("system"), 1),
        row("export_directory", json!(""), 0),
        json!({"id": "ai_language", "value_json": "{broken", "source_rank": 0}),
    ];
    let snapshot = decode(&rows).unwrap();
    assert_eq!(snapshot["theme"].value, "system");
    assert_eq!(snapshot["theme"].source, "Synced");
    assert_eq!(snapshot["export_directory"].value, "");
    assert_eq!(snapshot["ai_language"].value, "");
    assert_eq!(snapshot["ai_language"].source, "Legacy");
    assert_eq!(snapshot["audio_retention"].value, "none");
    assert_eq!(snapshot["spoken_languages"].value, r#"["en","ja"]"#);
    assert_eq!(snapshot["automatic_updates"].value, true);
    assert!(Arc::ptr_eq(
        &snapshot["theme"].revision.context,
        &snapshot["ai_language"].revision.context
    ));
}

#[test]
fn invalid_synced_value_falls_back_to_legacy_without_reviving_shadowed_local() {
    let snapshot = decode(&[
        row("theme", json!("light"), 0),
        row("theme", json!(false), 1),
        row(LEGACY_SETTINGS, json!({"general": {"theme": "dark"}}), 0),
    ])
    .unwrap();
    assert_eq!(snapshot["theme"].value, "dark");
    assert_eq!(snapshot["theme"].source, "Legacy");
}

#[test]
fn normalization_retains_empty_choices_and_rejects_invalid_direct_values() {
    let def = definition("spoken_languages").unwrap();
    assert_eq!(normalize(def, json!([]), true), Some(json!("[]")));
    assert_eq!(normalize(def, json!("[]"), true), Some(json!("[]")));
    assert_eq!(normalize(def, json!("en,ja"), true), None);
    assert_eq!(
        normalize(def, json!("en,ja"), false),
        Some(json!(r#"["en","ja"]"#))
    );
    assert_eq!(normalize(def, json!("{}"), false), None);
    let def = definition("audio_retention").unwrap();
    assert_eq!(normalize(def, json!(false), true), Some(json!("none")));
    assert_eq!(normalize(def, json!(true), true), Some(json!("forever")));
    assert_eq!(normalize(def, json!("unknown"), true), None);
    assert_eq!(
        normalize(definition("export_directory").unwrap(), json!(""), true),
        Some(json!(""))
    );
    assert_eq!(
        normalize(definition("autostart").unwrap(), json!("false"), true),
        None
    );
}

#[test]
fn drafts_survive_snapshots_and_keep_original_conflict_revision() {
    let initial = decode(&[row("export_directory", json!("/old"), 0)]).unwrap();
    let changed = decode(&[row("export_directory", json!("/remote"), 0)]).unwrap();
    let mut draft = Draft::new(initial["export_directory"].clone());
    draft.dirty = true;
    draft.text = "/my-edit".into();
    draft.observe(changed["export_directory"].clone());
    assert_eq!(draft.text, "/my-edit");
    assert_eq!(draft.base, initial["export_directory"]);
    draft.dirty = false;
    draft.saving = true;
    draft.observe(changed["export_directory"].clone());
    assert_eq!(draft.text, "/my-edit");
    draft.saving = false;
    draft.observe(changed["export_directory"].clone());
    assert_eq!(draft.text, "/remote");
}

#[test]
fn restore_uses_latest_snapshot_and_save_ack_keeps_edits_typed_while_pending() {
    let original = decode(&[row("export_directory", json!("/original"), 0)]).unwrap();
    let saved = decode(&[row("export_directory", json!("/submitted"), 0)]).unwrap();
    let remote = decode(&[row("export_directory", json!("/remote"), 0)]).unwrap();
    let mut draft = Draft::new(original["export_directory"].clone());
    draft.dirty = true;
    draft.saving = true;
    draft.text = "/typed-while-saving".into();
    draft.observe(remote["export_directory"].clone());
    draft.saved(saved["export_directory"].clone(), "/submitted");
    assert!(draft.dirty);
    assert!(!draft.saving);
    assert_eq!(draft.base, saved["export_directory"]);
    assert_eq!(draft.text, "/typed-while-saving");
    draft.restore();
    assert!(!draft.dirty);
    assert_eq!(draft.text, "/remote");
}

#[test]
fn oversized_values_are_errors_not_empty_successes() {
    assert!(matches!(
        decode(&[json!({"id": "export_directory", "oversized": 1})]),
        Err(ServiceError::Unsupported(_))
    ));
}

struct Fixture {
    directory: PathBuf,
    runtime: RuntimeHandle,
}

impl Fixture {
    async fn start() -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let directory = PathBuf::from(std::env::var_os("HOME").expect("test HOME"))
            .join(".cache/anarlog-gpui-product-tests")
            .join(format!(
                "{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&directory).unwrap();
        let (runtime, ready) = RuntimeHandle::start(Profile {
            database: directory.join("library.sqlite"),
        })
        .unwrap();
        ready.receive().await.unwrap();
        Self { directory, runtime }
    }

    async fn insert(&self, key: &str, value: Value, synced: bool) {
        let key = key.to_string();
        self.runtime
            .submit(move |services| async move {
                services
                    .executor
                    .execute(
                        format!(
                            "INSERT INTO {} (id, value_json, updated_at) VALUES (?, ?, 'fixture')
                 ON CONFLICT(id) DO UPDATE SET value_json = excluded.value_json",
                            if synced {
                                "synced_preferences"
                            } else {
                                "app_settings"
                            }
                        ),
                        vec![json!(key), json!(value.to_string())],
                    )
                    .await
                    .map_err(|error| ServiceError::Failed(error.to_string().into()))?;
                Ok(())
            })
            .unwrap()
            .receive()
            .await
            .unwrap();
    }

    async fn snapshot(&self) -> Snapshot {
        let watch = watch(&self.runtime).unwrap().receive().await.unwrap();
        let result = decode(&watch.snapshots.borrow().rows).unwrap();
        watch.unsubscribe().await.unwrap();
        result
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let runtime = self.runtime.clone();
        let _ = std::thread::spawn(move || block_on(runtime.shutdown())).join();
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn canonical_database_writes_are_durable_independent_and_conflict_checked() {
    block_on(async {
        let mut fixture = Fixture::start().await;
        let original = fixture.snapshot().await;
        let updated = save(
            &fixture.runtime,
            "export_directory".into(),
            original["export_directory"].clone(),
            json!("/notes"),
        )
        .unwrap()
        .receive()
        .await
        .unwrap();
        assert_eq!(updated.value, "/notes");
        assert!(matches!(
            save(
                &fixture.runtime,
                "export_directory".into(),
                original["export_directory"].clone(),
                json!("/overwrite")
            )
            .unwrap()
            .receive()
            .await,
            Err(ServiceError::Conflict)
        ));
        save(
            &fixture.runtime,
            "summary_length".into(),
            original["summary_length"].clone(),
            json!("concise"),
        )
        .unwrap()
        .receive()
        .await
        .unwrap();
        let current = fixture.snapshot().await;
        assert_eq!(current["export_directory"].value, "/notes");
        assert_eq!(current["summary_length"].value, "concise");
        drop(
            save(
                &fixture.runtime,
                "export_directory".into(),
                current["export_directory"].clone(),
                json!("/admitted-write"),
            )
            .unwrap(),
        );
        fixture.runtime.shutdown().await.unwrap();
        let (runtime, ready) = RuntimeHandle::start(Profile {
            database: fixture.directory.join("library.sqlite"),
        })
        .unwrap();
        ready.receive().await.unwrap();
        fixture.runtime = runtime;
        let reopened = fixture.snapshot().await;
        assert_eq!(reopened["export_directory"].value, "/admitted-write");
        assert_eq!(reopened["summary_length"].value, "concise");
    });
}

#[test]
fn workspace_binding_change_blocks_stale_synced_preference_and_correctly_scopes_retry() {
    block_on(async {
        let fixture = Fixture::start().await;
        fixture
            .insert(BINDING, json!({"workspace_id": "workspace-a"}), false)
            .await;
        let old = fixture.snapshot().await;
        fixture
            .insert(BINDING, json!({"workspace_id": "workspace-b"}), false)
            .await;
        assert!(matches!(
            save(
                &fixture.runtime,
                "theme".into(),
                old["theme"].clone(),
                json!("dark")
            )
            .unwrap()
            .receive()
            .await,
            Err(ServiceError::Conflict)
        ));
        let current = fixture.snapshot().await;
        save(
            &fixture.runtime,
            "theme".into(),
            current["theme"].clone(),
            json!("dark"),
        )
        .unwrap()
        .receive()
        .await
        .unwrap();
        let rows = fixture
            .runtime
            .submit(|services| async move {
                services
                    .executor
                    .execute(
                        "SELECT workspace_id FROM synced_preferences WHERE id = 'theme'".into(),
                        vec![],
                    )
                    .await
                    .map_err(|error| ServiceError::Failed(error.to_string().into()))
            })
            .unwrap()
            .receive()
            .await
            .unwrap();
        assert_eq!(rows[0]["workspace_id"], "workspace-b");
        assert_eq!(fixture.snapshot().await["theme"].value, "dark");
    });
}

#[test]
fn side_effect_preferences_never_claim_persistence_without_platform_services() {
    block_on(async {
        let fixture = Fixture::start().await;
        let snapshot = fixture.snapshot().await;
        for key in [
            "autostart",
            "lock_app",
            "cloud_sync_enabled",
            "current_stt_model",
        ] {
            assert!(matches!(
                save(
                    &fixture.runtime,
                    key.into(),
                    snapshot[key].clone(),
                    json!(true)
                ),
                Err(ServiceError::Unsupported(_))
            ));
        }
        assert_eq!(fixture.snapshot().await, snapshot);
    });
}

#[test]
fn settings_watch_omits_unrelated_and_secret_rows() {
    block_on(async {
        let fixture = Fixture::start().await;
        fixture
            .runtime
            .submit(|services| async move {
                services
                    .executor
                    .execute(
                        "DELETE FROM app_settings WHERE id = 'cloudsync_workspace_binding'".into(),
                        vec![],
                    )
                    .await
                    .map_err(|error| ServiceError::Failed(error.to_string().into()))?;
                Ok(())
            })
            .unwrap()
            .receive()
            .await
            .unwrap();
        fixture
            .insert("provider_secret", json!("fixture-not-a-real-secret"), false)
            .await;
        fixture
            .insert("export_directory", json!("/fixture"), false)
            .await;
        let watch = watch(&fixture.runtime).unwrap().receive().await.unwrap();
        let rows = watch.snapshots.borrow().rows.clone();
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0]["id"], "export_directory");
        watch.unsubscribe().await.unwrap();
    });
}

#[test]
#[ignore = "Explicit micro-workload; not an application performance comparison"]
fn settings_decode_micro_workload() {
    let mut rows: Vec<Value> = definitions()
        .iter()
        .map(|def| row(&def.key, def.default.clone(), 0))
        .collect();
    rows.push(row(
        LEGACY_SETTINGS,
        json!({"unused_preserved_data": "x".repeat(256 * 1024)}),
        0,
    ));
    let mut elapsed = Vec::with_capacity(1000);
    for _ in 0..1000 {
        let started = Instant::now();
        let result = std::hint::black_box(decode(std::hint::black_box(&rows)).unwrap());
        assert_eq!(result.len(), definitions().len());
        elapsed.push(started.elapsed().as_micros());
    }
    elapsed.sort_unstable();
    eprintln!(
        "settings_decode_micro_workload: profile={} iterations=1000 rows={} legacy_bytes=262144 p50_us={} p95_us={} max_us={}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        rows.len(),
        elapsed[500],
        elapsed[950],
        elapsed[999]
    );
}
