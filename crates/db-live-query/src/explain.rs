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
            let normalized_name = normalize_identifier(name);
            if known_tables.contains(&normalized_name) {
                tables.insert(normalized_name);
            } else if let Some(real) = alias_map.get(&normalized_name) {
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

fn normalize_identifier(token: &str) -> String {
    let token = token.trim_matches(|c: char| matches!(c, ',' | ')' | ';' | '('));
    let token = token.rsplit('.').next().unwrap_or(token);
    strip_identifier_quotes(token).to_string()
}

fn strip_identifier_quotes(token: &str) -> &str {
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

        let raw_table = normalize_identifier(tokens[table_idx]);
        if !known_tables.contains(&raw_table) {
            continue;
        }

        let alias_idx = if table_idx + 1 < upper_tokens.len() && upper_tokens[table_idx + 1] == "AS"
        {
            table_idx + 2
        } else {
            table_idx + 1
        };

        if alias_idx < tokens.len() {
            let alias = normalize_identifier(tokens[alias_idx]);
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
                && !known_tables.contains(&alias)
            {
                map.insert(alias, raw_table.clone());
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_db() -> hypr_db_core2::Db3 {
        let db = hypr_db_core2::Db3::connect_memory_plain().await.unwrap();
        hypr_db_app::migrate(db.pool()).await.unwrap();
        db
    }

    #[tokio::test]
    async fn single_table() {
        let db = test_db().await;
        let tables = extract_tables(db.pool(), "SELECT id FROM daily_notes WHERE id = ?")
            .await
            .unwrap();
        assert_eq!(tables, HashSet::from(["daily_notes".to_string()]));
    }

    #[tokio::test]
    async fn join_query() {
        let db = test_db().await;
        let tables = extract_tables(
            db.pool(),
            "SELECT ds.id FROM daily_summaries ds JOIN daily_notes dn ON ds.daily_note_id = dn.id",
        )
        .await
        .unwrap();
        assert!(tables.contains("daily_summaries"));
        assert!(tables.contains("daily_notes"));
        assert_eq!(tables.len(), 2);
    }

    #[tokio::test]
    async fn alias_query() {
        let db = test_db().await;
        let tables = extract_tables(
            db.pool(),
            "SELECT dn.id FROM daily_notes AS dn WHERE dn.date = '2026-04-11'",
        )
        .await
        .unwrap();
        assert_eq!(tables, HashSet::from(["daily_notes".to_string()]));
    }

    #[tokio::test]
    async fn subquery() {
        let db = test_db().await;
        let tables = extract_tables(
            db.pool(),
            "SELECT id FROM daily_notes \
             WHERE EXISTS ( \
               SELECT 1 FROM daily_summaries \
               WHERE daily_summaries.daily_note_id = daily_notes.id \
             )",
        )
        .await
        .unwrap();
        assert!(tables.contains("daily_notes"));
        assert!(tables.contains("daily_summaries"));
        assert_eq!(tables.len(), 2);
    }

    #[tokio::test]
    async fn quoted_alias_query() {
        let db = test_db().await;
        let tables = extract_tables(
            db.pool(),
            r#"SELECT "dn".id FROM "daily_notes" AS "dn" WHERE "dn".date = '2026-04-11'"#,
        )
        .await
        .unwrap();
        assert_eq!(tables, HashSet::from(["daily_notes".to_string()]));
    }

    #[tokio::test]
    async fn schema_qualified_query() {
        let db = test_db().await;
        let tables = extract_tables(
            db.pool(),
            "SELECT dn.id FROM main.daily_notes dn WHERE dn.date = '2026-04-11'",
        )
        .await
        .unwrap();
        assert_eq!(tables, HashSet::from(["daily_notes".to_string()]));
    }
}
