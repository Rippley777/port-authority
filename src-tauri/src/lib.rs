pub mod autopilot;
use tauri::Manager;
mod commands;
pub mod platform;
pub mod ports;
pub mod process;
use std::sync::{Arc, Mutex};

pub type ScannerState = Arc<Mutex<ports::scanner::Scanner>>;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(Arc::new(Mutex::new(ports::scanner::Scanner::new())))
        .setup(|app| {
            let scanner = app.state::<ScannerState>().inner().clone();
            let binary = std::env::current_exe()?;
            let mut script = app.path().resource_dir()?.join("shell/port-authority.sh");
            if cfg!(debug_assertions) && !script.exists() {
                script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../shell/port-authority.sh")
                    .canonicalize()?;
            }
            let quote = |path: &std::path::Path| {
                format!("'{}'", path.to_string_lossy().replace('\'', "'\"'\"'"))
            };
            let setup = format!(
                "export PORT_AUTHORITY_BIN={}\nsource {}\npa_autopilot_on",
                quote(&binary),
                quote(&script)
            );
            let engine = Arc::new(autopilot::engine::Autopilot::new(scanner, setup));
            #[cfg(unix)]
            if let Err(error) = autopilot::ipc::start(engine.clone()) {
                *engine.connection_error.lock().unwrap() = Some(error);
            }
            engine.start_maintenance();
            app.manage(engine);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::autopilot::autopilot_snapshot,
            commands::autopilot::autopilot_enable,
            commands::autopilot::autopilot_action,
            commands::ports::scan_ports,
            commands::ports::open_port,
            commands::processes::control_process,
            commands::processes::reveal_executable
        ])
        .run(tauri::generate_context!())
        .expect("Unable to start Port Authority");
}
