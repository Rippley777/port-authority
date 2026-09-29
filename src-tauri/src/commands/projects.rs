use crate::projects::{
    actions::{self, Applications},
    cache::ProjectEngine,
    models::ProjectSnapshot,
};
use std::sync::Arc;
#[tauri::command]
pub fn projects_snapshot(state: tauri::State<'_, Arc<ProjectEngine>>) -> ProjectSnapshot {
    state.snapshot()
}
#[tauri::command]
pub async fn project_pin(
    state: tauri::State<'_, Arc<ProjectEngine>>,
    root: String,
    pinned: bool,
) -> Result<(), String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.pin(&root, pinned))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn project_forget(
    state: tauri::State<'_, Arc<ProjectEngine>>,
    root: String,
) -> Result<(), String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.forget(&root))
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn projects_refresh(state: tauri::State<'_, Arc<ProjectEngine>>) {
    state.invalidate();
}
#[tauri::command]
pub async fn project_applications() -> Result<Applications, String> {
    tauri::async_runtime::spawn_blocking(actions::applications)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn project_action(
    state: tauri::State<'_, Arc<ProjectEngine>>,
    root: String,
    action: String,
    preference: String,
    custom: Option<String>,
) -> Result<(), String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        actions::act(&state, &root, &action, &preference, custom.as_deref())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn project_refresh(
    state: tauri::State<'_, Arc<ProjectEngine>>,
    root: String,
) -> Result<crate::projects::models::ProjectIdentity, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.refresh_project(&root))
        .await
        .map_err(|e| e.to_string())?
}
