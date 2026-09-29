use crate::{ports::models::PortEntry, ScannerState};
#[tauri::command]
pub async fn scan_ports(state: tauri::State<'_, ScannerState>) -> Result<Vec<PortEntry>, String> {
    let scanner = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        scanner
            .lock()
            .map_err(|_| "Socket scanner is unavailable. Restart Port Authority.".to_string())?
            .scan()
    })
    .await
    .map_err(|e| format!("Socket scan interrupted: {e}"))?
}
#[tauri::command]
pub async fn open_port(port: u16, protocol: String) -> Result<(), String> {
    if port == 0 || !matches!(protocol.as_str(), "http" | "https") {
        return Err("Use a port from 1–65535 and HTTP or HTTPS.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        open::that(format!("{protocol}://localhost:{port}"))
            .map_err(|e| format!("Unable to open browser: {e}"))
    })
    .await
    .map_err(|e| e.to_string())?
}
