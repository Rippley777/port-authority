pub mod autopilot;
pub mod projects;
pub mod recovery;
pub mod timeline;
use tauri::{Emitter, Manager};
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
            app.manage(Arc::new(Mutex::new(timeline::Timeline::new(
                &app.path().app_data_dir()?.join("port-timeline.sqlite"),
            ))));
            let projects = projects::cache::ProjectEngine::new(Some(
                app.path().app_data_dir()?.join("recent-projects.json"),
            ));
            let handle = app.handle().clone();
            projects.on_enriched(move |entries| {
                let _ = handle.emit("projects-resolved", entries);
            });
            app.manage(projects.clone());
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
            let recovery = Arc::new(recovery::Recovery::new(
                &app.path().app_data_dir()?.join("port-timeline.sqlite"),
                scanner.clone(),
                projects,
            ));
            app.manage(recovery.clone());
            let mut autopilot = autopilot::engine::Autopilot::new(scanner, setup);
            autopilot.recovery = Some(recovery);
            let engine = Arc::new(autopilot);
            #[cfg(unix)]
            if let Err(error) = autopilot::ipc::start(engine.clone()) {
                *engine.connection_error.lock().unwrap() = Some(error);
            }
            engine.start_maintenance();
            app.manage(engine);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::recovery::recovery_restart,
            commands::recovery::recovery_status,
            commands::recovery::recovery_inspect,
            commands::recovery::recovery_profile,
            commands::recovery::recovery_save_profile,
            commands::recovery::recovery_test_profile,
            commands::recovery::recovery_terminal,
            commands::timeline::timeline_query,
            commands::timeline::timeline_configure,
            commands::timeline::timeline_clear,
            commands::timeline::timeline_monitoring,
            commands::timeline::timeline_inspect_tree,
            commands::timeline::timeline_stop_tree,
            commands::projects::projects_snapshot,
            commands::projects::project_pin,
            commands::projects::project_forget,
            commands::projects::projects_refresh,
            commands::projects::project_refresh,
            commands::projects::project_applications,
            commands::projects::project_action,
            commands::autopilot::autopilot_snapshot,
            commands::autopilot::autopilot_enable,
            commands::autopilot::autopilot_action,
            commands::ports::scan_ports,
            commands::ports::open_port,
            commands::processes::control_process,
            commands::processes::reveal_executable
        ])
        .build(tauri::generate_context!())
        .expect("Unable to start Port Authority")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                if let Ok(mut timeline) = app.state::<timeline::TimelineState>().lock() {
                    timeline.pause("Port Authority closed");
                }
            }
        });
}
