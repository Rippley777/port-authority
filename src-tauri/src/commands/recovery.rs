use crate::{
    process::ProcessIdentity,
    recovery::{
        models::{LaunchProfile, RecoveryStatus},
        RecoveryStateHandle,
    },
};
#[tauri::command]
pub async fn recovery_restart(
    id: String,
    identity: ProcessIdentity,
    port: u16,
    force: bool,
    state: tauri::State<'_, RecoveryStateHandle>,
) -> Result<RecoveryStatus, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.restart(&id, identity, port, force))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn recovery_status(
    id: String,
    state: tauri::State<'_, RecoveryStateHandle>,
) -> Option<RecoveryStatus> {
    state.status(&id)
}
#[tauri::command]
pub fn recovery_profile(
    project: String,
    state: tauri::State<'_, RecoveryStateHandle>,
) -> Option<LaunchProfile> {
    state.profile(&project)
}
#[tauri::command]
pub async fn recovery_save_profile(
    profile: LaunchProfile,
    state: tauri::State<'_, RecoveryStateHandle>,
) -> Result<(), String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.save_profile(profile))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn recovery_test_profile(
    project: String,
    port: u16,
    state: tauri::State<'_, RecoveryStateHandle>,
) -> Result<(String, RecoveryStatus), String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.test_profile(&project, port))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn recovery_terminal(directory: String) -> Result<(), String> {
    let path = std::path::Path::new(&directory);
    if !path.is_absolute() || !path.is_dir() {
        return Err("The original working directory no longer exists.".into());
    }
    crate::projects::actions::terminal(path, "auto")
}

#[tauri::command]
pub async fn recovery_inspect(
    id: String,
    state: tauri::State<'_, RecoveryStateHandle>,
) -> Result<Vec<crate::timeline::ancestry::TreeMember>, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.inspect(&id))
        .await
        .map_err(|e| e.to_string())?
}
