use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use sqlx::{Row, SqlitePool};

use hypr_db_change::ChangeNotifier;

use crate::DependencyTarget;

#[derive(Debug, thiserror::Error)]
pub enum DependencyResolutionError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error("query has no reactive dependencies")]
    EmptyDependencySet,
    #[error("unsupported reactive dependency: {name} ({kind})")]
    UnsupportedObject { name: String, kind: String },
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CatalogStore {
    state: Arc<tokio::sync::Mutex<Option<SchemaCatalog>>>,
}

#[derive(Clone, Debug)]
pub(crate) struct SchemaCatalog {
    schema_version: i64,
    objects: HashMap<String, SchemaObject>,
    query_objects: HashSet<String>,
    raw_to_targets: HashMap<String, HashSet<DependencyTarget>>,
    target_to_raw: HashMap<DependencyTarget, HashSet<String>>,
}

#[derive(Clone, Debug)]
struct SchemaObject {
    name: String,
    kind: SchemaObjectKind,
}

#[derive(Clone, Debug)]
enum SchemaObjectKind {
    Table,
    View,
    Virtual { module: Option<String> },
    Shadow,
}

impl CatalogStore {
    pub(crate) async fn analyze_query(
        &self,
        pool: &SqlitePool,
        sql: &str,
    ) -> Result<HashSet<DependencyTarget>, DependencyResolutionError> {
        let catalog = self.catalog(pool, false).await?;
        catalog.resolve_query(pool, sql).await
    }

    pub(crate) async fn canonicalize_raw_tables(
        &self,
        pool: &SqlitePool,
        raw_tables: &HashSet<String>,
    ) -> Result<HashSet<DependencyTarget>, sqlx::Error> {
        let mut catalog = self.catalog(pool, false).await?;
        if raw_tables
            .iter()
            .any(|name| !catalog.objects.contains_key(name))
        {
            catalog = self.catalog(pool, true).await?;
        }

        let mut targets = HashSet::new();
        for raw_table in raw_tables {
            if let Some(mapped) = catalog.raw_to_targets.get(raw_table) {
                targets.extend(mapped.iter().cloned());
            }
        }

        Ok(targets)
    }

    pub(crate) async fn latest_dependency_seq(
        &self,
        change_notifier: &ChangeNotifier,
        targets: &HashSet<DependencyTarget>,
        pool: &SqlitePool,
    ) -> Result<Option<u64>, sqlx::Error> {
        let catalog = self.catalog(pool, false).await?;
        let mut latest = None;

        for target in targets {
            if let Some(raw_tables) = catalog.target_to_raw.get(target) {
                for raw_table in raw_tables {
                    if let Some(seq) = change_notifier.latest_table_seq(raw_table) {
                        latest = Some(latest.unwrap_or(seq).max(seq));
                    }
                }
            }
        }

        Ok(latest)
    }

    async fn catalog(
        &self,
        pool: &SqlitePool,
        force_reload: bool,
    ) -> Result<SchemaCatalog, sqlx::Error> {
        let current_schema_version = load_schema_version(pool).await?;

        let mut state = self.state.lock().await;
        let should_reload = force_reload
            || state
                .as_ref()
                .is_none_or(|catalog| catalog.schema_version != current_schema_version);

        if should_reload {
            *state = Some(SchemaCatalog::load(pool, current_schema_version).await?);
        }

        Ok(state.clone().expect("catalog should be populated"))
    }
}

