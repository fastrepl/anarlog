use db_migrate::{DbSchema, MigrateError, MigrationScope, MigrationStep, migrate};
use hypr_db_core2::Db3;

const CREATE_WIDGETS_SQL: &str = r#"
CREATE TABLE widgets (
    id INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL DEFAULT ''
)
"#;

const CREATE_CLOUDSYNC_WIDGETS_SQL: &str = r#"
CREATE TABLE widgets (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL DEFAULT ''
)
"#;

const CREATE_WIDGETS_WITH_STATUS_SQL: &str = r#"
CREATE TABLE widgets (
    id INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'draft'
)
"#;

const SEED_WIDGETS_SQL: &str = r#"
INSERT INTO widgets (id, name) VALUES (1, 'alpha')
"#;

const ADD_SLUG_SQL: &str = r#"
ALTER TABLE widgets ADD COLUMN slug TEXT NOT NULL DEFAULT ''
"#;

const PLAIN_SCHEMA_STEPS: &[MigrationStep] = &[
    MigrationStep {
        id: "20260415010101_create_widgets",
        scope: MigrationScope::Plain,
        sql: CREATE_WIDGETS_SQL,
    },
    MigrationStep {
        id: "20260415010102_seed_widgets",
        scope: MigrationScope::Plain,
        sql: SEED_WIDGETS_SQL,
    },
];

const MODIFIED_PLAIN_SCHEMA_STEPS: &[MigrationStep] = &[MigrationStep {
    id: "20260415010101_create_widgets",
    scope: MigrationScope::Plain,
    sql: CREATE_WIDGETS_WITH_STATUS_SQL,
}];

const MISSING_VERSION_SCHEMA_STEPS: &[MigrationStep] = &[MigrationStep {
    id: "20260415010102_seed_widgets",
    scope: MigrationScope::Plain,
    sql: SEED_WIDGETS_SQL,
}];

const DUPLICATE_VERSION_SCHEMA_STEPS: &[MigrationStep] = &[
    MigrationStep {
        id: "20260415020101_create_widgets",
        scope: MigrationScope::Plain,
        sql: CREATE_WIDGETS_SQL,
    },
    MigrationStep {
        id: "20260415020101_seed_widgets",
        scope: MigrationScope::Plain,
        sql: SEED_WIDGETS_SQL,
    },
];

const INVALID_STEP_ID_SCHEMA_STEPS: &[MigrationStep] = &[MigrationStep {
    id: "invalid_step_id",
    scope: MigrationScope::Plain,
    sql: CREATE_WIDGETS_SQL,
}];

const CLOUDSYNC_BASE_STEPS: &[MigrationStep] = &[MigrationStep {
    id: "20260415030101_create_widgets",
    scope: MigrationScope::Plain,
    sql: CREATE_CLOUDSYNC_WIDGETS_SQL,
}];

const CLOUDSYNC_ALTER_STEPS: &[MigrationStep] = &[
    MigrationStep {
        id: "20260415030101_create_widgets",
        scope: MigrationScope::Plain,
        sql: CREATE_CLOUDSYNC_WIDGETS_SQL,
    },
    MigrationStep {
        id: "20260415030102_add_slug",
        scope: MigrationScope::CloudsyncAlter {
            table_name: "widgets",
        },
        sql: ADD_SLUG_SQL,
    },
];

fn schema(steps: &'static [MigrationStep], validate_cloudsync_table: fn(&str) -> bool) -> DbSchema {
    DbSchema {
        steps,
        validate_cloudsync_table,
    }
}

fn never_synced(_: &str) -> bool {
    false
}

fn widgets_synced(table_name: &str) -> bool {
    table_name == "widgets"
}

async fn open_plain_db() -> Db3 {
    Db3::connect_memory_plain().await.unwrap()
}

async fn applied_versions(db: &Db3) -> Vec<i64> {
    sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
        .fetch_all(db.pool().as_ref())
        .await
        .unwrap()
}

async fn widget_names(db: &Db3) -> Vec<String> {
    sqlx::query_scalar("SELECT name FROM widgets ORDER BY id")
        .fetch_all(db.pool().as_ref())
        .await
        .unwrap()
}

async fn widget_columns(db: &Db3) -> Vec<String> {
    sqlx::query_scalar("SELECT name FROM pragma_table_info('widgets') ORDER BY cid")
        .fetch_all(db.pool().as_ref())
        .await
        .unwrap()
}

