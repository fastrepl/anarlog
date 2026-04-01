use std::collections::HashSet;

use sqlx::{Row, SqlitePool};

/// # Safety
///
/// `sql` is interpolated into `format!("EXPLAIN QUERY PLAN {sql}")` and executed directly.
/// Only pass SQL from trusted code, never user input.
pub async fn extract_tables(pool: &SqlitePool, sql: &str) -> Result<HashSet<String>, sqlx::Error> {
    let master_rows = sqlx::query(
        "SELECT tbl_name FROM sqlite_master WHERE type = 'table' AND tbl_name NOT LIKE 'sqlite_%'",
    )
    .fetch_all(pool)
    .await?;

    let known_tables: HashSet<String> = master_rows
        .iter()
        .map(|r| r.get::<String, _>("tbl_name"))
        .collect();

    let alias_map = build_alias_map(sql, &known_tables);

    let eqp_rows = sqlx::query(&format!("EXPLAIN QUERY PLAN {sql}"))
        .fetch_all(pool)
        .await?;

    let mut tables = HashSet::new();
    for row in &eqp_rows {
        let detail: &str = row.get("detail");
        if let Some(name) = parse_table_from_detail(detail) {
            if known_tables.contains(name) {
                tables.insert(name.to_string());
            } else if let Some(real) = alias_map.get(name) {
                tables.insert(real.clone());
            }
        }
    }

    Ok(tables)
}

fn parse_table_from_detail(detail: &str) -> Option<&str> {
    let trimmed = detail.trim();
    let rest = trimmed
        .strip_prefix("SCAN ")
        .or_else(|| trimmed.strip_prefix("SEARCH "))?;
    rest.split_whitespace().next()
}

/// Build a map from alias → table name by scanning for `FROM table alias` and
/// `JOIN table alias` patterns in the SQL. This is intentionally simple — it
/// handles the common cases that EXPLAIN QUERY PLAN surfaces aliases for.
fn build_alias_map(
    sql: &str,
    known_tables: &HashSet<String>,
) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let upper = sql.to_uppercase();
    let tokens: Vec<&str> = sql.split_whitespace().collect();
    let upper_tokens: Vec<&str> = upper.split_whitespace().collect();

    for i in 0..tokens.len() {
        let is_from_or_join = matches!(
            upper_tokens[i],
            "FROM" | "JOIN" | "INNER" | "LEFT" | "RIGHT" | "CROSS"
        );
        if !is_from_or_join {
            continue;
        }

        // Skip "INNER/LEFT/RIGHT/CROSS" to get to "JOIN"
        let table_idx = if matches!(upper_tokens[i], "INNER" | "LEFT" | "RIGHT" | "CROSS") {
            if i + 1 < tokens.len() && upper_tokens[i + 1] == "JOIN" {
                i + 2
            } else {
                continue;
            }
        } else {
            i + 1
        };

        if table_idx >= tokens.len() {
            continue;
        }

        // Strip trailing comma/paren from the table token
        let raw_table = tokens[table_idx].trim_end_matches(|c| c == ',' || c == ')');
        if !known_tables.contains(raw_table) {
            continue;
        }

        // Check for optional AS alias or bare alias
        let alias_idx = if table_idx + 1 < upper_tokens.len() && upper_tokens[table_idx + 1] == "AS"
        {
            table_idx + 2
        } else {
            table_idx + 1
        };

        if alias_idx < tokens.len() {
            let alias =
                tokens[alias_idx].trim_end_matches(|c: char| c == ',' || c == ')' || c == ';');
            let alias_upper = alias.to_uppercase();
            if !alias.is_empty()
                && !matches!(
                    alias_upper.as_str(),
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
                && !known_tables.contains(alias)
            {
                map.insert(alias.to_string(), raw_table.to_string());
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> SqlitePool {
        let db = hypr_db_core2::Db3::connect_memory_plain().await.unwrap();
        hypr_db_app::migrate(db.pool()).await.unwrap();
        db.pool().clone()
    }

    #[tokio::test]
    async fn single_table() {
        let pool = test_pool().await;
        let tables = extract_tables(&pool, "SELECT id FROM meetings WHERE id = ?")
            .await
            .unwrap();
        assert_eq!(tables, HashSet::from(["meetings".to_string()]));
    }

    #[tokio::test]
    async fn join_query() {
        let pool = test_pool().await;
        let tables = extract_tables(
            &pool,
            "SELECT w.id FROM words w JOIN meetings m ON w.meeting_id = m.id",
        )
        .await
        .unwrap();
        assert!(tables.contains("words"));
        assert!(tables.contains("meetings"));
        assert_eq!(tables.len(), 2);
    }

    #[tokio::test]
    async fn fts_virtual_table() {
        let pool = test_pool().await;
        let tables = extract_tables(
            &pool,
            "SELECT rowid FROM meetings_fts WHERE meetings_fts MATCH 'test'",
        )
        .await
        .unwrap();
        assert!(tables.contains("meetings_fts"));
    }

    #[tokio::test]
    async fn subquery() {
        let pool = test_pool().await;
        let tables = extract_tables(
            &pool,
            "SELECT id FROM meetings WHERE id IN (SELECT meeting_id FROM words)",
        )
        .await
        .unwrap();
        assert!(tables.contains("meetings"));
        assert!(tables.contains("words"));
        assert_eq!(tables.len(), 2);
    }
}
