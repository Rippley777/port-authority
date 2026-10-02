use super::{
    privacy::{trace, AccessState, InspectionError},
    ProcessIdentity,
};
use crate::ports::models::PortEntry;
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};
use sysinfo::{ProcessRefreshKind, UpdateKind};

/// Core identity/control never asks for cwd, environment, or launch paths on macOS.
pub fn refresh_kind() -> ProcessRefreshKind {
    let kind = ProcessRefreshKind::nothing()
        .with_memory()
        .with_cpu()
        .with_user(UpdateKind::OnlyIfNotSet);
    #[cfg(target_os = "macos")]
    {
        kind
    }
    #[cfg(not(target_os = "macos"))]
    {
        kind.with_cmd(UpdateKind::Always)
            .with_exe(UpdateKind::Always)
            .with_cwd(UpdateKind::Always)
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataAccess {
    pub cwd: AccessState,
    pub executable: AccessState,
    pub command: AccessState,
}
#[derive(Clone)]
struct Metadata {
    cwd: Option<String>,
    executable: Option<String>,
    command: Vec<String>,
    access: MetadataAccess,
}
#[derive(Default)]
struct MetadataCache {
    values: HashMap<ProcessIdentity, Metadata>,
}
impl MetadataCache {
    fn resolve(
        &mut self,
        identity: ProcessIdentity,
        lookup: impl FnOnce() -> Result<Metadata, InspectionError>,
    ) {
        if self.values.contains_key(&identity) || self.values.len() >= 4096 {
            return;
        }
        let value = match lookup() {
            Ok(v) => v,
            Err(InspectionError::ProcessExited | InspectionError::Io(_)) => return,
            Err(e) => Metadata {
                cwd: None,
                executable: None,
                command: vec![],
                access: MetadataAccess {
                    cwd: e.state(),
                    executable: e.state(),
                    command: e.state(),
                },
            },
        };
        self.values.insert(identity, value);
    }
}
fn cache() -> &'static Mutex<MetadataCache> {
    static CACHE: OnceLock<Mutex<MetadataCache>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}
pub fn apply_cached(entries: &mut [PortEntry]) {
    if !cfg!(target_os = "macos") {
        return;
    }
    // A scan must never wait behind optional inspection.
    let Ok(cache) = cache().try_lock() else {
        return;
    };
    apply_values(&cache, entries);
}
fn apply_values(cache: &MetadataCache, entries: &mut [PortEntry]) {
    for entry in entries {
        if entry
            .launch
            .as_ref()
            .is_some_and(|l| l.source == crate::recovery::models::Source::ShellObserved)
        {
            continue;
        }
        let Some(value) = entry.identity().and_then(|id| cache.values.get(&id)) else {
            continue;
        };
        entry.cwd = value.cwd.clone();
        entry.executable = value.executable.clone();
        entry.command = value.command.clone();
        entry.permission_limited = value.access.cwd != AccessState::Available
            || value.access.command != AccessState::Available;
        entry.metadata_access = Some(value.access.clone());
    }
}
pub fn enrich(entries: &mut [PortEntry]) {
    if !cfg!(target_os = "macos") {
        return;
    }
    {
        let mut cache = cache().lock().unwrap();
        for entry in entries.iter() {
            if entry
                .launch
                .as_ref()
                .is_some_and(|l| l.source == crate::recovery::models::Source::ShellObserved)
            {
                continue;
            }
            if let Some(id) = entry.identity() {
                cache.resolve(id, || lookup(id, None));
            }
        }
        // Remove exited identities, including PID reuse. Missing BSD access does
        // not mean exit and must not erase a remembered permission failure.
        #[cfg(target_os = "macos")]
        cache.values.retain(|id, _| match basic(id.pid) {
            Ok(p) => p.started_at == id.started_at,
            Err(InspectionError::ProcessExited) => false,
            _ => true,
        });
        #[cfg(not(target_os = "macos"))]
        cache
            .values
            .retain(|id, _| entries.iter().any(|e| e.identity() == Some(*id)));
        apply_values(&cache, entries);
    }
}
pub fn refresh_for_action(entries: &mut [PortEntry]) {
    if !cfg!(target_os = "macos") {
        return;
    }
    {
        let mut cache = cache().lock().unwrap();
        for entry in entries.iter_mut() {
            let Some(id) = entry.identity() else {
                continue;
            };
            match lookup(id, cache.values.get(&id)) {
                Ok(value) => {
                    cache.values.insert(id, value);
                }
                Err(error) => {
                    // Never let stale successful metadata authorize a control
                    // action when fresh inspection failed.
                    cache.values.remove(&id);
                    entry.cwd = None;
                    entry.executable = None;
                    entry.command.clear();
                    entry.permission_limited = true;
                    entry.metadata_access = Some(MetadataAccess {
                        cwd: error.state(),
                        executable: error.state(),
                        command: error.state(),
                    });
                    cache.resolve(id, || Err(error));
                }
            }
        }
        apply_values(&cache, entries);
    }
}
pub fn explicit_retry() {
    cache().lock().unwrap().values.clear();
}
pub fn executable(identity: ProcessIdentity) -> Option<String> {
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[sysinfo::Pid::from_u32(identity.pid)]),
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::Always),
    );
    let p = system.process(sysinfo::Pid::from_u32(identity.pid))?;
    (p.start_time() == identity.started_at)
        .then(|| p.exe().map(|p| p.to_string_lossy().into_owned()))
        .flatten()
}
fn lookup(
    identity: ProcessIdentity,
    previous: Option<&Metadata>,
) -> Result<Metadata, InspectionError> {
    trace(identity.pid, "metadata", "begin");
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[sysinfo::Pid::from_u32(identity.pid)]),
        true,
        ProcessRefreshKind::nothing()
            .with_cmd(UpdateKind::Always)
            .with_exe(UpdateKind::Always),
    );
    let p = match system.process(sysinfo::Pid::from_u32(identity.pid)) {
        Some(p) => p,
        None => {
            #[cfg(target_os = "macos")]
            if basic(identity.pid)?.started_at == identity.started_at {
                // sysinfo could not expose metadata for a live identity. It does
                // not report errno; cache Unsupported rather than inventing denial.
                return Err(InspectionError::Unsupported);
            }
            return Err(InspectionError::ProcessExited);
        }
    };
    if p.start_time() != identity.started_at {
        return Err(InspectionError::ProcessExited);
    }
    let executable = p.exe().map(|p| p.to_string_lossy().into_owned());
    let command: Vec<_> = p
        .cmd()
        .iter()
        .map(|v| v.to_string_lossy().into_owned())
        .collect();
    let cwd = match previous.map(|p| p.access.cwd) {
        Some(AccessState::PermissionDenied) => Err(InspectionError::PermissionDenied),
        Some(AccessState::ProtectedResource) => Err(InspectionError::ProtectedResource),
        Some(AccessState::Unsupported) => Err(InspectionError::Unsupported),
        _ => working_directory(identity),
    };
    if matches!(
        cwd,
        Err(InspectionError::ProcessExited | InspectionError::Io(_))
    ) {
        return Err(cwd.unwrap_err());
    }
    let cwd_access = cwd
        .as_ref()
        .map(|_| AccessState::Available)
        .unwrap_or_else(|e| e.state());
    trace(identity.pid, "metadata", "complete");
    Ok(Metadata {
        access: MetadataAccess {
            cwd: cwd_access,
            // sysinfo does not expose errno: absence is Unknown, never a fabricated TCC denial.
            executable: if executable.is_some() {
                AccessState::Available
            } else {
                AccessState::Unknown
            },
            command: if command.is_empty() {
                AccessState::Unknown
            } else {
                AccessState::Available
            },
        },
        cwd: cwd.ok(),
        executable,
        command,
    })
}

