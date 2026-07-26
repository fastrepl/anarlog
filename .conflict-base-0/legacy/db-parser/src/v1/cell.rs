use serde_json::Value;

pub(super) fn is_tombstone(cells: &serde_json::Map<String, Value>) -> bool {
    cells.values().any(|v| {
        v.get(0)
            .and_then(|inner| inner.as_str())
            .is_some_and(|s| s == "\u{FFFC}")
    })
}

pub(super) fn get_cell_str<'a>(
    cells: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Option<&'a str> {
    cells.get(key)?.get(0)?.as_str()
}

pub(super) fn get_cell_f64(cells: &serde_json::Map<String, Value>, key: &str) -> Option<f64> {
    let val = cells.get(key)?.get(0)?;
    val.as_f64().or_else(|| val.as_i64().map(|n| n as f64))
}

pub(super) fn get_cell_i64(cells: &serde_json::Map<String, Value>, key: &str) -> Option<i64> {
    let val = cells.get(key)?.get(0)?;
    val.as_i64().or_else(|| val.as_f64().map(|n| n as i64))
}
