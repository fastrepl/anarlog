use std::collections::{HashMap, HashSet};

use sqlx::SqlitePool;

use crate::DependencyTarget;
use crate::schema::{CatalogStore, DependencyResolutionError};

/// # Safety
///
/// `sql` is interpolated into `format!("EXPLAIN QUERY PLAN {sql}")` and executed directly.
/// Only pass SQL from trusted code, never user input.
pub async fn extract_dependencies(
    pool: &SqlitePool,
    sql: &str,
) -> Result<HashSet<DependencyTarget>, DependencyResolutionError> {
    CatalogStore::default().analyze_query(pool, sql).await
}

pub(crate) fn parse_table_from_detail(detail: &str) -> Option<&str> {
    let trimmed = detail.trim();
    let rest = trimmed
        .strip_prefix("SCAN ")
        .or_else(|| trimmed.strip_prefix("SEARCH "))?;
    rest.split_whitespace().next()
}

pub(crate) fn normalize_identifier(token: &str) -> String {
    let token = token.trim_matches(|c: char| matches!(c, ',' | ')' | ';' | '('));
    let token = token.rsplit('.').next().unwrap_or(token);
    strip_identifier_quotes(token).to_string()
}

pub(crate) fn strip_identifier_quotes(token: &str) -> &str {
    if token.len() >= 2 {
        if (token.starts_with('"') && token.ends_with('"'))
            || (token.starts_with('`') && token.ends_with('`'))
            || (token.starts_with('[') && token.ends_with(']'))
        {
            return &token[1..token.len() - 1];
        }
    }

    token
}

pub(crate) fn build_alias_map(
    sql: &str,
    known_objects: &HashSet<String>,
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let upper = sql.to_uppercase();
    let tokens: Vec<&str> = sql.split_whitespace().collect();
    let upper_tokens: Vec<&str> = upper.split_whitespace().collect();

    for i in 0..tokens.len() {
        let Some(table_idx) = table_reference_index(&upper_tokens, i) else {
            continue;
        };

        if table_idx >= tokens.len() {
            continue;
        }

        let raw_object = normalize_identifier(tokens[table_idx]);
        if !known_objects.contains(&raw_object) {
            continue;
        }

        let alias_idx = alias_token_index(&upper_tokens, table_idx);

        if alias_idx < tokens.len() {
            let alias = normalize_identifier(tokens[alias_idx]);
            if !alias.is_empty() && !is_alias_stop_word(&alias) && !known_objects.contains(&alias) {
                map.insert(alias, raw_object.clone());
            }
        }
    }
    map
}

fn table_reference_index(upper_tokens: &[&str], index: usize) -> Option<usize> {
    if index >= upper_tokens.len() {
        return None;
    }

    if matches!(upper_tokens[index], "FROM" | "JOIN") {
        return Some(index + 1);
    }

    if matches!(upper_tokens[index], "INNER" | "LEFT" | "RIGHT" | "CROSS")
        && upper_tokens.get(index + 1) == Some(&"JOIN")
    {
        return Some(index + 2);
    }

    None
}

fn alias_token_index(upper_tokens: &[&str], table_idx: usize) -> usize {
    if upper_tokens.get(table_idx + 1) == Some(&"AS") {
        table_idx + 2
    } else {
        table_idx + 1
    }
}

