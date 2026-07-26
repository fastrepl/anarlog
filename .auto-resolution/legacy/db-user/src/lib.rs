mod calendars_ops;
mod calendars_types;
mod chat_conversations_ops;
mod chat_conversations_types;
mod chat_groups_ops;
mod chat_groups_types;
mod chat_messages_ops;
mod chat_messages_types;
mod chat_messages_v2_ops;
mod chat_messages_v2_types;
mod config_ops;
mod config_types;
mod events_ops;
mod events_types;
mod extensions_ops;
mod extensions_types;
mod humans_ops;
mod humans_types;
mod organizations_ops;
mod organizations_types;
mod sessions_ops;
mod sessions_types;
mod tags_ops;
mod tags_types;
mod templates_ops;
mod templates_types;

#[allow(unused)]
pub use calendars_ops::*;
#[allow(unused)]
pub use calendars_types::*;
#[allow(unused)]
pub use chat_conversations_ops::*;
#[allow(unused)]
pub use chat_conversations_types::*;
#[allow(unused)]
pub use chat_groups_ops::*;
#[allow(unused)]
pub use chat_groups_types::*;
#[allow(unused)]
pub use chat_messages_ops::*;
#[allow(unused)]
pub use chat_messages_types::*;
#[allow(unused)]
pub use chat_messages_v2_ops::*;
#[allow(unused)]
pub use chat_messages_v2_types::*;
#[allow(unused)]
pub use config_ops::*;
#[allow(unused)]
pub use config_types::*;
#[allow(unused)]
pub use events_ops::*;
#[allow(unused)]
pub use events_types::*;
#[allow(unused)]
pub use extensions_ops::*;
#[allow(unused)]
pub use extensions_types::*;
#[allow(unused)]
pub use humans_ops::*;
#[allow(unused)]
pub use humans_types::*;
#[allow(unused)]
pub use organizations_ops::*;
#[allow(unused)]
pub use organizations_types::*;
#[allow(unused)]
pub use sessions_ops::*;
#[allow(unused)]
pub use sessions_types::*;
#[allow(unused)]
pub use tags_ops::*;
#[allow(unused)]
pub use tags_types::*;
#[allow(unused)]
pub use templates_ops::*;
#[allow(unused)]
pub use templates_types::*;

pub use legacy_db_core::{Database, Error};

#[macro_export]
macro_rules! user_common_derives {
    (#[sql_table($table:expr)] $(#[$meta:meta])* $vis:vis $kind:ident $name:ident {
        $($body:tt)*
    }) => {
        #[derive(
            Debug,
            PartialEq,
            Clone,
            serde::Serialize,
            serde::Deserialize,
            specta::Type,
            schemars::JsonSchema,
        )]
        $(#[$meta])* $vis $kind $name {
            $($body)*
        }

        impl legacy_db_core::SqlTable for $name {
            fn sql_table() -> &'static str {
                $table
            }
        }
    };

    ($item:item) => {
        #[derive(
            Debug,
            PartialEq,
            Clone,
            serde::Serialize,
            serde::Deserialize,
            specta::Type,
            schemars::JsonSchema,
        )]
        $item
    };
}

#[derive(Clone)]
pub struct UserDatabase {
    db: legacy_db_core::Database,
}

impl UserDatabase {
    pub fn from(db: legacy_db_core::Database) -> Self {
        Self { db }
    }
}

impl std::ops::Deref for UserDatabase {
    type Target = legacy_db_core::Database;

    fn deref(&self) -> &Self::Target {
        &self.db
    }
}

// Append only. Do not reorder.
const MIGRATIONS: [&str; 27] = [
    include_str!("./calendars_migration.sql"),
    include_str!("./configs_migration.sql"),
    include_str!("./events_migration.sql"),
    include_str!("./humans_migration.sql"),
    include_str!("./organizations_migration.sql"),
    include_str!("./sessions_migration.sql"),
    include_str!("./session_participants_migration.sql"),
    include_str!("./templates_migration.sql"),
    include_str!("./chat_groups_migration.sql"),
    include_str!("./chat_messages_migration.sql"),
    include_str!("./extension_mappings_migration.sql"),
    include_str!("./tags_migration.sql"),
    include_str!("./tag_sessions_migration.sql"),
    include_str!("./calendars_migration_1.sql"),
    include_str!("./sessions_migration_1.sql"),
    include_str!("./sessions_migration_2.sql"),
    include_str!("./sessions_migration_3.sql"),
    include_str!("./sessions_migration_4.sql"),
    include_str!("./chat_groups_migration_1.sql"),
    include_str!("./events_migration_1.sql"),
    include_str!("./session_participants_migration_1.sql"),
    include_str!("./events_migration_2.sql"),
    include_str!("./chat_messages_migration_1.sql"),
    include_str!("./chat_messages_migration_2.sql"),
    include_str!("./templates_migration_1.sql"),
    include_str!("./chat_conversations_migration.sql"),
    include_str!("./chat_messages_v2_migration.sql"),
];

pub async fn migrate(db: &UserDatabase) -> Result<(), crate::Error> {
    let conn = db.conn()?;
    legacy_db_core::migrate(&conn, MIGRATIONS.to_vec()).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::UserDatabase;
    use crate::migrate;
    use legacy_db_core::DatabaseBuilder;

    pub async fn setup_db() -> UserDatabase {
        let base_db = DatabaseBuilder::default().memory().build().await.unwrap();
        let user_db = UserDatabase::from(base_db);
        migrate(&user_db).await.unwrap();
        user_db
    }

    #[tokio::test]
    async fn test_migrate() {
        let _ = setup_db().await;
    }
}
