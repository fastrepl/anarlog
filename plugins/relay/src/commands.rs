use crate::relay::PendingResults;

#[tauri::command]
pub async fn relay_result(
    state: tauri::State<'_, PendingResults>,
    id: u64,
    ok: bool,
    data: serde_json::Value,
) -> Result<(), String> {
    if let Some(tx) = state.take(id).await {
        let _ = tx.send(serde_json::json!({ "ok": ok, "data": data }));
    }
    Ok(())
}
