use std::mem::zeroed;
use sysinfo::Process;
use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
use windows_sys::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, TerminateProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_TERMINATE,
};

pub fn terminate(process: &Process, force: bool) -> Result<(), String> {
    if !force {
        return Err("Windows does not provide a general graceful termination signal for arbitrary processes. Stop the service in its own application, or use Force Kill with confirmation.".into());
    }
    // SAFETY: This block operates only on an owned handle returned by OpenProcess.
    // Out-pointers refer to initialized FILETIME values. The handle is closed on
    // every path; creation time is rechecked on that same handle before termination.
    unsafe {
        let handle = OpenProcess(
            PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            process.pid().as_u32(),
        );
        if handle.is_null() {
            return Err(format!(
                "Unable to open PID {}: {}",
                process.pid(),
                std::io::Error::last_os_error()
            ));
        }
        let mut created: FILETIME = zeroed();
        let mut exited: FILETIME = zeroed();
        let mut kernel: FILETIME = zeroed();
        let mut user: FILETIME = zeroed();
        if GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) == 0 {
            let error = std::io::Error::last_os_error();
            CloseHandle(handle);
            return Err(format!("Unable to validate process identity: {error}"));
        }
        let ticks = ((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64;
        let started_at = (ticks / 10_000_000).saturating_sub(11_644_473_600);
        if started_at != process.start_time() {
            CloseHandle(handle);
            return Err("Process identity changed. Refresh before trying again.".into());
        }
        let result = TerminateProcess(handle, 1);
        let error = std::io::Error::last_os_error();
        CloseHandle(handle);
        if result == 0 {
            Err(format!(
                "Unable to terminate PID {}: {error}",
                process.pid()
            ))
        } else {
            Ok(())
        }
    }
}
