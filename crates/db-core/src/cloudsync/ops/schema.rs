use sqlx::{Executor, Sqlite};

use anlg_cloudsync::{OwnedSqliteConnection, ReservedConnection};

use super::super::{CloudsyncInterruptHandle, CloudsyncTableSpec};

pub(crate) async fn interruptible_cleanup<C: OwnedSqliteConnection>(
    connection: &mut ReservedConnection<C>,
    table_name: &str,
    interrupt: &CloudsyncInterruptHandle,
) -> Result<(), anlg_cloudsync::Error> {
    let was_enabled =
        anlg_cloudsync::is_enabled(connection.connection().await?, table_name).await?;
    sqlx::query("SAVEPOINT cloudsync_cleanup")
        .execute(connection.connection().await?)
        .await?;
    let registration = match interrupt.register(connection.connection().await?).await {
        Ok(registration) => registration,
        Err(error) => {
            rollback_cleanup_savepoint(connection, table_name, was_enabled).await?;
            return Err(error.into());
        }
    };
    let result = anlg_cloudsync::cleanup_on_connection(connection, table_name).await;
    let finish_result: Result<(), anlg_cloudsync::Error> = match connection.connection().await {
        Ok(connection) => registration.finish(connection).await.map_err(Into::into),
        Err(error) => Err(error),
    };
    if let Err(error) = finish_result {
        rollback_cleanup_savepoint(connection, table_name, was_enabled).await?;
        return Err(error);
    }

    match result {
        Ok(()) => match sqlx::query("RELEASE cloudsync_cleanup")
            .execute(connection.connection().await?)
            .await
        {
            Ok(_) => Ok(()),
            Err(error) => {
                rollback_cleanup_savepoint(connection, table_name, was_enabled).await?;
                Err(error.into())
            }
        },
        Err(error) => {
            rollback_cleanup_savepoint(connection, table_name, was_enabled).await?;
            Err(error)
        }
    }
}

async fn rollback_cleanup_savepoint<C: OwnedSqliteConnection>(
    connection: &mut ReservedConnection<C>,
    table_name: &str,
    was_enabled: bool,
) -> Result<(), anlg_cloudsync::Error> {
    sqlx::raw_sql("ROLLBACK TO cloudsync_cleanup; RELEASE cloudsync_cleanup")
        .execute(connection.connection().await?)
        .await?;
    let has_settings: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM sqlite_schema WHERE name = 'cloudsync_table_settings')",
    )
    .fetch_one(connection.connection().await?)
    .await?;
    if has_settings {
        let algo: Option<String> = sqlx::query_scalar(
            "SELECT value FROM cloudsync_table_settings WHERE tbl_name = ? AND col_name = '*' AND key = 'algo'",
        )
        .bind(table_name)
        .fetch_optional(connection.connection().await?)
        .await?
        .flatten();
        if let Some(algo) = algo {
            // SQL rollback does not restore sqlite-sync's removed in-memory table.
            // The persisted table already passed validation, possibly with integer-key,
            // nullable-key, or missing-default exemptions (flags 1 | 2 | 4).
            anlg_cloudsync::init_on_connection(connection, table_name, Some(&algo), Some(7))
                .await?;
            if !was_enabled {
                anlg_cloudsync::disable(connection.connection().await?, table_name).await?;
            }
        }
    }
    Ok(())
}

pub(crate) async fn interruptible_init<C: OwnedSqliteConnection>(
    connection: &mut ReservedConnection<C>,
    table_name: &str,
    crdt_algo: Option<&str>,
    init_flags: Option<i64>,
    interrupt: &CloudsyncInterruptHandle,
) -> Result<(), anlg_cloudsync::Error> {
    let registration = interrupt.register(connection.connection().await?).await?;
    let result =
        anlg_cloudsync::init_on_connection(connection, table_name, crdt_algo, init_flags).await;
    registration.finish(connection.connection().await?).await?;
    result
}

pub(super) async fn init_enabled_tables<C: OwnedSqliteConnection>(
    connection: &mut ReservedConnection<C>,
    tables: &[CloudsyncTableSpec],
    interrupt: &CloudsyncInterruptHandle,
) -> Result<(), anlg_cloudsync::Error> {
    for table in tables.iter().filter(|table| table.enabled) {
        interruptible_init(
            connection,
            &table.table_name,
            table.crdt_algo.as_deref(),
            table.init_flags,
            interrupt,
        )
        .await?;
    }

    Ok(())
}

