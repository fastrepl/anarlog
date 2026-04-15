use sha2::{Digest, Sha384};

use hypr_db_core2::Db3;

use crate::error::MigrateError;
use crate::schema::{DbSchema, MigrationScope, MigrationStep};

fn compute_checksum(sql: &str) -> String {
    let hash = Sha384::digest(sql.as_bytes());
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) async fn run_migrations(db: &Db3, schema: DbSchema) -> Result<(), MigrateError> {
    ensure_migrations_table(db.pool().as_ref()).await?;
    run_migration_steps(db, schema).await?;
    Ok(())
}

async fn ensure_migrations_table(pool: &sqlx::SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS _char_migrations (
            id TEXT PRIMARY KEY NOT NULL,
            checksum TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
        )",
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn run_migration_steps(db: &Db3, schema: DbSchema) -> Result<(), MigrateError> {
    for step in schema.steps {
        validate_step(schema, step)?;

        let checksum = compute_checksum(step.sql);

        let applied_checksum: Option<String> =
            sqlx::query_scalar("SELECT checksum FROM _char_migrations WHERE id = ?")
                .bind(step.id)
                .fetch_optional(db.pool().as_ref())
                .await?;

        if let Some(applied_checksum) = applied_checksum {
            if applied_checksum != checksum {
                return Err(MigrateError::StepChecksumMismatch { step_id: step.id });
            }
            continue;
        }

        match step.scope {
            MigrationScope::Plain => run_plain_step(db.pool().as_ref(), step, &checksum).await?,
            MigrationScope::CloudsyncAlter { table_name } => {
                run_cloudsync_step(db, step, table_name, &checksum).await?
            }
        }
    }

    Ok(())
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

async fn run_plain_step(
    pool: &sqlx::SqlitePool,
    step: &MigrationStep,
    checksum: &str,
) -> Result<(), MigrateError> {
    let mut tx = pool.begin().await?;
    sqlx::raw_sql(step.sql).execute(&mut *tx).await?;
    record_step(&mut *tx, step, checksum).await?;
    tx.commit().await?;
    Ok(())
}

async fn run_cloudsync_step(
    db: &Db3,
    step: &MigrationStep,
    table_name: &'static str,
    checksum: &str,
) -> Result<(), MigrateError> {
    if db.cloudsync_enabled() {
        let mut conn = db.pool().acquire().await?;
        hypr_db_core2::cloudsync_begin_alter_on(&mut *conn, table_name).await?;
        sqlx::raw_sql(step.sql).execute(&mut *conn).await?;
        hypr_db_core2::cloudsync_commit_alter_on(&mut *conn, table_name).await?;
        record_step(&mut *conn, step, checksum).await?;
    } else {
        let mut tx = db.pool().begin().await?;
        sqlx::raw_sql(step.sql).execute(&mut *tx).await?;
        record_step(&mut *tx, step, checksum).await?;
        tx.commit().await?;
    }
    Ok(())
}

async fn record_step<'e, E>(
    executor: E,
    step: &MigrationStep,
    checksum: &str,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("INSERT INTO _char_migrations (id, checksum) VALUES (?, ?)")
        .bind(step.id)
        .bind(checksum)
        .execute(executor)
        .await?;
    Ok(())
}
