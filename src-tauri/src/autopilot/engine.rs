use super::{
    capture::Capture,
    classifier::{classify, project_name},
    detector,
    models::{now, Action, Conflict, Risk, Snapshot, Status},
    runner::ManagedRun,
};
use crate::{ports::models::PortEntry, process::controller, ScannerState};
use std::{
    collections::HashMap,
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

struct Record {
    view: Conflict,
    capture: Option<Capture>,
    run: Option<Arc<ManagedRun>>,
}
#[derive(Default)]
struct Store {
    records: HashMap<String, Record>,
    launches: Vec<Arc<ManagedRun>>,
}
pub struct Autopilot {
    pub recovery: Option<crate::recovery::RecoveryStateHandle>,
    scanner: ScannerState,
    store: Mutex<Store>,
    operation: Mutex<()>,
    enabled: AtomicBool,
    pub connection_error: Mutex<Option<String>>,
    pub setup_command: String,
}
impl Autopilot {
    pub fn new(scanner: ScannerState, setup_command: String) -> Self {
        Self {
            recovery: None,
            scanner,
            store: Mutex::new(Store::default()),
            operation: Mutex::new(()),
            enabled: AtomicBool::new(false),
            connection_error: Mutex::new(None),
            setup_command,
        }
    }
    pub fn enable(&self, enabled: bool) -> Result<(), String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Wait for the current recovery to finish before changing Autopilot.")?;
        if enabled
            && (!cfg!(unix)
                || self
                    .connection_error
                    .lock()
                    .map_err(|_| "Autopilot unavailable")?
                    .is_some())
        {
            return Err(
                "Shell integration is unavailable. See the connection status in Autopilot.".into(),
            );
        }
        self.enabled.store(enabled, Ordering::SeqCst);
        if !enabled {
            let mut store = self.store.lock().map_err(|_| "Autopilot unavailable")?;
            // Active launch supervision remains alive. Pending failed-command contexts are discarded.
            store.records.clear();
        }
        Ok(())
    }
    pub fn start_maintenance(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(10));
            let Some(engine) = weak.upgrade() else {
                break;
            };
            if let Ok(mut store) = engine.store.lock() {
                Self::cleanup(&mut store);
            };
        });
    }
    fn scan(&self) -> Result<Vec<PortEntry>, String> {
        let mut entries = self
            .scanner
            .lock()
            .map_err(|_| "Socket scanner is unavailable".to_owned())?
            .scan()?;
        crate::process::inspector::enrich(&mut entries);
        Ok(entries)
    }
    pub fn observe_launch(
        &self,
        capture: Capture,
        root: crate::process::ProcessIdentity,
    ) -> Result<String, String> {
        if !self.enabled.load(Ordering::SeqCst) {
            return Err("Enable Conflict Autopilot to capture launches.".into());
        }
        self.recovery
            .as_ref()
            .ok_or("Command Recovery unavailable")?
            .observe(capture, root)
    }
    pub fn register(&self, capture: Capture) -> Result<String, String> {
        if !self.enabled.load(Ordering::SeqCst) {
            return Err(
                "Enable Conflict Autopilot in Port Authority before capturing commands.".into(),
            );
        }
        capture.validate()?;
        let port = detector::detect(&capture.error, capture.exit_code != 0)
            .ok_or("The output does not identify one unambiguous occupied port.")?;
        let (mut entries, observed) = {
            let mut scanner = self
                .scanner
                .lock()
                .map_err(|_| "Socket scanner is unavailable")?;
            let before = scanner.observed_identities();
            (scanner.scan()?, before)
        };
        crate::process::inspector::enrich(&mut entries);
        let owners: Vec<_> = entries
            .iter()
            .filter(|p| p.port == port && p.protocol == "TCP")
            .collect();
        let mut owner = owners
            .first()
            .filter(|first| {
                !entries
                    .iter()
                    .any(|p| p.port == port && p.protocol != "TCP")
                    && owners.iter().all(|p| {
                        p.pid.is_some() && p.pid == first.pid && p.started_at == first.started_at
                    })
            })
            .map(|p| (*p).clone());
        if let (Some(recovery), Some(entry)) = (&self.recovery, &mut owner) {
            let mut entries = recovery.scan()?;
            recovery.enrich(&mut entries);
            if let Some(enriched) = entries
                .into_iter()
                .find(|p| p.identity() == entry.identity() && p.port == entry.port)
            {
                *entry = enriched;
            }
        }
        let seen = owner
            .as_ref()
            .and_then(|p| p.identity())
            .is_some_and(|id| observed.contains(&id));
        let safety = classify(owner.as_ref(), &capture.cwd, seen);
        let alternate_supported = capture.alternate(port.saturating_add(1)).is_some();
        let alternative = if alternate_supported {
            next_port(port, &entries)
        } else {
            None
        };
        let mut store = self.store.lock().map_err(|_| "Autopilot unavailable")?;
        Self::cleanup(&mut store);
        if !self.enabled.load(Ordering::SeqCst) {
            return Err("Autopilot was disabled while capturing this conflict.".into());
        }
        // Repeated failures from the same project update one pending notification.
        if let Some((id, record)) = store.records.iter_mut().find(|(_, r)| {
            r.view.port == port
                && r.view.cwd == capture.cwd
                && r.capture.as_ref().is_some_and(|c| c.argv == capture.argv)
                && r.view.status == Status::Detected
        }) {
            record.view.evidence = crate::recovery::launch_context::redact(
                &detector::clean_output(&capture.error),
                &capture,
            );
            record.view.expires_at = now() + 900;
            record.capture = Some(capture);
            return Ok(id.clone());
        }
        if store.records.len() >= 32 {
            return Err(
                "Autopilot has 32 recent conflicts. Ignore an old conflict before adding another."
                    .into(),
            );
        }
        let owner_run = owner
            .as_ref()
            .and_then(|p| p.pid)
            .and_then(|pid| store.launches.iter().find(|run| run.owns_pid(pid)));
        let restart_owner_available = (owner_run.is_some()
            || owner.as_ref().is_some_and(|p| p.restartable))
            && safety.risk != Risk::Blocked;
        let id = Uuid::new_v4().to_string();
        let view = Conflict {
            id: id.clone(), port, command: crate::recovery::launch_context::safe_argv(&capture), cwd: capture.cwd.clone(), project: project_name(&capture.cwd), owner,
            safety, status: Status::Detected, message: "A development command failed because its port was already in use.".into(), evidence: crate::recovery::launch_context::redact(&detector::clean_output(&capture.error), &capture), steps: vec![], output: String::new(), alternative,
            alternate_reason: if alternate_supported { "Uses the framework’s explicit --port option. Availability is checked again before launch." } else { "Alternate ports are supported for direct Vite/Next dev commands and simple npm scripts containing vite or next dev. This command cannot be rewritten safely." }.into(),
            restart_owner_available,
            restart_owner_reason: if restart_owner_available { "The owner has a recoverable launch context. Its launch root and ports will be verified. This restarts the owner; it does not retry the blocked project." } else { "The owner’s complete launch environment was not captured, or the process is protected. Restart cannot be reproduced safely." }.into(),
            launched_pid: None, recovery_port: None, recovery_action: None, retry_command: None, listener: None, created_at: now(), expires_at: now()+900,
        };
        store.records.insert(
            id.clone(),
            Record {
                view,
                capture: Some(capture),
                run: None,
            },
        );
        Ok(id)
    }
    fn cleanup(store: &mut Store) {
        store.launches.retain(|run| run.exit_status().is_none());
        for record in store.records.values_mut() {
            if let Some(run) = &record.run {
                record.view.output = run.output_text();
                if let Some(exit) = run.exit_status() {
                    if matches!(record.view.status, Status::Resolved | Status::Unverified) {
                        record.view.status = Status::Failed;
                        record.view.message = format!("{exit} See captured output below.");
                    }
                    // The detached supervisor can finish draining output just after
                    // the root exits. Keep polling it until the final output arrives.
                    if run.output_finished() || now() >= record.view.expires_at {
                        record.run = None;
                    }
                }
            }
            if now() >= record.view.expires_at
                && !matches!(record.view.status, Status::Resolving | Status::Starting)
            {
                record.capture = None;
                if record.run.is_none()
                    && !matches!(record.view.status, Status::Ignored | Status::Expired)
                {
                    record.view.status = Status::Expired;
                    record.view.message =
                        "This capture expired. Run the command again to capture fresh context."
                            .into();
                }
            }
        }
        store.records.retain(|_, r| {
            !(matches!(r.view.status, Status::Ignored)
                || r.view.status == Status::Expired && now() > r.view.expires_at + 60)
        });
    }
    pub fn snapshot(&self) -> Result<Snapshot, String> {
        let mut store = self.store.lock().map_err(|_| "Autopilot unavailable")?;
        Self::cleanup(&mut store);
        let mut conflicts: Vec<_> = store.records.values().map(|r| r.view.clone()).collect();
        conflicts.sort_by_key(|c| std::cmp::Reverse(c.created_at));
        Ok(Snapshot {
            enabled: self.enabled.load(Ordering::SeqCst),
            supported: cfg!(unix),
            connection_error: self
                .connection_error
                .lock()
                .map_err(|_| "Autopilot unavailable")?
                .clone(),
            setup_command: self.setup_command.clone(),
            conflicts,
        })
    }
    fn update(&self, id: &str, status: Status, message: impl Into<String>, step: Option<String>) {
        if let Ok(mut store) = self.store.lock() {
            if let Some(record) = store.records.get_mut(id) {
                record.view.status = status;
                record.view.message = message.into();
                if let Some(step) = step {
                    record.view.steps.push(step);
                }
            }
        }
    }
    pub fn act(
        &self,
        id: &str,
        action: Action,
        approved: bool,
        target_port: Option<u16>,
    ) -> Result<(), String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Another recovery is in progress. Wait for it to finish.")?;
        let _shared_operation = if action != Action::RestartOwner {
            self.recovery
                .as_ref()
                .map(|r| {
                    r.operation
                        .try_lock()
                        .map_err(|_| "Another command recovery is in progress")
                })
                .transpose()?
        } else {
            None
        };
        if action == Action::Ignore {
            let mut store = self.store.lock().map_err(|_| "Autopilot unavailable")?;
            store.records.remove(id);
            return Ok(());
        }
        if !self.enabled.load(Ordering::SeqCst) {
            return Err("Conflict Autopilot is disabled.".into());
        }
        let (view, capture) = {
            let mut store = self.store.lock().map_err(|_| "Autopilot unavailable")?;
            Self::cleanup(&mut store);
            if store.launches.len() >= 8 {
                return Err("Eight recovered commands are still running. Stop a managed service before launching another.".into());
            }
            let record = store
                .records
                .get(id)
                .ok_or("Conflict no longer exists. Run the command again.")?;
            if record
                .run
                .as_ref()
                .is_some_and(|r| r.exit_status().is_none())
            {
                return Err("The previous retry is still running. Inspect or stop it before launching another copy.".into());
            }
            if !matches!(
                record.view.status,
                Status::Detected | Status::Failed | Status::ForceRequired
            ) {
                return Err("This conflict is not awaiting recovery.".into());
            }
            (
                record.view.clone(),
                record
                    .capture
                    .clone()
                    .ok_or("Captured context expired. Run the command again.")?,
            )
        };
        if action == Action::ForceRetry && view.status != Status::ForceRequired {
            return Err("Force Kill is offered only after graceful termination has failed.".into());
        }
        if matches!(action, Action::ForceRetry | Action::RestartOwner) && !approved {
            return Err("Explicit confirmation is required for this operation.".into());
        }
        // Validate executable and cwd BEFORE touching the blocking process.
        capture.validate()?;
        self.update(
            id,
            Status::Resolving,
            "Checking the current owner and command context…",
            None,
        );
        if let Ok(mut store) = self.store.lock() {
            if let Some(r) = store.records.get_mut(id) {
                r.view.recovery_action = Some(action);
                if action != Action::ForceRetry {
                    r.view.steps.clear();
                }
                r.view.output.clear();
                r.view.listener = None;
            }
        }
        let result = self.recover(&view, capture, action, approved, target_port);
        if let Err(error) = &result {
            self.update(id, Status::Failed, error, None);
        }
        result
    }
    fn recover(
        &self,
        view: &Conflict,
        mut capture: Capture,
        action: Action,
        approved: bool,
        target_port: Option<u16>,
    ) -> Result<(), String> {
        if action == Action::RestartOwner {
            if let Some(recovery) = &self.recovery {
                let owner = self
                    .verify_owner(view)?
                    .ok_or("The owner is no longer listening")?;
                let launch = view
                    .owner
                    .as_ref()
                    .and_then(|p| p.launch.as_ref())
                    .ok_or("Owner launch context is unavailable")?;
                let status = recovery.restart(
                    &launch.id,
                    owner.identity().ok_or("Owner identity is unavailable")?,
                    view.port,
                    false,
                )?;
                if let Ok(mut store) = self.store.lock() {
                    if let Some(record) = store.records.get_mut(&view.id) {
                        record.view.output = status.output;
                        record.view.launched_pid = status.new_pid;
                        record.view.recovery_port = Some(view.port);
                    }
                }
                self.update(&view.id,Status::Resolved,"Owner restarted and its ports were verified. The blocked project has not been retried.",Some(status.message));
                return Ok(());
            }
        }
        let mut port = view.port;
        if action == Action::Alternate {
            port = target_port
                .filter(|p| Some(*p) == view.alternative)
                .ok_or("The suggested alternate port changed. Review the current suggestion.")?;
            capture = capture
                .alternate(port)
                .ok_or("This command cannot be safely rewritten for another port.")?;
            if !port_free(port, &self.scan()?) {
                if let Ok(mut store) = self.store.lock() {
                    if let Some(r) = store.records.get_mut(&view.id) {
                        r.view.alternative = next_port(port, &self.scan()?);
                    }
                }
                return Err(format!("Port {port} was claimed by another process. Nothing was stopped. Review the new suggestion."));
            }
            self.update(
                &view.id,
                Status::Resolving,
                format!("Port {port} is available."),
                Some(format!(
                    "Verified port {port} is available; the existing owner was left running"
                )),
            );
        } else if action == Action::Retry {
            if !port_free(port, &self.scan()?) {
                return Err(format!(
                    "Port {port} is still occupied. Inspect its owner before retrying."
                ));
            }
        } else {
            let current = self.verify_owner(view)?;
            let Some(owner) = current else {
                if action == Action::RestartOwner {
                    return Err(
                        "The recorded owner is no longer listening. Nothing was restarted.".into(),
                    );
                }
                return self.launch(&view.id, capture, port, false);
            };
            let safety = classify(Some(&owner), &view.cwd, view.safety.previously_observed);
            if safety.risk == Risk::Blocked {
                return Err(safety.reasons.join(" "));
            }
            if safety.risk != Risk::Low && !approved {
                return Err(format!(
                    "Confirmation required. {}",
                    safety.reasons.join(" ")
                ));
            }
            let owner_run = if action == Action::RestartOwner {
                let store = self.store.lock().map_err(|_| "Autopilot unavailable")?;
                Some(store.launches.iter().find(|r| owner.pid.is_some_and(|pid|r.owns_pid(pid))).cloned().ok_or("The owner has no complete, active captured launch context. Restart is unavailable.")?)
            } else {
                None
            };
            if let Some(run) = &owner_run {
                capture = run.capture.clone();
                capture.validate()?;
            }
            // Recheck socket ownership immediately before the independent PID/start-time guard.
            self.verify_owner(view)?.ok_or(
                "The owner stopped while preparing recovery. Click Retry to recheck the free port.",
            )?;
            let stopped_tree = self
                .recovery
                .as_ref()
                .map(|r| r.stop_for_conflict(&owner, action == Action::ForceRetry))
                .transpose()?
                .unwrap_or(false);
            if !stopped_tree {
                controller::control(
                    owner.pid.ok_or("Missing PID")?,
                    owner.started_at,
                    if action == Action::ForceRetry {
                        "force"
                    } else {
                        "kill"
                    },
                )?;
            }
            self.update(
                &view.id,
                Status::Resolving,
                "Waiting for the owner to release the port…",
                Some(format!(
                    "Sent {} to PID {}",
                    if action == Action::ForceRetry {
                        "force termination"
                    } else {
                        "graceful termination"
                    },
                    owner.pid.unwrap()
                )),
            );
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                let entries = self.scan()?;
                if port_free(port, &entries) {
                    break;
                }
                // A replacement owner must never be killed by a force escalation.
                let matching: Vec<_> = entries
                    .iter()
                    .filter(|p| p.port == port && p.protocol == "TCP")
                    .collect();
                if matching.iter().any(|p| !same_identity(p, &owner)) {
                    return Err("The port now belongs to a different process. No further termination was attempted. Capture a new conflict.".into());
                }
                if Instant::now() >= deadline {
                    if action == Action::RestartOwner {
                        return Err("The owner did not release its port. Restart was not attempted; inspect the process manually.".into());
                    }
                    self.update(&view.id,Status::ForceRequired,"The owner did not release the port. You can inspect it or explicitly confirm Force Kill & Retry.",None);
                    return Ok(());
                }
                thread::sleep(Duration::from_millis(100));
            }
            self.update(
                &view.id,
                Status::Resolving,
                format!("Freed port {port}."),
                Some(format!("Confirmed port {port} is free")),
            );
            if let Some(run) = owner_run {
                let deadline = Instant::now() + Duration::from_secs(3);
                while run.exit_status().is_none() {
                    if Instant::now() >= deadline {
                        return Err("The owner released the port, but its launcher is still running. Restart was not attempted to avoid creating a duplicate supervisor.".into());
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
        self.launch(&view.id, capture, port, action == Action::RestartOwner)
    }
    fn verify_owner(&self, view: &Conflict) -> Result<Option<PortEntry>, String> {
        let mut entries = self
            .scanner
            .lock()
            .map_err(|_| "Socket scanner is unavailable")?
            .scan_fresh()?;
        crate::process::inspector::refresh_for_action(&mut entries);
        if port_free(view.port, &entries) {
            return Ok(None);
        }
        if entries
            .iter()
            .any(|p| p.port == view.port && p.protocol != "TCP")
        {
            return Err("A UDP socket also uses this port. TCP recovery cannot establish an unambiguous owner; nothing was stopped.".into());
        }
        let expected = view
            .owner
            .as_ref()
            .ok_or("The original owner was not verified. Nothing will be killed.")?;
        let owners: Vec<_> = entries
            .into_iter()
            .filter(|p| p.port == view.port && p.protocol == "TCP")
            .collect();
        if owners.is_empty() || owners.iter().any(|p| !same_identity(p, expected)) {
            return Err("The owner or its launch context changed. Nothing was killed. Run the command again to capture the new conflict.".into());
        }
        Ok(owners.into_iter().next())
    }
    fn launch(
        &self,
        id: &str,
        capture: Capture,
        port: u16,
        restarting_owner: bool,
    ) -> Result<(), String> {
        if !port_free(port, &self.scan()?) {
            return Err(format!(
                "Port {port} was claimed before launch. The command was not retried."
            ));
        }
        let run = if let Some(recovery) = &self.recovery {
            recovery.spawn_retry(capture)?
        } else {
            ManagedRun::spawn(capture)?
        };
        if let Some(recovery) = &self.recovery {
            let _ = recovery.observe_relaunch(
                run.capture.clone(),
                crate::process::ProcessIdentity {
                    pid: run.pid,
                    started_at: run.started_at,
                },
            );
        }
        {
            let mut store = self.store.lock().map_err(|_| "Autopilot unavailable")?;
            store.launches.push(run.clone());
            if let Some(record) = store.records.get_mut(id) {
                record.run = Some(run.clone());
                record.view.launched_pid = Some(run.pid);
                record.view.recovery_port = Some(port);
                record.view.retry_command =
                    Some(crate::recovery::launch_context::safe_argv(&run.capture));
            }
        }
        self.update(
            id,
            Status::Starting,
            format!(
                "Command launched as PID {}. Waiting to verify port {port}…",
                run.pid
            ),
            Some(format!(
                "{} the confirmed command with its captured working directory and environment",
                if restarting_owner {
                    "Relaunched owner using"
                } else {
                    "Retried"
                }
            )),
        );
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let entries = self.scan()?;
            let listeners: Vec<_> = entries
                .iter()
                .filter(|p| p.port == port && p.protocol == "TCP")
                .collect();
            if !listeners.is_empty()
                && listeners
                    .iter()
                    .all(|p| p.pid.is_some_and(|pid| run.owns_pid(pid)))
            {
                if let Ok(mut store) = self.store.lock() {
                    if let Some(r) = store.records.get_mut(id) {
                        r.view.listener = listeners.first().map(|p| (*p).clone());
                    }
                }
                self.update(id,Status::Resolved,if restarting_owner {format!("Owner restarted and verified on port {port}. The blocked project has not been retried.")} else {format!("Your retried command is listening on port {port}.")},Some(format!("Verified that the new process owns port {port}")));
                return Ok(());
            }
            if let Some(exit) = run.exit_status() {
                thread::sleep(Duration::from_millis(80));
                let text = run.output_text();
                let lines: Vec<_> = text.lines().rev().take(8).collect();
                let reason: String = lines
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join("\n")
                    .chars()
                    .take(1600)
                    .collect();
                return Err(format!("{exit} {reason}"));
            }
            if Instant::now() >= deadline {
                self.update(id,Status::Unverified,format!("PID {} is still running, but it has not been verified as the owner of port {port} after 15 seconds. Inspect its output; another retry is disabled while it runs.",run.pid),None);
                return Ok(());
            }
            thread::sleep(Duration::from_millis(200));
        }
    }
}
pub fn same_identity(a: &PortEntry, b: &PortEntry) -> bool {
    a.pid == b.pid
        && a.started_at == b.started_at
        && a.executable == b.executable
        && a.command == b.command
        && a.cwd == b.cwd
        && a.user == b.user
        && a.parent_pid == b.parent_pid
        && a.protected == b.protected
        && a.system == b.system
}
pub fn port_free(port: u16, entries: &[PortEntry]) -> bool {
    if port == 0 || entries.iter().any(|p| p.port == port) {
        return false;
    }
    // Bind checks catch listeners hidden by permissions and non-listening bound sockets.
    if TcpListener::bind(("0.0.0.0", port)).is_err() {
        return false;
    }
    match TcpListener::bind(("::", port)) {
        Ok(_) => true,
        Err(e) => matches!(
            e.kind(),
            std::io::ErrorKind::AddrNotAvailable | std::io::ErrorKind::Unsupported
        ),
    }
}
fn next_port(port: u16, entries: &[PortEntry]) -> Option<u16> {
    (u32::from(port) + 1..=(u32::from(port) + 100).min(65535))
        .map(|p| p as u16)
        .find(|p| port_free(*p, entries))
}

#[cfg(all(test, unix))]
#[path = "tests.rs"]
mod tests;