impl SchemaCatalog {
    async fn load(pool: &SqlitePool, schema_version: i64) -> Result<Self, sqlx::Error> {
        let sqlite_master_rows =
            sqlx::query("SELECT type, name, sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'")
                .fetch_all(pool)
                .await?;

        let master_entries: HashMap<String, (String, Option<String>)> = sqlite_master_rows
            .into_iter()
            .map(|row| {
                (
                    row.get::<String, _>("name"),
                    (
                        row.get::<String, _>("type"),
                        row.get::<Option<String>, _>("sql"),
                    ),
                )
            })
            .collect();

        let table_list_rows = sqlx::query("PRAGMA table_list").fetch_all(pool).await?;
        let mut objects = HashMap::new();

        for row in table_list_rows {
            let name = row.get::<String, _>("name");
            if name.starts_with("sqlite_") {
                continue;
            }

            let kind = match row.get::<String, _>("type").as_str() {
                "table" => SchemaObjectKind::Table,
                "view" => SchemaObjectKind::View,
                "virtual" => SchemaObjectKind::Virtual {
                    module: master_entries
                        .get(&name)
                        .and_then(|(_, sql)| sql.as_deref())
                        .and_then(parse_virtual_table_module),
                },
                "shadow" => SchemaObjectKind::Shadow,
                _ => match master_entries.get(&name).map(|(kind, _)| kind.as_str()) {
                    Some("view") => SchemaObjectKind::View,
                    Some("table") => SchemaObjectKind::Table,
                    _ => SchemaObjectKind::Table,
                },
            };

            objects.insert(name.clone(), SchemaObject { name, kind });
        }

        let mut query_objects = HashSet::new();
        let mut raw_to_targets = HashMap::new();
        let mut target_to_raw = HashMap::new();

        for object in objects.values() {
            match &object.kind {
                SchemaObjectKind::Table => {
                    query_objects.insert(object.name.clone());
                    register_target_mapping(
                        &mut raw_to_targets,
                        &mut target_to_raw,
                        object.name.clone(),
                        DependencyTarget::Table(object.name.clone()),
                    );
                }
                SchemaObjectKind::View => {
                    query_objects.insert(object.name.clone());
                }
                SchemaObjectKind::Virtual { module } => {
                    query_objects.insert(object.name.clone());
                    if let Some(target) = supported_virtual_target(&object.name, module.as_deref())
                    {
                        register_target_mapping(
                            &mut raw_to_targets,
                            &mut target_to_raw,
                            object.name.clone(),
                            target,
                        );
                    }
                }
                SchemaObjectKind::Shadow => {}
            }
        }

        for object in objects.values() {
            let SchemaObjectKind::Virtual { module } = &object.kind else {
                continue;
            };
            let Some(target) = supported_virtual_target(&object.name, module.as_deref()) else {
                continue;
            };

            for shadow_table in supported_shadow_tables(&object.name, module.as_deref()) {
                let is_shadow = objects
                    .get(&shadow_table)
                    .is_some_and(|object| matches!(object.kind, SchemaObjectKind::Shadow));
                if is_shadow {
                    register_target_mapping(
                        &mut raw_to_targets,
                        &mut target_to_raw,
                        shadow_table,
                        target.clone(),
                    );
                }
            }
        }

        Ok(Self {
            schema_version,
            objects,
            query_objects,
            raw_to_targets,
            target_to_raw,
        })
    }

    pub(crate) async fn resolve_query(
        &self,
        pool: &SqlitePool,
        sql: &str,
    ) -> Result<HashSet<DependencyTarget>, DependencyResolutionError> {
        let alias_map = super::explain::build_alias_map(sql, &self.query_objects);
        let eqp_rows = sqlx::query(sqlx::AssertSqlSafe(format!("EXPLAIN QUERY PLAN {sql}")))
            .fetch_all(pool)
            .await?;

        let mut targets = HashSet::new();

        for row in &eqp_rows {
            let detail: &str = row.get("detail");
            let Some(name) = super::explain::parse_table_from_detail(detail) else {
                continue;
            };
            let normalized_name = super::explain::normalize_identifier(name);
            let schema_name = if self.query_objects.contains(&normalized_name) {
                normalized_name
            } else if let Some(mapped) = alias_map.get(&normalized_name) {
                mapped.clone()
            } else {
                continue;
            };

            let target = self.resolve_query_object(&schema_name)?;
            targets.insert(target);
        }

        if targets.is_empty() {
            return Err(DependencyResolutionError::EmptyDependencySet);
        }

        Ok(targets)
    }

    fn resolve_query_object(
        &self,
        name: &str,
    ) -> Result<DependencyTarget, DependencyResolutionError> {
        let object =
            self.objects
                .get(name)
                .ok_or_else(|| DependencyResolutionError::UnsupportedObject {
                    name: name.to_string(),
                    kind: "unknown object".to_string(),
                })?;

        match &object.kind {
            SchemaObjectKind::Table => Ok(DependencyTarget::Table(name.to_string())),
            SchemaObjectKind::Virtual { module } => {
                supported_virtual_target(name, module.as_deref()).ok_or_else(|| {
                    DependencyResolutionError::UnsupportedObject {
                        name: name.to_string(),
                        kind: match module {
                            Some(module) => format!("virtual table module `{module}`"),
                            None => "virtual table with unknown module".to_string(),
                        },
                    }
                })
            }
            SchemaObjectKind::View => Err(DependencyResolutionError::UnsupportedObject {
                name: name.to_string(),
                kind: "view expansion missing from query plan".to_string(),
            }),
            SchemaObjectKind::Shadow => Err(DependencyResolutionError::UnsupportedObject {
                name: name.to_string(),
                kind: "shadow table".to_string(),
            }),
        }
    }
}

