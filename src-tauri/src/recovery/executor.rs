//! A separate, detached supervisor drains child pipes after the desktop exits.
//! Only bounded, redacted output touches disk; execution context travels through stdin.
use crate::{
    autopilot::{
        capture::Capture,
        runner::{output_text, ManagedRun},
    },
    process::ProcessIdentity,
};
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader, Read, Seek, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};
#[derive(Serialize, Deserialize)]
struct Request {
    capture: Capture,
    output: PathBuf,
}

pub fn launch(capture: Capture, directory: &Path) -> Result<(ProcessIdentity, PathBuf), String> {
    capture.validate()?;
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
    }
    // Keep at most 200 recent output artifacts, matching the metadata retention cap.
    let mut files: Vec<_> = std::fs::read_dir(directory)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter(|e| {
            e.path().extension().is_some_and(|x| x == "log")
                && e.path()
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| uuid::Uuid::parse_str(s).is_ok())
        })
        .collect();
    files.sort_by_key(|e| std::cmp::Reverse(e.metadata().and_then(|m| m.modified()).ok()));
    for file in files.iter().skip(199) {
        let _ = std::fs::remove_file(file.path());
    }
    let output = directory.join(format!("{}.log", uuid::Uuid::new_v4()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(&output).map_err(|e| e.to_string())?;
    let mut command = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    #[cfg(not(test))]
    command.arg("--recovery-supervise");
    #[cfg(test)]
    command.args([
        "--ignored",
        "--exact",
        "recovery::tests::supervisor_fixture",
        "--nocapture",
    ]);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: setsid is async-signal-safe and this closure does not allocate.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x00000008 | 0x00000200);
    }
    let mut supervisor = command
        .spawn()
        .map_err(|e| format!("Unable to launch supervisor: {e}"))?;
    let request = Request {
        capture,
        output: output.clone(),
    };
    let mut stdin = supervisor
        .stdin
        .take()
        .ok_or("Supervisor input unavailable")?;
    serde_json::to_writer(&mut stdin, &request).map_err(|_| "Unable to transfer launch context")?;
    drop(stdin);
    let stdout = supervisor
        .stdout
        .take()
        .ok_or("Supervisor output unavailable")?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let mut reader = BufReader::new(stdout).take(16384);
        let result = loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break Err("Launch supervisor did not return an identity".into()),
                Ok(_) => {
                    if let Ok(result) =
                        serde_json::from_str::<Result<ProcessIdentity, String>>(&line)
                    {
                        break result;
                    }
                }
            }
        };
        let _ = tx.send(result);
        let _ = supervisor.wait();
    });
    let root = rx
        .recv_timeout(Duration::from_secs(10))
        .map_err(|_| "Supervisor did not respond. Inspect processes before trying again.")??;
    Ok((root, output))
}
pub fn supervise() -> i32 {
    let mut bytes = vec![];
    if std::io::stdin()
        .take(524289)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() > 524288
    {
        return 2;
    }
    let Ok(request) = serde_json::from_slice::<Request>(&bytes) else {
        return 2;
    };
    let mut file = match std::fs::OpenOptions::new()
        .write(true)
        .open(&request.output)
    {
        Ok(f) => f,
        Err(_) => return 2,
    };
    let result = ManagedRun::spawn(request.capture.clone());
    let identity: Result<ProcessIdentity, String> = result
        .as_ref()
        .map(|r| ProcessIdentity {
            pid: r.pid,
            started_at: r.started_at,
        })
        .map_err(Clone::clone);
    println!("{}", serde_json::to_string(&identity).unwrap());
    let _ = std::io::stdout().flush();
    let Ok(run) = result else {
        return 1;
    };
    let mut previous = String::new();
    loop {
        let exit = run.exit_status();
        if exit.is_some() {
            std::thread::sleep(Duration::from_millis(100));
        }
        let raw = output_text(&run.output);
        let truncated = run
            .output
            .lock()
            .is_ok_and(|o| o.len() >= crate::autopilot::runner::OUTPUT_LIMIT);
        let raw = if truncated {
            raw.split_once('\n').map(|(_, rest)| rest).unwrap_or("")
        } else {
            &raw
        };
        // Publish only complete lines while running, so partial secrets at the trailing
        // read boundary cannot be written before their remaining bytes arrive.
        let complete = if exit.is_some() {
            raw
        } else {
            raw.rfind('\n').map(|end| &raw[..=end]).unwrap_or("")
        };
        let mut output = super::launch_context::redact(complete, &request.capture);
        if let Some(exit) = &exit {
            output.push_str(&format!("\n{exit}"));
        }
        if output.len() > crate::autopilot::runner::OUTPUT_LIMIT {
            let mut start = output.len() - crate::autopilot::runner::OUTPUT_LIMIT;
            while !output.is_char_boundary(start) {
                start += 1;
            }
            output.drain(..start);
        }
        // Redaction is done on the whole ring so secrets split across reads are not exposed.
        if output != previous {
            let _ = file.rewind();
            let _ = file.write_all(output.as_bytes());
            let _ = file.set_len(output.len() as u64);
            let _ = file.flush();
            previous = output;
        }
        if exit.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    0
}
pub fn output(path: &Path) -> String {
    let mut bytes = vec![];
    if let Ok(file) = std::fs::File::open(path) {
        let _ = file.take(65536).read_to_end(&mut bytes);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}
