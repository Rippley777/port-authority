use crate::{platform, process::inspector};
use sysinfo::{Pid, ProcessesToUpdate, System};

pub fn inspect_identity(pid: u32, started_at: Option<u64>) -> Result<System, String> {
    if started_at.is_none() || started_at == Some(0) {
        return Err("Process identity is unavailable. Refresh ports before trying again.".into());
    }
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[Pid::from_u32(pid)]),
        true,
        inspector::refresh_kind(),
    );
    let process = system
        .process(Pid::from_u32(pid))
        .ok_or_else(|| format!("PID {pid} is no longer running. Refresh the port list."))?;
    if Some(process.start_time()) != started_at {
        return Err(format!(
            "PID {pid} has been reused by a different process. Refresh before trying again."
        ));
    }
    Ok(system)
}

pub fn control(pid: u32, started_at: Option<u64>, action: &str) -> Result<(), String> {
    let force = match action {
        "kill" => false,
        "force" => true,
        "restart" => {
            return Err(
                "Restart is unavailable: the original environment cannot be safely reproduced."
                    .into(),
            )
        }
        _ => return Err("Unknown process action.".into()),
    };
    let system = inspect_identity(pid, started_at)?;
    let process = system
        .process(Pid::from_u32(pid))
        .ok_or("Process exited during inspection.")?;
    if platform::protected(pid, &process.name().to_string_lossy()) || !platform::same_user(process)
    {
        return Err(format!("PID {pid} is a protected or different-user process. Port Authority will not terminate it."));
    }
    platform::terminate(process, force)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_missing_identity_and_unknown_operations() {
        assert!(control(99999, None, "kill").is_err());
        assert!(control(99999, Some(1), "inject").is_err());
        assert!(control(99999, Some(1), "restart").is_err());
    }
    #[cfg(unix)]
    #[test]
    fn terminates_only_its_own_test_children() {
        use std::{
            process::Command,
            time::{Duration, Instant},
        };
        for action in ["kill", "force"] {
            let mut child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
            let pid = child.id();
            let mut system = System::new();
            system.refresh_processes_specifics(
                ProcessesToUpdate::Some(&[Pid::from_u32(pid)]),
                true,
                inspector::refresh_kind(),
            );
            let started = system.process(Pid::from_u32(pid)).unwrap().start_time();
            let result = control(pid, Some(started), action);
            if result.is_err() {
                let _ = child.kill();
                let _ = child.wait();
            }
            result.unwrap();
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if child.try_wait().unwrap().is_some() {
                    break;
                }
                if Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("Child did not terminate after {action}");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
    #[test]
    fn refuses_self_and_reused_pid() {
        let pid = std::process::id();
        assert!(control(pid, Some(1), "force")
            .unwrap_err()
            .contains("reused"));
        let mut sys = System::new();
        sys.refresh_processes(ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), true);
        let start = sys.process(Pid::from_u32(pid)).unwrap().start_time();
        assert!(control(pid, Some(start), "force")
            .unwrap_err()
            .contains("protected"));
    }
}
