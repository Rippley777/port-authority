use crate::timeline::{models::*, TimelineState};
#[tauri::command]
pub async fn timeline_query(
    state: tauri::State<'_, TimelineState>,
    query: Query,
) -> Result<Page, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .lock()
            .map_err(|_| "History unavailable")?
            .query(query)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn timeline_configure(
    state: tauri::State<'_, TimelineState>,
    config: Config,
) -> Result<(), String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state
            .lock()
            .map_err(|_| "History unavailable")?
            .configure(config)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn timeline_clear(state: tauri::State<'_, TimelineState>) -> Result<(), String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state.lock().map_err(|_| "History unavailable")?.clear()
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn timeline_monitoring(
    enabled: bool,
    state: tauri::State<'_, TimelineState>,
) -> Result<(), String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut timeline = state.lock().map_err(|_| "History unavailable")?;
        timeline.monitoring_paused = !enabled;
        if !enabled {
            timeline.pause("Monitoring paused");
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn timeline_inspect_tree(
    root: ProcessIdentity,
) -> Result<Vec<crate::timeline::ancestry::TreeMember>, String> {
    tauri::async_runtime::spawn_blocking(move || crate::timeline::ancestry::inspect_tree(root))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn timeline_stop_tree(
    root: ProcessIdentity,
    approved: Vec<ProcessIdentity>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::timeline::ancestry::stop_tree(root, approved)
    })
    .await
    .map_err(|e| e.to_string())?
}
