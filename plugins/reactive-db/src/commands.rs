use crate::{ManagedState, QueryEvent, execute_query};

#[tauri::command]
#[specta::specta]
pub async fn subscribe<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    state: tauri::State<'_, ManagedState>,
    sql: String,
    params: Vec<serde_json::Value>,
    on_event: tauri::ipc::Channel<QueryEvent>,
) -> Result<String, String> {
    let mut guard = state.lock().await;

    let pool = guard.pool.as_ref().ok_or("not initialized")?;
    let updates_tx = guard.updates_tx.as_ref().ok_or("not initialized")?;

    // Extract which tables this query depends on.
    let tables = crate::explain::extract_tables(pool, &sql)
        .await
        .map_err(|e| e.to_string())?;

    // Execute the query once and send the initial result set.
    let rows = execute_query(pool, &sql, &params)
        .await
        .map_err(|e| e.to_string())?;
    on_event
        .send(QueryEvent::Result(rows))
        .map_err(|e| e.to_string())?;

    // Clone everything the spawned task needs.
    let rx = updates_tx.subscribe();
    let task_pool = pool.clone();
    let task_sql = sql.clone();
    let task_params = params.clone();
    let task_tables = tables.clone();
    let task_channel = on_event.clone();

    tokio::spawn(async move {
        let mut rx = rx;
        loop {
            match rx.recv().await {
                Ok(update) => {
                    if !task_tables.contains(&update.table) {
                        continue;
                    }

                    // Drain any additional pending updates before re-querying
                    // so we don't re-run the same query multiple times for a
                    // batch of writes.
                    while rx.try_recv().is_ok() {}

                    match execute_query(&task_pool, &task_sql, &task_params).await {
                        Ok(rows) => {
                            if task_channel.send(QueryEvent::Result(rows)).is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            if task_channel.send(QueryEvent::Error(e.to_string())).is_err() {
                                break;
                            }
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    let id = uuid::Uuid::new_v4().to_string();

    guard.subscriptions.insert(
        id.clone(),
        crate::Subscription {
            sql,
            params,
            tables,
            channel: on_event,
        },
    );

    Ok(id)
}

#[tauri::command]
#[specta::specta]
pub async fn unsubscribe<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    state: tauri::State<'_, ManagedState>,
    subscription_id: String,
) -> Result<(), String> {
    let mut guard = state.lock().await;
    guard
        .subscriptions
        .remove(&subscription_id)
        .ok_or_else(|| format!("subscription not found: {subscription_id}"))?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn execute<R: tauri::Runtime>(
    _app: tauri::AppHandle<R>,
    state: tauri::State<'_, ManagedState>,
    sql: String,
    params: Vec<serde_json::Value>,
) -> Result<Vec<serde_json::Value>, String> {
    let guard = state.lock().await;
    let pool = guard.pool.as_ref().ok_or("not initialized")?;
    execute_query(pool, &sql, &params)
        .await
        .map_err(|e| e.to_string())
}