#[cfg(target_os = "macos")]
pub struct BasicProcess {
    pub name: String,
    pub started_at: u64,
    pub parent: u32,
    pub uid: u32,
    pub memory: Option<u64>,
    pub cpu_time: Option<u64>,
}
#[cfg(target_os = "macos")]
pub fn basic(pid: u32) -> Result<BasicProcess, InspectionError> {
    let pid = i32::try_from(pid).map_err(|_| InspectionError::ProcessExited)?;
    // SAFETY: zeroed C POD structure with its exact size passed to libproc.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of_val(&info);
    let result = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            &mut info as *mut _ as *mut _,
            size as i32,
        )
    };
    if result != size as i32 {
        let e = std::io::Error::last_os_error();
        return Err(if e.raw_os_error() == Some(libc::ESRCH) {
            InspectionError::ProcessExited
        } else {
            e.into()
        });
    }
    let name: Vec<u8> = info
        .pbi_name
        .iter()
        .take_while(|c| **c != 0)
        .map(|c| *c as u8)
        .collect();
    let name = if name.is_empty() {
        info.pbi_comm
            .iter()
            .take_while(|c| **c != 0)
            .map(|c| *c as u8)
            .collect()
    } else {
        name
    };
    let mut task: libc::proc_taskinfo = unsafe { std::mem::zeroed() };
    let task_size = std::mem::size_of_val(&task);
    let task_available = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTASKINFO,
            0,
            &mut task as *mut _ as *mut _,
            task_size as i32,
        )
    } == task_size as i32;
    Ok(BasicProcess {
        name: String::from_utf8_lossy(&name).into_owned(),
        started_at: info.pbi_start_tvsec,
        parent: info.pbi_ppid,
        uid: info.pbi_uid,
        memory: task_available.then_some(task.pti_resident_size),
        cpu_time: task_available
            .then_some(task.pti_total_user.saturating_add(task.pti_total_system)),
    })
}
#[cfg(target_os = "macos")]
pub fn working_directory(identity: ProcessIdentity) -> Result<String, InspectionError> {
    trace(identity.pid, "resolve_cwd", "begin");
    let mut info: libc::proc_vnodepathinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of_val(&info);
    // This returns a kernel path string; it does not open the target directory.
    let result = unsafe {
        libc::proc_pidinfo(
            identity.pid as i32,
            libc::PROC_PIDVNODEPATHINFO,
            0,
            &mut info as *mut _ as *mut _,
            size as i32,
        )
    };
    if result != size as i32 {
        let error: InspectionError = std::io::Error::last_os_error().into();
        trace(
            identity.pid,
            "resolve_cwd",
            if matches!(error, InspectionError::PermissionDenied) {
                "permission_denied"
            } else {
                "unavailable"
            },
        );
        return Err(error);
    }
    if basic(identity.pid)?.started_at != identity.started_at {
        return Err(InspectionError::ProcessExited);
    }
    let bytes: Vec<_> = info
        .pvi_cdir
        .vip_path
        .iter()
        .flatten()
        .take_while(|c| **c != 0)
        .map(|c| *c as u8)
        .collect();
    if bytes.is_empty() {
        return Err(InspectionError::Unsupported);
    }
    let path = String::from_utf8(bytes).map_err(|_| InspectionError::Unsupported)?;
    if super::privacy::blocked(std::path::Path::new(&path)) {
        trace(identity.pid, "resolve_cwd", "protected_resource");
        return Err(InspectionError::ProtectedResource);
    }
    trace(identity.pid, "resolve_cwd", "available");
    Ok(path)
}
#[cfg(not(target_os = "macos"))]
pub fn working_directory(identity: ProcessIdentity) -> Result<String, InspectionError> {
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[sysinfo::Pid::from_u32(identity.pid)]),
        true,
        refresh_kind(),
    );
    let p = system
        .process(sysinfo::Pid::from_u32(identity.pid))
        .ok_or(InspectionError::ProcessExited)?;
    if p.start_time() != identity.started_at {
        return Err(InspectionError::ProcessExited);
    }
    p.cwd()
        .map(|p| p.to_string_lossy().into_owned())
        .ok_or(InspectionError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn denial_is_not_retried_and_pid_reuse_gets_its_own_state() {
        let mut cache = MetadataCache::default();
        let old = ProcessIdentity {
            pid: 42,
            started_at: 100,
        };
        cache.resolve(old, || Err(InspectionError::PermissionDenied));
        for _ in 0..100 {
            cache.resolve(old, || panic!("denial was retried"));
        }
        assert_eq!(cache.values[&old].access.cwd, AccessState::PermissionDenied);
        let new = ProcessIdentity {
            pid: 42,
            started_at: 101,
        };
        let mut calls = 0;
        cache.resolve(new, || {
            calls += 1;
            Err(InspectionError::ProtectedResource)
        });
        assert_eq!(calls, 1);
        assert_eq!(
            cache.values[&new].access.cwd,
            AccessState::ProtectedResource
        );
    }
    #[test]
    fn exited_processes_and_io_races_do_not_become_permission_denials() {
        let mut cache = MetadataCache::default();
        let id = ProcessIdentity {
            pid: 42,
            started_at: 100,
        };
        cache.resolve(id, || Err(InspectionError::ProcessExited));
        assert!(cache.values.is_empty());
        cache.resolve(id, || Err(InspectionError::Unsupported));
        cache.resolve(id, || panic!("unsupported operation was retried"));
    }
}
