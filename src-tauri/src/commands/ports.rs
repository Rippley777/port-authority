use crate::{ports::models::PortEntry, ScannerState};
#[tauri::command]
pub async fn scan_ports(
    record_history: Option<bool>,
    timeline: tauri::State<'_, crate::timeline::TimelineState>,
    state: tauri::State<'_, ScannerState>,
    projects: tauri::State<'_, std::sync::Arc<crate::projects::cache::ProjectEngine>>,
) -> Result<Vec<PortEntry>, String> {
    let timeline = timeline.inner().clone();
    let scanner = state.inner().clone();
    let projects = projects.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = scanner
            .lock()
            .map_err(|_| "Socket scanner is unavailable. Restart Port Authority.".to_string())?
            .scan();
        let mut entries = match result {
            Ok(entries) => entries,
            Err(error) => {
                if let Ok(mut t) = timeline.lock() {
                    t.pause("Socket scan unavailable");
                }
                return Err(error);
            }
        };
        projects.enrich(&mut entries, true);
        if let Ok(mut t) = timeline.lock() {
            if record_history != Some(false) {
                t.observe(&entries, &projects);
            } else {
                t.pause("History recording disabled");
            }
        }
        projects.enrich(&mut entries, false);
        Ok(entries)
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
