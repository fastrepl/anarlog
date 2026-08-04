use crate::{FakeWindowBounds, OverlayBound};
use tauri::Manager;

async fn update_bounds(
    window: &tauri::Window,
    state: &tauri::State<'_, FakeWindowBounds>,
    name: String,
    bounds: OverlayBound,
) -> Result<(), String> {
    let mut state = state.0.write().await;
    let map = state.entry(window.label().to_string()).or_default();
    map.insert(name, bounds);
    Ok(())
}

async fn remove_bounds(
    window: &tauri::Window,
    state: &tauri::State<'_, FakeWindowBounds>,
    name: String,
) -> Result<bool, String> {
    let mut state = state.0.write().await;
    let Some(map) = state.get_mut(window.label()) else {
        return Ok(true);
    };

    map.remove(&name);

    if map.is_empty() {
        state.remove(window.label());
        return Ok(true);
    }

    Ok(false)
}

#[tauri::command]
#[specta::specta]
pub async fn set_fake_window_bounds(
    window: tauri::Window,
    name: String,
    bounds: OverlayBound,
    state: tauri::State<'_, FakeWindowBounds>,
) -> Result<(), String> {
    update_bounds(&window, &state, name, bounds).await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_fake_window(
    window: tauri::Window,
    name: String,
    state: tauri::State<'_, FakeWindowBounds>,
) -> Result<(), String> {
    if remove_bounds(&window, &state, name).await? {
        crate::abort_overlay_listener(window.app_handle(), window.label()).await;
    }

    Ok(())
}