fn is_alias_stop_word(alias: &str) -> bool {
    matches!(
        alias.to_uppercase().as_str(),
        "ON" | "WHERE"
            | "SET"
            | "JOIN"
            | "INNER"
            | "LEFT"
            | "RIGHT"
            | "CROSS"
            | "ORDER"
            | "GROUP"
            | "HAVING"
            | "LIMIT"
            | "UNION"
            | "EXCEPT"
            | "INTERSECT"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIVE_QUERY_TEST_MIGRATION_STEPS: &[hypr_db_migrate::MigrationStep] =
        &[hypr_db_migrate::MigrationStep {
            id: "20260415000000_live_query_test_schema",
            scope: hypr_db_migrate::MigrationScope::Plain,
            sql: include_str!("../tests/common/live_query_test_schema.sql"),
        }];

    fn live_query_test_schema() -> hypr_db_migrate::DbSchema {
        hypr_db_migrate::DbSchema {
            steps: LIVE_QUERY_TEST_MIGRATION_STEPS,
            validate_cloudsync_table: |_| false,
        }
    }

    async fn test_db() -> hypr_db_core::Db {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        hypr_db_migrate::migrate(&db, live_query_test_schema())
            .await
            .unwrap();
        db
    }

    #[tokio::test]
    async fn single_table() {
        let db = test_db().await;
        let targets = extract_dependencies(db.pool(), "SELECT id FROM daily_notes WHERE id = ?")
            .await
            .unwrap();
        assert_eq!(
            targets,
            HashSet::from([DependencyTarget::Table("daily_notes".to_string())])
        );
    }

    #[tokio::test]
    async fn join_query() {
        let db = test_db().await;
        let targets = extract_dependencies(
            db.pool(),
            "SELECT ds.id FROM daily_summaries ds JOIN daily_notes dn ON ds.daily_note_id = dn.id",
        )
        .await
        .unwrap();
        assert!(targets.contains(&DependencyTarget::Table("daily_summaries".to_string())));
        assert!(targets.contains(&DependencyTarget::Table("daily_notes".to_string())));
        assert_eq!(targets.len(), 2);
    }

    #[tokio::test]
    async fn alias_query() {
        let db = test_db().await;
        let targets = extract_dependencies(
            db.pool(),
            "SELECT dn.id FROM daily_notes AS dn WHERE dn.date = '2026-04-11'",
        )
        .await
        .unwrap();
        assert_eq!(
            targets,
            HashSet::from([DependencyTarget::Table("daily_notes".to_string())])
        );
    }

    #[tokio::test]
    async fn subquery() {
        let db = test_db().await;
        let targets = extract_dependencies(
            db.pool(),
            "SELECT id FROM daily_notes \
             WHERE EXISTS ( \
               SELECT 1 FROM daily_summaries \
               WHERE daily_summaries.daily_note_id = daily_notes.id \
             )",
        )
        .await
        .unwrap();
        assert!(targets.contains(&DependencyTarget::Table("daily_notes".to_string())));
        assert!(targets.contains(&DependencyTarget::Table("daily_summaries".to_string())));
        assert_eq!(targets.len(), 2);
    }

    #[tokio::test]
    async fn quoted_alias_query() {
        let db = test_db().await;
        let targets = extract_dependencies(
            db.pool(),
            r#"SELECT "dn".id FROM "daily_notes" AS "dn" WHERE "dn".date = '2026-04-11'"#,
        )
        .await
        .unwrap();
        assert_eq!(
            targets,
            HashSet::from([DependencyTarget::Table("daily_notes".to_string())])
        );
    }

    #[tokio::test]
    async fn bracket_quoted_alias_query() {
        let db = test_db().await;
        let targets = extract_dependencies(
            db.pool(),
            "SELECT [dn].id FROM [daily_notes] AS [dn] WHERE [dn].date = '2026-04-11'",
        )
        .await
        .unwrap();
        assert_eq!(
            targets,
            HashSet::from([DependencyTarget::Table("daily_notes".to_string())])
        );
    }

    #[tokio::test]
    async fn schema_qualified_query() {
        let db = test_db().await;
        let targets = extract_dependencies(
            db.pool(),
            "SELECT dn.id FROM main.daily_notes dn WHERE dn.date = '2026-04-11'",
        )
        .await
        .unwrap();
        assert_eq!(
            targets,
            HashSet::from([DependencyTarget::Table("daily_notes".to_string())])
        );
    }

    #[tokio::test]
    async fn view_query_resolves_to_base_table() {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        sqlx::query("CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT NOT NULL)")
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query("CREATE VIEW notes_view AS SELECT id, body FROM notes")
            .execute(db.pool())
            .await
            .unwrap();

        let targets = extract_dependencies(
            db.pool(),
            "SELECT id FROM notes_view WHERE body IS NOT NULL",
        )
        .await
        .unwrap();

        assert_eq!(
            targets,
            HashSet::from([DependencyTarget::Table("notes".to_string())])
        );
    }

    #[tokio::test]
    async fn fts_match_query_resolves_to_virtual_table_target() {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        sqlx::query("CREATE VIRTUAL TABLE docs_fts USING fts5(title, body)")
            .execute(db.pool())
            .await
            .unwrap();

        let targets = extract_dependencies(
            db.pool(),
            "SELECT rowid, title FROM docs_fts WHERE docs_fts MATCH 'hello'",
        )
        .await
        .unwrap();

        assert_eq!(
            targets,
            HashSet::from([DependencyTarget::VirtualTable("docs_fts".to_string())])
        );
    }

    #[tokio::test]
    async fn empty_dependency_set_is_non_reactive() {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        let error = extract_dependencies(db.pool(), "SELECT 1")
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            DependencyResolutionError::EmptyDependencySet
        ));
    }

    #[tokio::test]
    async fn unsupported_virtual_tables_are_non_reactive() {
        let db = hypr_db_core::Db::connect_memory_plain().await.unwrap();
        sqlx::query("CREATE VIRTUAL TABLE docs_rtree USING rtree(id, min_x, max_x)")
            .execute(db.pool())
            .await
            .unwrap();

        let error = extract_dependencies(db.pool(), "SELECT id FROM docs_rtree")
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            DependencyResolutionError::UnsupportedObject { .. }
        ));
    }

    #[test]
    fn parse_table_from_detail_accepts_scan_and_search_rows() {
        assert_eq!(
            parse_table_from_detail("SCAN daily_notes"),
            Some("daily_notes")
        );
        assert_eq!(
            parse_table_from_detail("SEARCH main.daily_notes USING INDEX idx_notes_date (date=?)"),
            Some("main.daily_notes")
        );
        assert_eq!(
            parse_table_from_detail("USE TEMP B-TREE FOR ORDER BY"),
            None
        );
    }

    #[test]
    fn normalize_identifier_strips_schema_quotes_and_punctuation() {
        assert_eq!(
            normalize_identifier("\"main\".\"daily_notes\","),
            "daily_notes"
        );
        assert_eq!(normalize_identifier("[daily_notes]);"), "daily_notes");
        assert_eq!(normalize_identifier("`daily_notes`"), "daily_notes");
    }

    #[test]
    fn build_alias_map_tracks_aliases_and_skips_sql_keywords() {
        let known_objects =
            HashSet::from(["daily_notes".to_string(), "daily_summaries".to_string()]);

        let aliases = build_alias_map(
            "SELECT dn.id FROM daily_notes dn JOIN daily_summaries AS ds ON ds.daily_note_id = dn.id WHERE dn.date IS NOT NULL ORDER BY dn.id",
            &known_objects,
        );
        assert_eq!(aliases.get("dn"), Some(&"daily_notes".to_string()));
        assert_eq!(aliases.get("ds"), Some(&"daily_summaries".to_string()));

        let no_aliases = build_alias_map("SELECT id FROM daily_notes ORDER BY id", &known_objects);
        assert!(no_aliases.is_empty());
    }
}
