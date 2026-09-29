//! OS-specific discovery is delegated to netstat2's native implementations:
//! macOS libproc, Linux netlink/procfs, Windows IP Helper API.
//! No shell commands are used for socket or process discovery.
use netstat2::{AddressFamilyFlags, ProtocolFlags, SocketInfo};

pub fn sockets() -> Result<Vec<SocketInfo>, String> {
    netstat2::get_sockets_info(
        AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6,
        ProtocolFlags::TCP | ProtocolFlags::UDP,
    ).map_err(|e| format!("Unable to inspect local sockets: {e}. Some sockets may require permission from their owner."))
}

#[cfg(unix)]
pub fn same_user(process: &sysinfo::Process) -> bool {
    // SAFETY: geteuid has no arguments or preconditions and does not mutate memory.
    let uid = unsafe { libc::geteuid() };
    process.user_id().map(|id| **id == uid).unwrap_or(false)
}
#[cfg(windows)]
pub fn same_user(process: &sysinfo::Process) -> bool {
    static OWNER: std::sync::OnceLock<Option<sysinfo::Uid>> = std::sync::OnceLock::new();
    let owner = OWNER.get_or_init(|| {
        let mut system = sysinfo::System::new();
        let current = sysinfo::Pid::from_u32(std::process::id());
        system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::Some(&[current]),
            true,
            sysinfo::ProcessRefreshKind::nothing().with_user(sysinfo::UpdateKind::OnlyIfNotSet),
        );
        system.process(current).and_then(|p| p.user_id()).cloned()
    });
    owner
        .as_ref()
        .zip(process.user_id())
        .map(|(a, b)| a == b)
        .unwrap_or(false)
}

pub fn protected(pid: u32, name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    pid <= 4
        || pid == std::process::id()
        || matches!(
            name.as_str(),
            "kernel_task"
                | "launchd"
                | "windowserver"
                | "loginwindow"
                | "mdnsresponder"
                | "controlcenter"
                | "system"
                | "systemd"
                | "init"
                | "kthreadd"
                | "dbus-daemon"
                | "csrss.exe"
                | "wininit.exe"
                | "services.exe"
                | "lsass.exe"
                | "smss.exe"
                | "winlogon.exe"
                | "svchost.exe"
                | "registry"
                | "secure system"
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protects_critical_names_and_application() {
        for name in [
            "launchd",
            "WindowServer",
            "systemd",
            "lsass.exe",
            "mDNSResponder",
        ] {
            assert!(protected(9876, name));
        }
        assert!(protected(1, "anything"));
        assert!(protected(std::process::id(), "anything"));
        assert!(!protected(65432, "node"));
    }
}

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::terminate;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::terminate;