pub async fn cloudsync_begin_alter_on<'e, E>(
    executor: E,
    table_name: &str,
) -> Result<(), anlg_cloudsync::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    anlg_cloudsync::begin_alter(executor, table_name).await
}

pub async fn cloudsync_is_enabled_on<'e, E>(
    executor: E,
    table_name: &str,
) -> Result<bool, anlg_cloudsync::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    anlg_cloudsync::is_enabled(executor, table_name).await
}

pub(crate) async fn cloudsync_has_local_unsent_changes_on<'e, E>(
    executor: E,
) -> Result<bool, anlg_cloudsync::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1
            FROM cloudsync_changes
            WHERE site_id = (
                SELECT site_id
                FROM cloudsync_site_id
                WHERE rowid = 0
            )
              AND db_version > COALESCE(
                (
                    SELECT CAST(value AS INTEGER)
                    FROM cloudsync_settings
                    WHERE key = 'send_dbversion'
                ),
                0
              )
            LIMIT 1
        )",
    )
    .fetch_one(executor)
    .await?)
}

pub async fn cloudsync_commit_alter_on<'e, E>(
    executor: E,
    table_name: &str,
) -> Result<(), anlg_cloudsync::Error>
where
    E: Executor<'e, Database = Sqlite>,
{
    anlg_cloudsync::commit_alter(executor, table_name).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cleanup_rollback_restores_native_tracking_and_sync_identity() {
        let db = crate::Db::connect_memory().await.unwrap();
        sqlx::query("CREATE TABLE items (id INTEGER PRIMARY KEY, value TEXT)")
            .execute(db.pool())
            .await
            .unwrap();
        db.cloudsync_init("items", Some("gos"), Some(7))
            .await
            .unwrap();
        let mut connection = ReservedConnection::new(db.pool().acquire().await.unwrap());
        sqlx::query("INSERT INTO items VALUES (1, 'before')")
            .execute(connection.connection().await.unwrap())
            .await
            .unwrap();
        let site_id = anlg_cloudsync::siteid(connection.connection().await.unwrap())
            .await
            .unwrap();
        sqlx::query("SELECT cloudsync_set('send_dbversion', '123')")
            .execute(connection.connection().await.unwrap())
            .await
            .unwrap();
        sqlx::query("SAVEPOINT cloudsync_cleanup")
            .execute(connection.connection().await.unwrap())
            .await
            .unwrap();
        anlg_cloudsync::cleanup_on_connection(&mut connection, "items")
            .await
            .unwrap();
        rollback_cleanup_savepoint(&mut connection, "items", true)
            .await
            .unwrap();
        sqlx::query("INSERT INTO items VALUES (2, 'after')")
            .execute(connection.connection().await.unwrap())
            .await
            .unwrap();
        assert_eq!(
            anlg_cloudsync::siteid(connection.connection().await.unwrap())
                .await
                .unwrap(),
            site_id
        );
        let cursor: String =
            sqlx::query_scalar("SELECT value FROM cloudsync_settings WHERE key = 'send_dbversion'")
                .fetch_one(connection.connection().await.unwrap())
                .await
                .unwrap();
        assert_eq!(cursor, "123");
        assert!(
            anlg_cloudsync::is_enabled(connection.connection().await.unwrap(), "items")
                .await
                .unwrap()
        );
        anlg_cloudsync::disable(connection.connection().await.unwrap(), "items")
            .await
            .unwrap();
        sqlx::query("SAVEPOINT cloudsync_cleanup")
            .execute(connection.connection().await.unwrap())
            .await
            .unwrap();
        anlg_cloudsync::cleanup_on_connection(&mut connection, "items")
            .await
            .unwrap();
        rollback_cleanup_savepoint(&mut connection, "items", false)
            .await
            .unwrap();
        assert!(
            !anlg_cloudsync::is_enabled(connection.connection().await.unwrap(), "items")
                .await
                .unwrap()
        );
        sqlx::query("INSERT INTO items VALUES (3, 'disabled')")
            .execute(connection.connection().await.unwrap())
            .await
            .unwrap();
    }
}
