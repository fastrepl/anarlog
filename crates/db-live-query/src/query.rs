use hypr_db_core2::Db3;
use sqlx::{Column, Row, TypeInfo, ValueRef};

use crate::{ProxyQueryMethod, ProxyQueryResult};

pub async fn run_query(
    db: &Db3,
    sql: &str,
    params: &[serde_json::Value],
) -> std::result::Result<Vec<serde_json::Value>, sqlx::Error> {
    let query = bind_params(sqlx::query(sql), params);
    let rows = query.fetch_all(db.pool().as_ref()).await?;
    Ok(rows.iter().map(row_to_json).collect())
}

pub async fn run_query_proxy(
    db: &Db3,
    sql: &str,
    params: &[serde_json::Value],
    method: ProxyQueryMethod,
) -> std::result::Result<ProxyQueryResult, sqlx::Error> {
    if method == ProxyQueryMethod::Run {
        bind_params(sqlx::query(sql), params)
            .execute(db.pool().as_ref())
            .await?;
        return Ok(ProxyQueryResult { rows: Vec::new() });
    }

    let query = bind_params(sqlx::query(sql), params);
    let rows = query.fetch_all(db.pool().as_ref()).await?;
    let rows = match method {
        ProxyQueryMethod::Get => rows
            .first()
            .map(row_to_json_array)
            .and_then(|value| value.as_array().cloned())
            .unwrap_or_default(),
        ProxyQueryMethod::All | ProxyQueryMethod::Values => {
            rows.iter().map(row_to_json_array).collect()
        }
        ProxyQueryMethod::Run => Vec::new(),
    };

    Ok(ProxyQueryResult { rows })
}

fn row_to_json(row: &sqlx::sqlite::SqliteRow) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (index, column) in row.columns().iter().enumerate() {
        let value = json_value_at(row, index);
        map.insert(column.name().to_string(), value);
    }

    serde_json::Value::Object(map)
}

fn row_to_json_array(row: &sqlx::sqlite::SqliteRow) -> serde_json::Value {
    serde_json::Value::Array(
        row.columns()
            .iter()
            .enumerate()
            .map(|(index, _)| json_value_at(row, index))
            .collect(),
    )
}

fn json_value_at(row: &sqlx::sqlite::SqliteRow, index: usize) -> serde_json::Value {
    match row.try_get_raw(index) {
        Ok(raw) if !raw.is_null() => match raw.type_info().name() {
            "INTEGER" | "INT" | "BOOLEAN" => row
                .get::<Option<i64>, _>(index)
                .map(serde_json::Value::from)
                .unwrap_or(serde_json::Value::Null),
            "REAL" => row
                .get::<Option<f64>, _>(index)
                .map(serde_json::Value::from)
                .unwrap_or(serde_json::Value::Null),
            "BLOB" => row
                .get::<Option<Vec<u8>>, _>(index)
                .map(serde_json::Value::from)
                .unwrap_or(serde_json::Value::Null),
            _ => row
                .get::<Option<String>, _>(index)
                .map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null),
        },
        _ => serde_json::Value::Null,
    }
}

fn bind_params<'q>(
    mut query: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
    params: &[serde_json::Value],
) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
    for param in params {
        query = match param {
            serde_json::Value::Null => query.bind(None::<String>),
            serde_json::Value::Bool(value) => query.bind(*value),
            serde_json::Value::Number(value) => {
                if let Some(integer) = value.as_i64() {
                    query.bind(integer)
                } else {
                    query.bind(value.as_f64().unwrap_or_default())
                }
            }
            serde_json::Value::String(value) => query.bind(value.clone()),
            other => query.bind(other.to_string()),
        };
    }

    query
}