fn register_target_mapping(
    raw_to_targets: &mut HashMap<String, HashSet<DependencyTarget>>,
    target_to_raw: &mut HashMap<DependencyTarget, HashSet<String>>,
    raw_table: String,
    target: DependencyTarget,
) {
    raw_to_targets
        .entry(raw_table.clone())
        .or_default()
        .insert(target.clone());
    target_to_raw.entry(target).or_default().insert(raw_table);
}

fn supported_virtual_target(name: &str, module: Option<&str>) -> Option<DependencyTarget> {
    match module.map(|module| module.to_ascii_lowercase()) {
        Some(module) if module == "fts5" => Some(DependencyTarget::VirtualTable(name.to_string())),
        _ => None,
    }
}

fn supported_shadow_tables(name: &str, module: Option<&str>) -> Vec<String> {
    match module.map(|module| module.to_ascii_lowercase()) {
        Some(module) if module == "fts5" => ["_config", "_content", "_data", "_docsize", "_idx"]
            .into_iter()
            .map(|suffix| format!("{name}{suffix}"))
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_virtual_table_module(sql: &str) -> Option<String> {
    let upper = sql.to_ascii_uppercase();
    let using_index = upper.find(" USING ")?;
    let after_using = sql[using_index + 7..].trim_start();
    let end_index = after_using
        .find(|ch: char| ch.is_whitespace() || ch == '(')
        .unwrap_or(after_using.len());
    let module = crate::explain::strip_identifier_quotes(&after_using[..end_index]).trim();
    (!module.is_empty()).then(|| module.to_ascii_lowercase())
}

async fn load_schema_version(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("PRAGMA schema_version")
        .fetch_one(pool)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_fts_db() -> hypr_db_core::Db {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        sqlx::query("CREATE VIRTUAL TABLE docs_fts USING fts5(title, body)")
            .execute(db.pool())
            .await
            .unwrap();
        db
    }

    #[tokio::test]
    async fn fts5_catalog_discovers_virtual_and_shadow_tables() {
        let db = setup_fts_db().await;
        let catalog = SchemaCatalog::load(db.pool(), load_schema_version(db.pool()).await.unwrap())
            .await
            .unwrap();

        assert!(matches!(
            catalog.objects.get("docs_fts").map(|object| &object.kind),
            Some(SchemaObjectKind::Virtual { module: Some(module) }) if module == "fts5"
        ));
        assert!(matches!(
            catalog
                .objects
                .get("docs_fts_data")
                .map(|object| &object.kind),
            Some(SchemaObjectKind::Shadow)
        ));
        assert!(
            catalog
                .raw_to_targets
                .get("docs_fts_data")
                .is_some_and(|targets| targets
                    .contains(&DependencyTarget::VirtualTable("docs_fts".to_string())))
        );
    }

    #[tokio::test]
    async fn unsupported_virtual_modules_are_not_reactive() {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        sqlx::query("CREATE VIRTUAL TABLE docs_rtree USING rtree(id, min_x, max_x)")
            .execute(db.pool())
            .await
            .unwrap();

        let catalog = SchemaCatalog::load(db.pool(), load_schema_version(db.pool()).await.unwrap())
            .await
            .unwrap();

        assert!(matches!(
            catalog.resolve_query_object("docs_rtree"),
            Err(DependencyResolutionError::UnsupportedObject { .. })
        ));
    }

    #[tokio::test]
    async fn canonicalize_raw_tables_reloads_after_schema_changes() {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        sqlx::query("CREATE TABLE existing_notes (id TEXT PRIMARY KEY NOT NULL)")
            .execute(db.pool())
            .await
            .unwrap();

        let store = CatalogStore::default();
        let targets = store
            .analyze_query(db.pool(), "SELECT id FROM existing_notes")
            .await
            .unwrap();
        assert_eq!(
            targets,
            HashSet::from([DependencyTarget::Table("existing_notes".to_string())])
        );

        sqlx::query("CREATE TABLE added_later (id TEXT PRIMARY KEY NOT NULL)")
            .execute(db.pool())
            .await
            .unwrap();

        let targets = store
            .canonicalize_raw_tables(db.pool(), &HashSet::from(["added_later".to_string()]))
            .await
            .unwrap();

        assert_eq!(
            targets,
            HashSet::from([DependencyTarget::Table("added_later".to_string())])
        );
    }
}
