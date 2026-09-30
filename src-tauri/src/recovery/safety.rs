use super::{correlation, models::LaunchContext};
use crate::{
    autopilot::capture::Capture,
    ports::models::PortEntry,
    process::{controller, ProcessIdentity},
    timeline::ancestry::{self, TreeMember},
};
pub fn validate(
    entry: &PortEntry,
    context: &LaunchContext,
    capture: &Capture,
) -> Result<Vec<TreeMember>, String> {
    if !std::path::Path::new(&capture.cwd).is_dir() {
        return Err(format!(
            "Cannot relaunch. The original working directory no longer exists: {}",
            capture.cwd
        ));
    }
    if !std::path::Path::new(&capture.executable).is_file() {
        return Err(format!(
            "Unable to restart. {} could not be found in the current environment.",
            capture
                .argv
                .first()
                .map(String::as_str)
                .unwrap_or("Executable")
        ));
    }
    capture.validate()?;
    if crate::autopilot::capture::resolve_executable(
        &capture.executable,
        std::path::Path::new(&capture.cwd),
    )
    .is_none()
    {
        return Err(format!(
            "{} could not be found in the current environment.",
            capture.argv[0]
        ));
    }
    if !context.recoverable || entry.protected || entry.protocol != "TCP" {
        return Err("Command is not safely recoverable.".into());
    }
    let identity = entry.identity().ok_or("Process identity unavailable")?;
    controller::inspect_identity(identity.pid, Some(identity.started_at))?;
    if !correlation::contains(entry, &ancestry::ancestors(entry), context.launch_root) {
        return Err("Launch ancestry changed. Refresh before restarting.".into());
    }
    ancestry::inspect_tree(context.launch_root)
}
pub fn stop(tree: &[TreeMember], force: bool) -> Result<(), String> {
    // Root first prevents ordinary supervisors from restarting the children we stop next.
    for member in tree {
        if super::resolver::alive(member.identity) {
            if let Err(error) = controller::control(
                member.identity.pid,
                Some(member.identity.started_at),
                if force { "force" } else { "kill" },
            ) {
                // A supervisor can reap a child between the identity check and signal.
                // A replaced PID is never signalled by controller::control.
                if super::resolver::alive(member.identity) {
                    return Err(error);
                }
            }
        }
    }
    Ok(())
}
pub fn same(expected: ProcessIdentity, actual: &PortEntry) -> bool {
    actual.identity() == Some(expected)
}
