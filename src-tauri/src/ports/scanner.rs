use super::models::PortEntry;
use crate::{platform, process::inspector};
use netstat2::{ProtocolSocketInfo, TcpState};
use std::collections::HashSet;
use sysinfo::{Pid, ProcessesToUpdate, System, Users};

pub struct Scanner {
    system: System,
    users: Users,
    observed: HashSet<crate::process::ProcessIdentity>,
}
impl Default for Scanner {
    fn default() -> Self {
        Self::new()
    }
}
impl Scanner {
    pub fn new() -> Self {
        Self {
            system: System::new(),
            users: Users::new_with_refreshed_list(),
            observed: HashSet::new(),
        }
    }
    pub fn scan_fresh(&mut self) -> Result<Vec<PortEntry>, String> {
        self.system = System::new();
        self.scan()
    }
    pub fn observed_identities(&self) -> HashSet<crate::process::ProcessIdentity> {
        self.observed.clone()
    }
    pub fn was_observed(&self, entry: &PortEntry) -> bool {
        entry
            .identity()
            .is_some_and(|identity| self.observed.contains(&identity))
    }
    pub fn scan(&mut self) -> Result<Vec<PortEntry>, String> {
        let sockets: Vec<_> = platform::sockets()?
            .into_iter()
            .filter(|socket| match &socket.protocol_socket_info {
                ProtocolSocketInfo::Tcp(tcp) => {
                    tcp.state == TcpState::Listen && tcp.local_port != 0
                }
                ProtocolSocketInfo::Udp(udp) => udp.local_port != 0,
            })
            .collect();
        if self.system.processes().len() > 4096 {
            self.system = System::new();
        }
        let pids: Vec<Pid> = sockets
            .iter()
            .flat_map(|s| s.associated_pids.iter().map(|p| Pid::from_u32(*p)))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        // Include cached PIDs so sysinfo removes dead processes. Metadata is refreshed only
        // when missing; a hard cache cap protects long-running sessions.
        let refresh_pids: Vec<Pid> = pids
            .iter()
            .copied()
            .chain(self.system.processes().keys().copied())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&refresh_pids),
            true,
            inspector::refresh_kind(),
        );
        let mut entries = Vec::new();
        for socket in sockets {
            let (protocol, address, port) = match socket.protocol_socket_info {
                ProtocolSocketInfo::Tcp(tcp) if tcp.state == TcpState::Listen => {
                    ("TCP", tcp.local_addr, tcp.local_port)
                }
                ProtocolSocketInfo::Udp(udp) => ("UDP", udp.local_addr, udp.local_port),
                _ => continue,
            };
            if port == 0 {
                continue;
            }
            let owners: Vec<Option<u32>> = if socket.associated_pids.is_empty() {
                vec![None]
            } else {
                socket.associated_pids.into_iter().map(Some).collect()
            };
            for pid in owners {
                let process = pid.and_then(|pid| self.system.process(Pid::from_u32(pid)));
                let name = process
                    .map(|p| p.name().to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Unknown process".into());
                let permission_limited = process.map(|p| p.exe().is_none()).unwrap_or(true);
                let system = process.map(|p| !platform::same_user(p)).unwrap_or(true);
                let protected = pid
                    .map(|pid| platform::protected(pid, &name))
                    .unwrap_or(true)
                    || system;
                entries.push(PortEntry {
                    project: None, service_name: None,
                    id: format!("{protocol}:{address}:{port}:{}", pid.map(|p| p.to_string()).unwrap_or_default()),
                    port, protocol: protocol.into(), address: address.to_string(), pid, process: name,
                    command: process.map(|p| p.cmd().iter().map(|a| a.to_string_lossy().into_owned()).collect()).unwrap_or_default(),
                    executable: process.and_then(|p| p.exe()).map(|p| p.to_string_lossy().into_owned()),
                    cwd: process.and_then(|p| p.cwd()).map(|p| p.to_string_lossy().into_owned()),
                    parent_pid: process.and_then(|p| p.parent()).map(|p| p.as_u32()),
                    user: process.and_then(|p| p.user_id()).map(|id| self.users.get_user_by_id(id).map(|u| u.name().to_owned()).unwrap_or_else(|| id.to_string())),
                    started_at: process.map(|p| p.start_time()).filter(|t| *t > 0),
                    memory: process.map(|p| p.memory()), cpu: process.map(|p| p.cpu_usage()),
                    system, protected, restartable: false,
                    restart_reason: "The original environment and process supervisor cannot be safely reproduced.".into(),
                    permission_limited,
                });
            }
        }
        entries.sort_by(|a, b| a.port.cmp(&b.port).then_with(|| a.id.cmp(&b.id)));
        entries.dedup_by(|a, b| a.id == b.id);
        if self.observed.len() > 4096 {
            self.observed.clear();
        }
        self.observed
            .extend(entries.iter().filter_map(PortEntry::identity));
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, UdpSocket};
    #[test]
    fn discovers_real_tcp_and_udp_sockets() {
        let tcp = TcpListener::bind("127.0.0.1:0").unwrap();
        let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
        let entries = Scanner::new().scan().unwrap();
        for (port, protocol) in [
            (tcp.local_addr().unwrap().port(), "TCP"),
            (udp.local_addr().unwrap().port(), "UDP"),
        ] {
            assert!(
                entries.iter().any(|e| e.port == port
                    && e.protocol == protocol
                    && e.pid == Some(std::process::id())),
                "Missing {protocol} socket on {port}"
            );
        }
    }
}
