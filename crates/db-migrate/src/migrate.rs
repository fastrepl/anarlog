use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

use anlg_db_core::Db;
use sqlx::migrate::{
    AppliedMigration, Migrate, MigrateError as SqlxMigrateError, Migration, MigrationType,
};
use sqlx::{Executor, SqlSafeStr, Sqlite, SqliteConnection};

use crate::error::MigrateError;
use crate::schema::{DbSchema, MigrationScope, MigrationStep};

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Clone, Copy)]
struct StepMeta {
    scope: MigrationScope,
    breaking: bool,
}

struct DbMigrateConnection<'a> {
    db: &'a Db,
    conn: sqlx::pool::PoolConnection<Sqlite>,
    meta_by_version: HashMap<i64, StepMeta>,
}

impl<'a> DbMigrateConnection<'a> {
    fn new(
        db: &'a Db,
        conn: sqlx::pool::PoolConnection<Sqlite>,
        meta_by_version: HashMap<i64, StepMeta>,
    ) -> Self {
        Self {
            db,
            conn,
            meta_by_version,
        }
    }
}

pub(crate) async fn run_migrations(
    db: &Db,
    schema: DbSchema,
    on_progress: impl FnMut(crate::MigrationProgress) + Send,
) -> Result<(), MigrateError> {
    let resolved = resolve_migrations(schema)?;
    let meta_by_version = resolved
        .iter()
        .map(|(step, migration)| {
            (
                migration.version,
                StepMeta {
                    scope: step.scope,
                    breaking: is_breaking_step(step.sql),
                },
            )
        })
        .collect();
    let migrations: Vec<_> = resolved
        .into_iter()
        .map(|(_, migration)| migration)
        .collect();

    let conn = db.pool().acquire().await?;
    let mut conn = DbMigrateConnection::new(db, conn, meta_by_version);
    run_direct(&migrations, &mut conn, on_progress).await?;
    Ok(())
}

const MIGRATIONS_TABLE: &str = "_sqlx_migrations";

async fn run_direct(
    migrations: &[Migration],
    conn: &mut DbMigrateConnection<'_>,
    mut on_progress: impl FnMut(crate::MigrationProgress) + Send,
) -> Result<(), MigrateError> {
    conn.lock().await?;
    conn.ensure_migrations_table(MIGRATIONS_TABLE).await?;
    ensure_schema_compat_table(&mut conn.conn).await?;

    if let Some(version) = conn.dirty_version(MIGRATIONS_TABLE).await? {
        return Err(SqlxMigrateError::Dirty(version).into());
    }

    let applied_migrations = conn.list_applied_migrations(MIGRATIONS_TABLE).await?;
    let max_known_version = migrations
        .iter()
        .map(|migration| migration.version)
        .max()
        .unwrap_or(0);
    let max_applied_version = applied_migrations
        .iter()
        .map(|migration| migration.version)
        .max()
        .unwrap_or(0);
    validate_applied_migrations(&applied_migrations, migrations, max_known_version)?;

    // A database written by a newer build is fine to keep using as long as
    // none of its extra migrations were marked "-- breaking"; the compat
    // floor records the newest breaking migration ever applied.
    let min_supported_version = read_min_supported_version(&mut conn.conn).await?;
    if min_supported_version > max_known_version {
        return Err(MigrateError::SchemaFromNewerApp {
            min_supported_version,
            max_known_version,
        });
    }

    let applied_migrations: HashMap<_, _> = applied_migrations
        .into_iter()
        .map(|migration| (migration.version, migration))
        .collect();

    if max_applied_version > max_known_version
        && let Some(migration) = migrations.iter().find(|migration| {
            !migration.migration_type.is_down_migration()
                && !applied_migrations.contains_key(&migration.version)
        })
    {
        return Err(MigrateError::DatabaseAhead {
            missing_version: migration.version,
            max_applied_version,
        });
    }

    let pending_total = migrations
        .iter()
        .filter(|migration| {
            !migration.migration_type.is_down_migration()
                && !applied_migrations.contains_key(&migration.version)
        })
        .count();
    let mut completed = 0;

    for migration in migrations {
        if migration.migration_type.is_down_migration() {
            continue;
        }

        match applied_migrations.get(&migration.version) {
            Some(applied_migration) => {
                if migration.checksum != applied_migration.checksum {
                    return Err(SqlxMigrateError::VersionMismatch(migration.version).into());
                }
            }
            None => {
                if completed == 0 {
                    on_progress(crate::MigrationProgress {
                        completed,
                        total: pending_total,
                    });
                }
                conn.apply(MIGRATIONS_TABLE, migration).await?;
                completed += 1;
                on_progress(crate::MigrationProgress {
                    completed,
                    total: pending_total,
                });
            }
        }
    }

    conn.unlock().await?;
    Ok(())
}

