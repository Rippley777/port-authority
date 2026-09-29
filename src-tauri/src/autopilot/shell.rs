use super::{
    capture::{resolve_executable, Capture},
    detector, ipc,
    runner::{output_text, pump},
};
use std::{
    collections::BTreeMap,
    os::unix::process::{CommandExt, ExitStatusExt},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
/// An explicit executable wrapper, never a DEBUG/preexec trap or shell evaluator.
pub fn run(argv: Vec<String>) -> i32 {
    run_with_submit(argv, ipc::submit)
}
pub(crate) fn run_with_submit(
    mut argv: Vec<String>,
    submit: impl FnOnce(Capture) -> Result<String, String>,
) -> i32 {
    if argv.first().map(String::as_str) == Some("--") {
        argv.remove(0);
    }
    if argv.is_empty() {
        eprintln!("Usage: pa <executable> [arguments…]\nExample: pa npm run dev");
        return 2;
    }
    let cwd = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Unable to read working directory: {e}");
            return 1;
        }
    };
    let executable = match resolve_executable(&argv[0], &cwd) {
        Some(p) => p,
        None => {
            eprintln!("Executable not found: {}. Use an external executable, not a shell alias or function.",argv[0]);
            return 127;
        }
    };
    // Capture exported variables before launching. Non-UTF8 contexts remain executable,
    // but cannot be safely transferred by this version of the JSON protocol.
    let env: Result<BTreeMap<String, String>, std::ffi::OsString> = std::env::vars_os()
        .map(|(k, v)| Ok((k.into_string()?, v.into_string()?)))
        .collect();
    let mut command = Command::new(&executable);
    command
        .arg0(&argv[0])
        .args(&argv[1..])
        .current_dir(&cwd)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            eprintln!("Unable to run {}: {e}", argv[0]);
            return 126;
        }
    };
    let output = Arc::new(Mutex::new(Vec::new()));
    let readers = [
        pump(child.stdout.take().unwrap(), output.clone(), Some(false)),
        pump(child.stderr.take().unwrap(), output.clone(), Some(true)),
    ];
    let status = match child.wait() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Unable to wait for the command: {e}");
            return 1;
        }
    };
    let deadline = Instant::now() + Duration::from_millis(250);
    while readers.iter().any(|r| !r.is_finished()) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    let exit_code = status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap_or(1));
    let error = output_text(&output);
    if detector::detect(&error, !status.success()).is_some() {
        if let (Ok(env), Some(cwd), Some(executable)) = (env, cwd.to_str(), executable.to_str()) {
            let capture = Capture {
                argv,
                executable: executable.to_owned(),
                cwd: cwd.to_owned(),
                env,
                error,
                exit_code,
            };
            match submit(capture) {
                Ok(_)=>eprintln!("\nPort Authority: conflict captured. Open Conflict Autopilot to inspect the owner and choose a recovery action. Nothing has been stopped."),
                Err(error)=>eprintln!("\nPort Authority: {error}"),
            }
        } else {
            eprintln!("Port Authority: this environment contains non-UTF8 data and cannot be captured exactly. The original exit status is preserved.");
        }
    }
    exit_code
}
