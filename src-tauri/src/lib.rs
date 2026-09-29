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
        .invoke_handler(tauri::generate_handler![
            commands::ports::scan_ports,
            commands::ports::open_port,
            commands::processes::control_process,
            commands::processes::reveal_executable
        ])
        .run(tauri::generate_context!())
        .expect("Unable to start Port Authority");
}
