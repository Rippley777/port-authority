use super::{adapters, launch_context, models::*};
use crate::{
    autopilot::capture::Capture,
    ports::models::PortEntry,
    process::{controller, inspector, ProcessIdentity},
    timeline::models::Ancestor,
};
use std::collections::BTreeMap;
use sysinfo::{Pid, ProcessesToUpdate, System, UpdateKind};

pub fn inspect(
    entry: &PortEntry,
    ancestors: &[Ancestor],
) -> Option<(LaunchContext, Option<Capture>)> {
    if entry.protected
        || adapters::excluded(&entry.process)
        || ancestors.iter().any(|a| adapters::excluded(&a.name))
    {
        return None;
    }
    let candidate = ancestors
        .iter()
        .take_while(|a| !matches!(a.name.as_str(), "zsh" | "bash" | "fish" | "Terminal"))
        .filter(|a| adapters::launch_root(&a.name, &a.command))
        .last();
    let (root, source) = candidate
        .map(|a| (a.identity, Source::ParentProcess))
        .unwrap_or((entry.identity()?, Source::ProcessInspection));
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[Pid::from_u32(root.pid)]),
        true,
        inspector::refresh_kind().with_environ(UpdateKind::Always),
    );
    let p = sys.process(Pid::from_u32(root.pid))?;
    if p.start_time() != root.started_at {
        return None;
    }
    let exact = p.cmd().iter().all(|a| a.to_str().is_some())
        && p.environ()
            .iter()
            .all(|v| v.to_str().is_some_and(|v| v.contains('=')));
    let argv: Vec<String> = p
        .cmd()
        .iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    let env: BTreeMap<String, String> = p
        .environ()
        .iter()
        .filter_map(|v| {
            v.to_str()?
                .split_once('=')
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
        })
        .collect();
    let actual_executable = p.exe()?;
    let invocation = argv.first().map(std::path::Path::new);
    // proc metadata can expose the real Python binary while argv still names a
    // virtualenv symlink. Keep that invocation path only when it resolves to the
    // same executable; never guess a different interpreter from the app's PATH.
    let executable = invocation
        .filter(|path| {
            path.is_absolute() && path.canonicalize().ok() == actual_executable.canonicalize().ok()
        })
        .unwrap_or(actual_executable);
    let compatible = adapters::compatible(&argv, executable)
        && (!env.contains_key("VIRTUAL_ENV") || invocation.is_some_and(|p| p.is_absolute()));
    let executable = executable.to_str()?.to_owned();
    let c = Capture {
        argv,
        executable,
        cwd: p.cwd()?.to_str()?.into(),
        env,
        error: String::new(),
        exit_code: 1,
    };
    let mut context = launch_context::from_capture(&c, root, source);
    // Raw node workers, rewritten npm titles, shells and unknown supervisors are metadata only.
    let complete = exact
        && compatible
        && adapters::supported(&c.argv)
        && !c.env.is_empty()
        && c.validate().is_ok();
    context.recoverable = complete;
    if !complete {
        context.reason = "Original launch command or environment could not be determined. Configure a recovery command or launch through shell integration.".into();
    }
    Some((context, complete.then_some(c)))
}
pub fn alive(identity: ProcessIdentity) -> bool {
    controller::inspect_identity(identity.pid, Some(identity.started_at))
        .ok()
        .and_then(|s| {
            s.process(Pid::from_u32(identity.pid))
                .map(|p| p.status() != sysinfo::ProcessStatus::Zombie)
        })
        .unwrap_or(false)
}