#[tokio::test]
async fn plain_migrations_apply_and_remain_idempotent() {
    let db = open_plain_db().await;

    migrate(&db, schema(PLAIN_SCHEMA_STEPS, never_synced))
        .await
        .unwrap();

    assert_eq!(
        applied_versions(&db).await,
        vec![20260415010101, 20260415010102]
    );
    assert_eq!(widget_names(&db).await, vec!["alpha".to_string()]);

    migrate(&db, schema(PLAIN_SCHEMA_STEPS, never_synced))
        .await
        .unwrap();

    assert_eq!(
        applied_versions(&db).await,
        vec![20260415010101, 20260415010102]
    );
    assert_eq!(widget_names(&db).await, vec!["alpha".to_string()]);
}

#[tokio::test]
async fn changed_checksum_is_rejected() {
    let db = open_plain_db().await;

    migrate(&db, schema(&PLAIN_SCHEMA_STEPS[..1], never_synced))
        .await
        .unwrap();

    let err = migrate(&db, schema(MODIFIED_PLAIN_SCHEMA_STEPS, never_synced))
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        MigrateError::SqlxMigrate(sqlx::migrate::MigrateError::VersionMismatch(20260415010101))
    ));
}

#[tokio::test]
async fn missing_applied_version_is_rejected() {
    let db = open_plain_db().await;

    migrate(&db, schema(PLAIN_SCHEMA_STEPS, never_synced))
        .await
        .unwrap();

    let err = migrate(&db, schema(MISSING_VERSION_SCHEMA_STEPS, never_synced))
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        MigrateError::SqlxMigrate(sqlx::migrate::MigrateError::VersionMissing(20260415010101))
    ));
}

#[tokio::test]
async fn invalid_manifest_metadata_is_rejected() {
    let db = open_plain_db().await;

    let invalid_id = migrate(&db, schema(INVALID_STEP_ID_SCHEMA_STEPS, never_synced))
        .await
        .unwrap_err();
    assert!(matches!(
        invalid_id,
        MigrateError::InvalidStepId {
            step_id: "invalid_step_id"
        }
    ));

    let duplicate_version = migrate(&db, schema(DUPLICATE_VERSION_SCHEMA_STEPS, never_synced))
        .await
        .unwrap_err();
    assert!(matches!(
        duplicate_version,
        MigrateError::DuplicateStepVersion {
            version: 20260415020101,
            ..
        }
    ));
}

#[tokio::test]
async fn cloudsync_alter_scope_falls_back_to_plain_when_cloudsync_is_disabled() {
    let db = open_plain_db().await;

    migrate(&db, schema(CLOUDSYNC_BASE_STEPS, widgets_synced))
        .await
        .unwrap();
    migrate(&db, schema(CLOUDSYNC_ALTER_STEPS, widgets_synced))
        .await
        .unwrap();

    assert_eq!(
        applied_versions(&db).await,
        vec![20260415030101, 20260415030102]
    );
    assert_eq!(
        widget_columns(&db).await,
        vec!["id".to_string(), "name".to_string(), "slug".to_string()]
    );
}

#[cfg(any(
    all(test, target_os = "macos", target_arch = "aarch64"),
    all(test, target_os = "macos", target_arch = "x86_64"),
    all(test, target_os = "linux", target_env = "gnu", target_arch = "aarch64"),
    all(test, target_os = "linux", target_env = "gnu", target_arch = "x86_64"),
    all(
        test,
        target_os = "linux",
        target_env = "musl",
        target_arch = "aarch64"
    ),
    all(test, target_os = "linux", target_env = "musl", target_arch = "x86_64"),
    all(test, target_os = "windows", target_arch = "x86_64"),
))]
#[tokio::test]
async fn cloudsync_alter_scope_runs_successfully_on_a_cloudsync_table() {
    let db = Db3::connect_memory().await.unwrap();

    migrate(&db, schema(CLOUDSYNC_BASE_STEPS, widgets_synced))
        .await
        .unwrap();

    db.cloudsync_init("widgets", None, None).await.unwrap();

    let enabled: bool = sqlx::query_scalar("SELECT cloudsync_is_enabled('widgets')")
        .fetch_one(db.pool().as_ref())
        .await
        .unwrap();
    assert!(enabled);

    migrate(&db, schema(CLOUDSYNC_ALTER_STEPS, widgets_synced))
        .await
        .unwrap();

    assert_eq!(
        applied_versions(&db).await,
        vec![20260415030101, 20260415030102]
    );
    assert_eq!(
        widget_columns(&db).await,
        vec!["id".to_string(), "name".to_string(), "slug".to_string()]
    );
}
