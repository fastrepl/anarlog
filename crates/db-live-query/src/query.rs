use hypr_db_core2::Db3;
use sqlx::{Column, Row, TypeInfo, ValueRef};

pub async fn run_query(
    db: &Db3,
    sql: &str,
    params: &[serde_json::Value],
) -> std::result::Result<Vec<serde_json::Value>, sqlx::Error> {
    let mut query = sqlx::query(sql);
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

    let rows = query.fetch_all(db.pool().as_ref()).await?;
    Ok(rows.iter().map(row_to_json).collect())
}

fn row_to_json(row: &sqlx::sqlite::SqliteRow) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (index, column) in row.columns().iter().enumerate() {
        let value = match row.try_get_raw(index) {
            Ok(raw) if !raw.is_null() => match raw.type_info().name() {
                "TEXT" => row
                    .get::<Option<String>, _>(index)
                    .map(serde_json::Value::String)
                    .unwrap_or(serde_json::Value::Null),
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
        };
        map.insert(column.name().to_string(), value);
    }

    serde_json::Value::Object(map)
}
