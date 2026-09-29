use crate::process::controller;
#[tauri::command]
pub async fn control_process(
    pid: u32,
    started_at: Option<u64>,
    action: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || controller::control(pid, started_at, &action))
        .await
        .map_err(|e| format!("Process operation interrupted: {e}"))?
}
#[tauri::command]
pub async fn reveal_executable(pid: u32, started_at: Option<u64>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let system = controller::inspect_identity(pid, started_at)?;
        let path = system
            .process(sysinfo::Pid::from_u32(pid))
            .and_then(|p| p.exe())
            .ok_or("Executable path is unavailable. The OS may require additional permissions.")?;
        let directory = path
            .parent()
            .ok_or("Executable directory is unavailable.")?;
        open::that(directory).map_err(|e| format!("Unable to open executable directory: {e}"))
    })
    .await
    .map_err(|e| e.to_string())?
}
