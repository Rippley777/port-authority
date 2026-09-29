use crate::autopilot::{
    engine::Autopilot,
    models::{Action, Snapshot},
};
use std::sync::Arc;
pub type AutopilotState = Arc<Autopilot>;
#[tauri::command]
pub async fn autopilot_snapshot(
    state: tauri::State<'_, AutopilotState>,
    projects: tauri::State<'_, Arc<crate::projects::cache::ProjectEngine>>,
) -> Result<Snapshot, String> {
    let engine = state.inner().clone();
    let projects = projects.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut snapshot = engine.snapshot()?;
        for conflict in &mut snapshot.conflicts {
            if let Some(owner) = &mut conflict.owner {
                projects.enrich(std::slice::from_mut(owner), false);
            }
            if let Some(listener) = &mut conflict.listener {
                projects.enrich(std::slice::from_mut(listener), false);
            }
        }
        Ok(snapshot)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn autopilot_enable(
    state: tauri::State<'_, AutopilotState>,
    enabled: bool,
) -> Result<(), String> {
    let engine = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || engine.enable(enabled))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn autopilot_action(
    state: tauri::State<'_, AutopilotState>,
    id: String,
    action: Action,
    approved: bool,
    target_port: Option<u16>,
) -> Result<(), String> {
    let engine = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || engine.act(&id, action, approved, target_port))
        .await
        .map_err(|e| e.to_string())?
}
