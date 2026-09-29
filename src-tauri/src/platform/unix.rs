use sysinfo::{Process, Signal};

pub fn terminate(process: &Process, force: bool) -> Result<(), String> {
    match process.kill_with(if force { Signal::Kill } else { Signal::Term }) {
        Some(true) => Ok(()),
        Some(false) => Err(format!(
            "Unable to terminate PID {}. Permission denied, or the process has exited.",
            process.pid()
        )),
        None => Err("This termination signal is unsupported on this platform.".into()),
    }
}