fn validate_applied_migrations(
    applied_migrations: &[AppliedMigration],
    migrations: &[Migration],
    max_known_version: i64,
) -> Result<(), SqlxMigrateError> {
    let versions: HashSet<_> = migrations
        .iter()
        .map(|migration| migration.version)
        .collect();

    for applied_migration in applied_migrations {
        if versions.contains(&applied_migration.version) {
            continue;
        }
        // Versions above everything this build knows come from a newer build
        // and are tolerated (subject to the compat floor); an unknown version
        // interleaved with known ones means a divergent history.
        if applied_migration.version <= max_known_version {
            return Err(SqlxMigrateError::VersionMissing(applied_migration.version));
        }
    }

    Ok(())
}

// A step whose leading comment block contains a "-- breaking" line makes the
// schema unreadable by builds that don't include it (e.g. dropped or renamed
// columns); applying it raises the compat floor so older builds refuse to
// open the database with a clear "update Anarlog" message instead of
// misbehaving on a schema they don't understand.
fn is_breaking_step(sql: &str) -> bool {
    sql.lines()
        .take_while(|line| {
            let line = line.trim();
            line.is_empty() || line.starts_with("--")
        })
        .any(|line| line.trim() == "-- breaking")
}

async fn ensure_schema_compat_table(conn: &mut SqliteConnection) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS _anlg_schema_compat ( \
             id INTEGER PRIMARY KEY CHECK (id = 0), \
             min_supported_version INTEGER NOT NULL \
         )",
    )
    .execute(&mut *conn)
    .await?;

    sqlx::query(
        "INSERT OR IGNORE INTO _anlg_schema_compat (id, min_supported_version) VALUES (0, 0)",
    )
    .execute(&mut *conn)
    .await?;

    Ok(())
}

async fn read_min_supported_version(conn: &mut SqliteConnection) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT min_supported_version FROM _anlg_schema_compat WHERE id = 0")
        .fetch_one(&mut *conn)
        .await
}

async fn raise_min_supported_version(
    conn: &mut SqliteConnection,
    version: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE _anlg_schema_compat \
         SET min_supported_version = MAX(min_supported_version, ?1) \
         WHERE id = 0",
    )
    .bind(version)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

fn resolve_migrations(
    schema: DbSchema,
) -> Result<Vec<(&'static MigrationStep, Migration)>, MigrateError> {
    let mut seen_versions = HashMap::new();
    let mut migrations = Vec::with_capacity(schema.steps.len());

    for step in schema.steps {
        validate_step(schema, step)?;

        let (version, description) = parse_step_id(step.id)?;

        if let Some(first_step_id) = seen_versions.insert(version, step.id) {
            return Err(MigrateError::DuplicateStepVersion {
                version,
                first_step_id,
                second_step_id: step.id,
            });
        }

        migrations.push((
            step,
            Migration::new(
                version,
                Cow::Borrowed(description),
                MigrationType::Simple,
                step.sql.into_sql_str(),
                step.sql.starts_with("-- no-transaction"),
            ),
        ));
    }

    migrations.sort_by_key(|(_, migration)| migration.version);
    Ok(migrations)
}

fn validate_step(schema: DbSchema, step: &MigrationStep) -> Result<(), MigrateError> {
    let MigrationScope::CloudsyncAlter { table_name } = step.scope else {
        return Ok(());
    };

    if (schema.validate_cloudsync_table)(table_name) {
        return Ok(());
    }

    Err(MigrateError::InvalidCloudsyncStep {
        step_id: step.id,
        table_name,
    })
}

fn parse_step_id(step_id: &'static str) -> Result<(i64, &'static str), MigrateError> {
    let Some((version, description)) = step_id.split_once('_') else {
        return Err(MigrateError::InvalidStepId { step_id });
    };

    let version = version
        .parse::<i64>()
        .ok()
        .filter(|version| *version > 0)
        .ok_or(MigrateError::InvalidStepId { step_id })?;

    if description.is_empty() {
        return Err(MigrateError::InvalidStepId { step_id });
    }

    Ok((version, description))
}

fn cloudsync_error(err: impl std::error::Error + Send + Sync + 'static) -> SqlxMigrateError {
    SqlxMigrateError::Execute(sqlx::Error::config(err))
}

