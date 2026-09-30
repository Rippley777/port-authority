#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--recovery-supervise")) {
        std::process::exit(port_authority_lib::recovery::executor::supervise());
    }
    #[cfg(unix)]
    if std::env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--autopilot-run")) {
        let args: Result<Vec<String>, _> = std::env::args_os()
            .skip(2)
            .map(|arg| arg.into_string())
            .collect();
        match args {
            Ok(args) => std::process::exit(port_authority_lib::autopilot::shell::run(args)),
            Err(_) => {
                eprintln!("Port Authority: non-UTF8 arguments are unsupported. Run this command without pa.");
                std::process::exit(2);
            }
        }
    }
    port_authority_lib::run()
}
