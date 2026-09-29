use super::models::*;
use crate::{
    platform,
    process::{controller, inspector},
};
use sysinfo::{Pid, ProcessesToUpdate, System};

pub fn ancestors(entry: &crate::ports::models::PortEntry) -> Vec<Ancestor> {
    let Some(identity) = entry.identity() else {
        return vec![];
    };
    // Revalidate the listener before associating a parent with its historical identity.
    let Ok(system) = controller::inspect_identity(identity.pid, Some(identity.started_at)) else {
        return vec![];
    };
    let mut parent = system
        .process(Pid::from_u32(identity.pid))
        .and_then(|p| p.parent());
    let mut youngest = identity.started_at;
    let mut output: Vec<Ancestor> = vec![];
    let mut sys = System::new();
    while let Some(pid) = parent {
        if output.len() >= 12
            || pid.as_u32() <= 1
            || output.iter().any(|a| a.identity.pid == pid.as_u32())
        {
            break;
        }
        sys.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            inspector::refresh_kind(),
        );
        let Some(p) = sys.process(pid) else {
            break;
        };
        if p.start_time() == 0 || p.start_time() > youngest || !platform::same_user(p) {
            break;
        }
        output.push(Ancestor {
            identity: ProcessIdentity {
                pid: pid.as_u32(),
                started_at: p.start_time(),
            },
            name: p.name().to_string_lossy().into(),
            command: p.cmd().iter().map(|a| a.to_string_lossy().into()).collect(),
            cwd: p.cwd().map(|p| p.to_string_lossy().into()),
            parent_pid: p.parent().map(|p| p.as_u32()),
        });
        youngest = p.start_time();
        parent = p.parent();
    }
    output
}
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TreeMember {
    pub identity: ProcessIdentity,
    pub name: String,
    pub parent_pid: Option<u32>,
    pub command: Vec<String>,
}

pub fn inspect_tree(root: ProcessIdentity) -> Result<Vec<TreeMember>, String> {
    controller::inspect_identity(root.pid, Some(root.started_at))?;
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, inspector::refresh_kind());
    let p = system
        .process(Pid::from_u32(root.pid))
        .ok_or("Parent exited")?;
    if p.start_time() != root.started_at {
        return Err("Parent identity changed".into());
    }
    let mut ids = vec![root.pid];
    loop {
        let next: Vec<_> = system
            .processes()
            .values()
            .filter(|p| {
                !ids.contains(&p.pid().as_u32())
                    && p.parent().is_some_and(|v| ids.contains(&v.as_u32()))
            })
            .map(|p| p.pid().as_u32())
            .collect();
        if next.is_empty() {
            break;
        }
        ids.extend(next);
        if ids.len() > 512 {
            return Err("Process tree is too large to safely control".into());
        }
    }
    ids.into_iter()
        .map(|id| {
            let p = system.process(Pid::from_u32(id)).ok_or("Process exited")?;
            if platform::protected(id, &p.name().to_string_lossy())
                || !platform::same_user(p)
                || p.start_time() == 0
            {
                return Err("Tree includes protected, unknown, or different-user processes".into());
            }
            Ok(TreeMember {
                identity: ProcessIdentity {
                    pid: id,
                    started_at: p.start_time(),
                },
                name: p.name().to_string_lossy().into(),
                parent_pid: p.parent().map(|p| p.as_u32()),
                command: p.cmd().iter().map(|a| a.to_string_lossy().into()).collect(),
            })
        })
        .collect()
}
pub fn stop_tree(root: ProcessIdentity, approved: Vec<ProcessIdentity>) -> Result<(), String> {
    let current = inspect_tree(root)?;
    let expected: std::collections::HashSet<_> = approved.into_iter().collect();
    if current
        .iter()
        .map(|p| p.identity)
        .collect::<std::collections::HashSet<_>>()
        != expected
    {
        return Err("The process tree changed. Inspect it again before stopping.".into());
    }
    // Stop the supervisor first to reduce respawning, then only the exact reviewed identities.
    for member in current {
        controller::control(
            member.identity.pid,
            Some(member.identity.started_at),
            "kill",
        )?;
    }
    Ok(())
}