impl Migrate for DbMigrateConnection<'_> {
    fn create_schema_if_not_exists<'e>(
        &'e mut self,
        schema_name: &'e str,
    ) -> BoxFuture<'e, Result<(), SqlxMigrateError>> {
        <SqliteConnection as Migrate>::create_schema_if_not_exists(&mut *self.conn, schema_name)
    }

    fn ensure_migrations_table<'e>(
        &'e mut self,
        table_name: &'e str,
    ) -> BoxFuture<'e, Result<(), SqlxMigrateError>> {
        <SqliteConnection as Migrate>::ensure_migrations_table(&mut *self.conn, table_name)
    }

    fn dirty_version<'e>(
        &'e mut self,
        table_name: &'e str,
    ) -> BoxFuture<'e, Result<Option<i64>, SqlxMigrateError>> {
        <SqliteConnection as Migrate>::dirty_version(&mut *self.conn, table_name)
    }

    fn list_applied_migrations<'e>(
        &'e mut self,
        table_name: &'e str,
    ) -> BoxFuture<'e, Result<Vec<AppliedMigration>, SqlxMigrateError>> {
        <SqliteConnection as Migrate>::list_applied_migrations(&mut *self.conn, table_name)
    }

    fn lock(&mut self) -> BoxFuture<'_, Result<(), SqlxMigrateError>> {
        <SqliteConnection as Migrate>::lock(&mut *self.conn)
    }

    fn unlock(&mut self) -> BoxFuture<'_, Result<(), SqlxMigrateError>> {
        <SqliteConnection as Migrate>::unlock(&mut *self.conn)
    }

    fn apply<'e>(
        &'e mut self,
        table_name: &'e str,
        migration: &'e Migration,
    ) -> BoxFuture<'e, Result<Duration, SqlxMigrateError>> {
        Box::pin(async move {
            let meta = self
                .meta_by_version
                .get(&migration.version)
                .copied()
                .unwrap_or(StepMeta {
                    scope: MigrationScope::Plain,
                    breaking: false,
                });

            // Raise the floor before the schema change lands: crashing in
            // between locks older builds out of a schema that is still
            // compatible, while the reverse order would let them open one
            // that is not.
            if meta.breaking {
                raise_min_supported_version(&mut self.conn, migration.version)
                    .await
                    .map_err(SqlxMigrateError::from)?;
            }

            match meta.scope {
                MigrationScope::Plain => {
                    <SqliteConnection as Migrate>::apply(&mut *self.conn, table_name, migration)
                        .await
                }
                MigrationScope::CloudsyncAlter {
                    table_name: cs_table,
                } => {
                    let cloudsync_table_enabled = self.db.cloudsync_enabled()
                        && anlg_db_core::cloudsync_is_enabled_on(&mut *self.conn, cs_table)
                            .await
                            .map_err(cloudsync_error)?;

                    if !cloudsync_table_enabled {
                        return <SqliteConnection as Migrate>::apply(
                            &mut *self.conn,
                            table_name,
                            migration,
                        )
                        .await;
                    }

                    let start = Instant::now();

                    // The alter window bypasses sqlx's per-migration transaction, so a
                    // crash mid-way would otherwise leave a half-migrated schema with no
                    // _sqlx_migrations row, and the re-run would fail on already-applied
                    // DDL. Wrap it ourselves unless the step opts out.
                    let wrap_in_transaction = !migration.no_tx;
                    if wrap_in_transaction {
                        sqlx::query("BEGIN IMMEDIATE")
                            .execute(&mut *self.conn)
                            .await
                            .map_err(SqlxMigrateError::from)?;
                    }

                    let result =
                        cloudsync_alter_migration(&mut self.conn, cs_table, migration).await;

                    match result {
                        Ok(()) if wrap_in_transaction => {
                            sqlx::query("COMMIT")
                                .execute(&mut *self.conn)
                                .await
                                .map_err(SqlxMigrateError::from)?;
                        }
                        Ok(()) => {}
                        Err(error) => {
                            if wrap_in_transaction {
                                let _ = sqlx::query("ROLLBACK").execute(&mut *self.conn).await;
                            }
                            return Err(error);
                        }
                    }

                    let elapsed = start.elapsed();
                    update_execution_time(&mut self.conn, migration.version, elapsed).await?;

                    Ok(elapsed)
                }
            }
        })
    }

    fn revert<'e>(
        &'e mut self,
        table_name: &'e str,
        migration: &'e Migration,
    ) -> BoxFuture<'e, Result<Duration, SqlxMigrateError>> {
        <SqliteConnection as Migrate>::revert(&mut *self.conn, table_name, migration)
    }
}

async fn cloudsync_alter_migration(
    conn: &mut SqliteConnection,
    cs_table: &str,
    migration: &Migration,
) -> Result<(), SqlxMigrateError> {
    anlg_db_core::cloudsync_begin_alter_on(&mut *conn, cs_table)
        .await
        .map_err(cloudsync_error)?;

    execute_migration(&mut *conn, migration).await?;

    anlg_db_core::cloudsync_commit_alter_on(&mut *conn, cs_table)
        .await
        .map_err(cloudsync_error)?;

    Ok(())
}

async fn execute_migration(
    conn: &mut SqliteConnection,
    migration: &Migration,
) -> Result<(), SqlxMigrateError> {
    conn.execute(migration.sql.clone())
        .await
        .map_err(|err| SqlxMigrateError::ExecuteMigration(err, migration.version))?;

    sqlx::query(
        r#"
INSERT INTO _sqlx_migrations ( version, description, success, checksum, execution_time )
VALUES ( ?1, ?2, TRUE, ?3, -1 )
        "#,
    )
    .bind(migration.version)
    .bind(&*migration.description)
    .bind(&*migration.checksum)
    .execute(&mut *conn)
    .await?;

    Ok(())
}

async fn update_execution_time(
    conn: &mut SqliteConnection,
    version: i64,
    elapsed: Duration,
) -> Result<(), SqlxMigrateError> {
    #[allow(clippy::cast_possible_truncation)]
    sqlx::query(
        r#"
UPDATE _sqlx_migrations
SET execution_time = ?1
WHERE version = ?2
        "#,
    )
    .bind(elapsed.as_nanos() as i64)
    .bind(version)
    .execute(&mut *conn)
    .await?;

    Ok(())
}
