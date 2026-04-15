use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

use hypr_db_core2::Db3;
use sqlx::migrate::{
    AppliedMigration, Migrate, MigrateError as SqlxMigrateError, Migration, MigrationType,
};
use sqlx::{Executor, Sqlite, SqliteConnection};

use crate::error::MigrateError;
use crate::schema::{DbSchema, MigrationScope, MigrationStep};

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

struct DbMigrateConnection<'a> {
    db: &'a Db3,
    conn: sqlx::pool::PoolConnection<Sqlite>,
    scopes_by_version: HashMap<i64, MigrationScope>,
}

impl<'a> DbMigrateConnection<'a> {
    fn new(
        db: &'a Db3,
        conn: sqlx::pool::PoolConnection<Sqlite>,
        scopes_by_version: HashMap<i64, MigrationScope>,
    ) -> Self {
        Self {
            db,
            conn,
            scopes_by_version,
        }
    }
}

pub(crate) async fn run_migrations(db: &Db3, schema: DbSchema) -> Result<(), MigrateError> {
    let resolved = resolve_migrations(schema)?;
    let scopes_by_version = resolved
        .iter()
        .map(|(step, migration)| (migration.version, step.scope))
        .collect();
    let migrations: Vec<_> = resolved
        .into_iter()
        .map(|(_, migration)| migration)
        .collect();

    let conn = db.pool().acquire().await?;
    let mut conn = DbMigrateConnection::new(db, conn, scopes_by_version);
    run_direct(&migrations, &mut conn).await?;
    Ok(())
}

async fn run_direct<C>(migrations: &[Migration], conn: &mut C) -> Result<(), SqlxMigrateError>
where
    C: Migrate,
{
    conn.lock().await?;
    conn.ensure_migrations_table().await?;

    if let Some(version) = conn.dirty_version().await? {
        return Err(SqlxMigrateError::Dirty(version));
    }

    let applied_migrations = conn.list_applied_migrations().await?;
    validate_applied_migrations(&applied_migrations, migrations)?;

    let applied_migrations: HashMap<_, _> = applied_migrations
        .into_iter()
        .map(|migration| (migration.version, migration))
        .collect();

    for migration in migrations {
        if migration.migration_type.is_down_migration() {
            continue;
        }

        match applied_migrations.get(&migration.version) {
            Some(applied_migration) => {
                if migration.checksum != applied_migration.checksum {
                    return Err(SqlxMigrateError::VersionMismatch(migration.version));
                }
            }
            None => {
                conn.apply(migration).await?;
            }
        }
    }

    conn.unlock().await?;
    Ok(())
}

fn validate_applied_migrations(
    applied_migrations: &[AppliedMigration],
    migrations: &[Migration],
) -> Result<(), SqlxMigrateError> {
    let versions: HashSet<_> = migrations
        .iter()
        .map(|migration| migration.version)
        .collect();

    for applied_migration in applied_migrations {
        if !versions.contains(&applied_migration.version) {
            return Err(SqlxMigrateError::VersionMissing(applied_migration.version));
        }
    }

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
                Cow::Borrowed(step.sql),
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
    fn ensure_migrations_table(&mut self) -> BoxFuture<'_, Result<(), SqlxMigrateError>> {
        <SqliteConnection as Migrate>::ensure_migrations_table(&mut *self.conn)
    }

    fn dirty_version(&mut self) -> BoxFuture<'_, Result<Option<i64>, SqlxMigrateError>> {
        <SqliteConnection as Migrate>::dirty_version(&mut *self.conn)
    }

    fn list_applied_migrations(
        &mut self,
    ) -> BoxFuture<'_, Result<Vec<AppliedMigration>, SqlxMigrateError>> {
        <SqliteConnection as Migrate>::list_applied_migrations(&mut *self.conn)
    }

    fn lock(&mut self) -> BoxFuture<'_, Result<(), SqlxMigrateError>> {
        <SqliteConnection as Migrate>::lock(&mut *self.conn)
    }

    fn unlock(&mut self) -> BoxFuture<'_, Result<(), SqlxMigrateError>> {
        <SqliteConnection as Migrate>::unlock(&mut *self.conn)
    }

    fn apply<'e: 'm, 'm>(
        &'e mut self,
        migration: &'m Migration,
    ) -> BoxFuture<'m, Result<Duration, SqlxMigrateError>> {
        Box::pin(async move {
            let scope = self
                .scopes_by_version
                .get(&migration.version)
                .copied()
                .unwrap_or(MigrationScope::Plain);

            match scope {
                MigrationScope::Plain => {
                    <SqliteConnection as Migrate>::apply(&mut *self.conn, migration).await
                }
                MigrationScope::CloudsyncAlter { table_name } => {
                    if !self.db.cloudsync_enabled() {
                        return <SqliteConnection as Migrate>::apply(&mut *self.conn, migration)
                            .await;
                    }

                    let start = Instant::now();

                    hypr_db_core2::cloudsync_begin_alter_on(&mut *self.conn, table_name)
                        .await
                        .map_err(cloudsync_error)?;

                    execute_migration(&mut *self.conn, migration).await?;

                    hypr_db_core2::cloudsync_commit_alter_on(&mut *self.conn, table_name)
                        .await
                        .map_err(cloudsync_error)?;

                    let elapsed = start.elapsed();
                    update_execution_time(&mut *self.conn, migration.version, elapsed).await?;

                    Ok(elapsed)
                }
            }
        })
    }

    fn revert<'e: 'm, 'm>(
        &'e mut self,
        migration: &'m Migration,
    ) -> BoxFuture<'m, Result<Duration, SqlxMigrateError>> {
        <SqliteConnection as Migrate>::revert(&mut *self.conn, migration)
    }
}

async fn execute_migration(
    conn: &mut SqliteConnection,
    migration: &Migration,
) -> Result<(), SqlxMigrateError> {
    conn.execute(&*migration.sql)
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
