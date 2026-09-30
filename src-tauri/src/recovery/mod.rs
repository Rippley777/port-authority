pub mod adapters;
pub mod correlation;
pub mod executor;
pub mod launch_context;
pub mod models;
pub mod persistence;
pub mod resolver;
pub mod safety;
use crate::{
    autopilot::{capture::Capture, engine::port_free, models::now},
    ports::models::PortEntry,
    process::ProcessIdentity,
    projects::cache::ProjectEngine,
    timeline::ancestry,
    ScannerState,
};
use models::*;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
struct Record {
    context: LaunchContext,
    capture: Option<Capture>,
}
#[derive(Default)]
struct Store {
    records: HashMap<String, Record>,
    profiles: HashMap<String, LaunchProfile>,
    statuses: HashMap<String, RecoveryStatus>,
    outputs: HashMap<String, PathBuf>,
}
pub type RecoveryStateHandle = Arc<Recovery>;
pub struct Recovery {
    store: Mutex<Store>,
    persistence: Mutex<Option<persistence::Persistence>>,
    pub operation: Mutex<()>,
    scanner: ScannerState,
    projects: Arc<ProjectEngine>,
    output_directory: PathBuf,
    pub storage_error: Option<String>,
}
impl Recovery {
    pub fn new(path: &Path, scanner: ScannerState, projects: Arc<ProjectEngine>) -> Self {
        let result = persistence::Persistence::open(path);
        let mut error = result.as_ref().err().cloned();
        let mut store = Store::default();
        if let Ok(repo) = &result {
            match (repo.contexts(), repo.profiles()) {
                (Ok(contexts), Ok(profiles)) => {
                    for context in contexts {
                        store.records.insert(
                            context.id.clone(),
                            Record {
                                context,
                                capture: None,
                            },
                        );
                    }
                    for p in profiles {
                        store.profiles.insert(p.project_id.clone(), p);
                    }
                }
                _ => error = Some("Recovery metadata could not be read".into()),
            }
        }
        Self {
            store: Mutex::new(store),
            persistence: Mutex::new(result.ok()),
            operation: Mutex::new(()),
            scanner,
            projects,
            output_directory: path.with_file_name("recovery-output"),
            storage_error: error,
        }
    }
    fn persist(&self, c: &LaunchContext) -> Result<(), String> {
        self.persistence
            .lock()
            .map_err(|_| "Recovery storage unavailable")?
            .as_ref()
            .ok_or("Recovery storage unavailable")?
            .save(c)
    }
    pub fn observe(&self, capture: Capture, root: ProcessIdentity) -> Result<String, String> {
        capture.validate()?;
        let sys = crate::process::controller::inspect_identity(root.pid, Some(root.started_at))?;
        let p = sys
            .process(sysinfo::Pid::from_u32(root.pid))
            .ok_or("Launch exited")?;
        if !crate::platform::same_user(p)
            || crate::platform::protected(root.pid, &p.name().to_string_lossy())
        {
            return Err("Launch cannot be captured".into());
        }
        // Observed execs may rewrite argv (npm does); executable and cwd must still agree.
        if p.cwd() != Some(Path::new(&capture.cwd)) {
            return Err("Launch working directory changed".into());
        }
        let context = launch_context::from_capture(&capture, root, Source::ShellObserved);
        let id = context.id.clone();
        self.persist(&context)?;
        let mut store = self.store.lock().map_err(|_| "Recovery unavailable")?;
        if store.records.len() >= 512 {
            store
                .records
                .retain(|_, r| resolver::alive(r.context.launch_root));
        }
        if store.records.len() >= 512 {
            return Err("Too many captured launches".into());
        }
        store.records.insert(
            id.clone(),
            Record {
                context,
                capture: Some(capture),
            },
        );
        Ok(id)
    }
    pub fn observe_relaunch(
        &self,
        capture: Capture,
        root: ProcessIdentity,
    ) -> Result<String, String> {
        let id = self.observe(capture, root)?;
        let mut store = self.store.lock().map_err(|_| "Recovery unavailable")?;
        let context = &mut store
            .records
            .get_mut(&id)
            .ok_or("Context unavailable")?
            .context;
        context.relaunched_at = Some(now());
        self.persist(context)?;
        Ok(id)
    }
    pub fn enrich(&self, entries: &mut [PortEntry]) {
        for entry in entries {
            if entry.protected || entry.identity().is_none() || adapters::excluded(&entry.process) {
                continue;
            }
            let ancestors = ancestry::ancestors(entry);
            if ancestors.iter().any(|a| adapters::excluded(&a.name)) {
                continue;
            }
            let project = entry.project.as_ref().map(|p| p.root_path.clone());
            let mut store = match self.store.lock() {
                Ok(s) => s,
                Err(_) => continue,
            };
            if store.records.len() >= 512 {
                store
                    .records
                    .retain(|_, r| resolver::alive(r.context.launch_root));
                let retained: std::collections::HashSet<_> =
                    store.records.keys().cloned().collect();
                store.statuses.retain(|id, _| retained.contains(id));
                store.outputs.retain(|id, _| retained.contains(id));
            }
            let profile = project
                .as_ref()
                .and_then(|p| store.profiles.get(p))
                .cloned();
            let existing = store
                .records
                .iter()
                .filter(|(_, r)| correlation::contains(entry, &ancestors, r.context.launch_root))
                .max_by_key(|(_, r)| {
                    (
                        matches!(r.context.source, Source::ShellObserved),
                        r.context.captured_at,
                    )
                })
                .map(|(id, _)| id.clone());
            let id = existing.or_else(|| {
                if store.records.len() >= 512 {
                    return None;
                }
                let (context, capture) = resolver::inspect(entry, &ancestors).or_else(|| {
                    let profile = profile.as_ref()?;
                    let capture = profile_capture(profile).ok()?;
                    let root = ancestors
                        .iter()
                        .rfind(|a| adapters::launch_root(&a.name, &a.command))
                        .map(|a| a.identity)
                        .or_else(|| entry.identity())?;
                    let context = launch_context::from_capture(&capture, root, Source::UserDefined);
                    Some((context, Some(capture)))
                })?;
                let id = context.id.clone();
                let _ = self.persist(&context);
                store
                    .records
                    .insert(id.clone(), Record { context, capture });
                Some(id)
            });
            let Some(id) = id else { continue };
            let record = store.records.get_mut(&id).unwrap();
            if record.context.project_id != project && project.is_some() {
                record.context.project_id = project;
                let _ = self.persist(&record.context);
            }
            // A fallback recipe must not silently replace a newly observed package manager.
            if let Some(profile) = profile.filter(|_| {
                record.context.source != Source::ShellObserved || !record.context.recoverable
            }) {
                if let Ok(capture) = profile_capture(&profile) {
                    let mut context = launch_context::from_capture(
                        &capture,
                        record.context.launch_root,
                        Source::UserDefined,
                    );
                    context.id = id.clone();
                    context.captured_at = record.context.captured_at;
                    context.project_id = Some(profile.project_id);
                    context.relaunched_at = record.context.relaunched_at;
                    record.context = context;
                    record.capture = Some(capture);
                }
            }
            entry.restartable = record.context.recoverable && entry.protocol == "TCP";
            entry.restart_reason = record.context.reason.clone();
            entry.launch = Some(record.context.clone());
        }
    }
    pub(crate) fn scan(&self) -> Result<Vec<PortEntry>, String> {
        let mut entries = self
            .scanner
            .lock()
            .map_err(|_| "Scanner unavailable")?
            .scan_fresh()?;
        self.projects.resolve_changed(&entries);
        self.projects.enrich(&mut entries, false);
        Ok(entries)
    }
    /// Conflict Autopilot uses the same launch-root strategy before retrying its own command.
    pub fn stop_for_conflict(&self, entry: &PortEntry, force: bool) -> Result<bool, String> {
        let mut owners = vec![entry.clone()];
        self.projects.resolve_changed(&owners);
        self.projects.enrich(&mut owners, false);
        self.enrich(&mut owners);
        let Some(context) = owners[0].launch.as_ref().filter(|c| c.recoverable) else {
            return Ok(false);
        };
        let capture = self
            .store
            .lock()
            .map_err(|_| "Recovery unavailable")?
            .records
            .get(&context.id)
            .and_then(|r| r.capture.clone())
            .ok_or("Runtime environment is unavailable")?;
        let tree = safety::validate(&owners[0], context, &capture)?;
        safety::stop(&tree, force)?;
        let deadline = Instant::now() + Duration::from_secs(5);
        while tree.iter().any(|m| resolver::alive(m.identity)) {
            if Instant::now() >= deadline {
                return Err("Launch tree did not stop. Inspect it before retrying.".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(true)
    }
    pub fn spawn_retry(
        &self,
        capture: Capture,
    ) -> Result<Arc<crate::autopilot::runner::ManagedRun>, String> {
        crate::autopilot::runner::ManagedRun::spawn_detached(capture, &self.output_directory)
    }
    pub fn inspect(&self, id: &str) -> Result<Vec<ancestry::TreeMember>, String> {
        let store = self.store.lock().map_err(|_| "Recovery unavailable")?;
        let record = store.records.get(id).ok_or("Launch context unavailable")?;
        let mut tree = ancestry::inspect_tree(record.context.launch_root)?;
        if let Some(c) = &record.capture {
            for p in &mut tree {
                let mut c = c.clone();
                c.argv = p.command.clone();
                p.command = launch_context::safe_argv(&c);
            }
        }
        Ok(tree)
    }
    pub fn status(&self, id: &str) -> Option<RecoveryStatus> {
        let store = self.store.lock().ok()?;
        let mut status = store.statuses.get(id).cloned()?;
        if let Some(path) = store.outputs.get(id) {
            status.output = executor::output(path);
        }
        Some(status)
    }
    fn update(&self, id: &str, state: RecoveryState, message: impl Into<String>) {
        if let Ok(mut s) = self.store.lock() {
            if let Some(v) = s.statuses.get_mut(id) {
                v.state = state;
                v.message = message.into();
            }
        }
    }
    pub fn restart(
        &self,
        id: &str,
        identity: ProcessIdentity,
        port: u16,
        force: bool,
    ) -> Result<RecoveryStatus, String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Another recovery is in progress")?;
        self.store
            .lock()
            .map_err(|_| "Recovery unavailable")?
            .statuses
            .insert(
                id.into(),
                RecoveryStatus {
                    state: RecoveryState::Validating,
                    message: "Validating launch context and current port ownership…".into(),
                    old_pid: identity.pid,
                    new_pid: None,
                    ports: vec![port],
                    output: String::new(),
                },
            );
        match self.restart_inner(id, identity, port, force) {
            Ok(()) => Ok(self.status(id).unwrap()),
            Err((state, message)) => {
                self.update(id, state, &message);
                Err(message)
            }
        }
    }
    fn restart_inner(
        &self,
        id: &str,
        identity: ProcessIdentity,
        port: u16,
        force: bool,
    ) -> Result<(), (RecoveryState, String)> {
        use RecoveryState::*;
        let fail = |state| move |error: String| (state, error);
        let (context, capture) = {
            let store = self
                .store
                .lock()
                .map_err(|_| (CommandNotRecoverable, "Recovery unavailable".into()))?;
            let r = store
                .records
                .get(id)
                .ok_or((CommandNotRecoverable, "Launch context expired".into()))?;
            (
                r.context.clone(),
                r.capture
                    .clone()
                    .ok_or((CommandNotRecoverable, r.context.reason.clone()))?,
            )
        };
        let entries = self.scan().map_err(fail(ProcessChanged))?;
        let entry = entries
            .iter()
            .find(|p| p.port == port && safety::same(identity, p))
            .ok_or((
                ProcessChanged,
                "Process disappeared or port ownership changed. Refresh before restarting.".into(),
            ))?;
        if context.project_id != entry.project.as_ref().map(|p| p.root_path.clone()) {
            return Err((
                ProcessChanged,
                "Project identity changed. Refresh before restarting.".into(),
            ));
        }
        let tree =
            safety::validate(entry, &context, &capture).map_err(fail(CommandNotRecoverable))?;
        let mut ports: Vec<u16> = entries
            .iter()
            .filter(|p| {
                p.identity()
                    .is_some_and(|i| tree.iter().any(|m| m.identity == i))
            })
            .map(|p| p.port)
            .collect();
        ports.sort();
        ports.dedup();
        if entries.iter().any(|p| {
            ports.contains(&p.port)
                && (p.protocol != "TCP"
                    || p.identity()
                        .is_none_or(|i| !tree.iter().any(|m| m.identity == i)))
        }) {
            return Err((
                ProcessChanged,
                "A port is shared with another process or UDP socket. Recovery is unavailable."
                    .into(),
            ));
        }
        self.store
            .lock()
            .unwrap()
            .statuses
            .get_mut(id)
            .unwrap()
            .ports = ports.clone();
        // Reinspect the complete tree and port ownership immediately before the first signal.
        let current = ancestry::inspect_tree(context.launch_root).map_err(fail(ProcessChanged))?;
        if current
            .iter()
            .map(|p| p.identity)
            .collect::<std::collections::HashSet<_>>()
            != tree.iter().map(|p| p.identity).collect()
        {
            return Err((
                ProcessChanged,
                "Process tree changed. Try again after refreshing.".into(),
            ));
        }
        let fresh = self.scan().map_err(fail(ProcessChanged))?;
        let fresh_ports: std::collections::BTreeSet<_> = fresh
            .iter()
            .filter(|p| {
                p.identity()
                    .is_some_and(|i| tree.iter().any(|m| m.identity == i))
            })
            .map(|p| p.port)
            .collect();
        if fresh_ports != ports.iter().copied().collect() {
            return Err((ProcessChanged,"The launch opened or released a port while preparing recovery. Refresh before restarting.".into()));
        }
        if ports.iter().any(|port| {
            !fresh.iter().any(|p| p.port == *port)
                || fresh.iter().filter(|p| p.port == *port).any(|p| {
                    p.identity()
                        .is_none_or(|i| !tree.iter().any(|m| m.identity == i))
                })
        }) {
            return Err((
                ProcessChanged,
                "Port ownership changed before termination.".into(),
            ));
        }
        self.update(
            id,
            Terminating,
            "Stopping the launch root and its children…",
        );
        safety::stop(&tree, force).map_err(fail(TerminationFailed))?;
        self.update(
            id,
            WaitingForExit,
            "Waiting for the original process tree to exit…",
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while tree.iter().any(|m| resolver::alive(m.identity)) {
            if Instant::now() >= deadline {
                return Err((
                    TerminationFailed,
                    "The process tree did not exit. Inspect it before using Kill & Relaunch."
                        .into(),
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        self.update(
            id,
            WaitingForPortRelease,
            "Confirming all launch ports are available…",
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let entries = self.scan().map_err(fail(PortNotReleased))?;
            if ports.iter().all(|p| port_free(*p, &entries)) {
                break;
            }
            if Instant::now() >= deadline {
                return Err((
                    PortNotReleased,
                    "A port was not released or was reclaimed. Nothing was relaunched.".into(),
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        capture.validate().map_err(fail(CommandNotRecoverable))?;
        self.update(id, Relaunching, "Relaunching the original command…");
        let (root, output) = executor::launch(capture.clone(), &self.output_directory)
            .map_err(fail(LaunchFailed))?;
        {
            let mut store = self.store.lock().unwrap();
            store.outputs.insert(id.into(), output);
            store.statuses.get_mut(id).unwrap().new_pid = Some(root.pid);
            let record = store.records.get_mut(id).unwrap();
            record.context.launch_root = root;
            record.context.relaunched_at = Some(now());
            let _ = self.persist(&record.context);
        }
        self.update(id, VerifyingProcess, "Checking the new process identity…");
        if root.started_at == 0 || !resolver::alive(root) {
            return Err((
                LaunchFailed,
                "The relaunched command exited before its process could be verified. View output."
                    .into(),
            ));
        }
        self.update(
            id,
            VerifyingPort,
            "Waiting for the new process tree to reopen every expected port…",
        );
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut stable = 0;
        loop {
            let entries = self.scan().map_err(fail(ExpectedPortNotOpened))?;
            let all = ports.iter().all(|port| {
                let owners: Vec<_> = entries.iter().filter(|p| p.port == *port).collect();
                !owners.is_empty()
                    && owners.iter().all(|p| {
                        p.protocol == "TCP"
                            && correlation::contains(p, &ancestry::ancestors(p), root)
                            && p.project.as_ref().map(|p| &p.root_path)
                                == context.project_id.as_ref()
                    })
            });
            if all && resolver::alive(root) {
                stable += 1;
            } else {
                stable = 0;
            }
            if stable >= 3 {
                let listener = entries
                    .iter()
                    .find(|p| p.port == port)
                    .and_then(|p| p.pid)
                    .unwrap_or(root.pid);
                self.store
                    .lock()
                    .unwrap()
                    .statuses
                    .get_mut(id)
                    .unwrap()
                    .new_pid = Some(listener);
                self.update(
                    id,
                    Running,
                    format!(
                        "Restarted successfully. PID {} → {listener}. Verified ports {}.",
                        identity.pid,
                        ports
                            .iter()
                            .map(u16::to_string)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                );
                return Ok(());
            }
            if !resolver::alive(root) || Instant::now() >= deadline {
                return Err((ExpectedPortNotOpened,format!("Process relaunched, but expected ports {} did not reopen reliably. View output, inspect the process, or open a terminal.",ports.iter().map(u16::to_string).collect::<Vec<_>>().join(", "))));
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }
    pub fn save_profile(&self, profile: LaunchProfile) -> Result<(), String> {
        self.projects.known(&profile.project_id)?;
        let mut c = profile_capture(&profile)?;
        c.env.retain(|name, _| launch_context::sensitive(name));
        if launch_context::safe_argv(&c) != c.argv {
            return Err("Keep credentials out of recovery commands. Use your project's environment configuration.".into());
        }
        self.persistence
            .lock()
            .map_err(|_| "Recovery storage unavailable")?
            .as_ref()
            .ok_or("Recovery storage unavailable")?
            .profile(&profile)?;
        self.store
            .lock()
            .map_err(|_| "Recovery unavailable")?
            .profiles
            .insert(profile.project_id.clone(), profile);
        Ok(())
    }
    pub fn profile(&self, project: &str) -> Option<LaunchProfile> {
        self.store.lock().ok()?.profiles.get(project).cloned()
    }
    pub fn test_profile(
        &self,
        project: &str,
        port: u16,
    ) -> Result<(String, RecoveryStatus), String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Another recovery is in progress")?;
        let profile = self
            .profile(project)
            .ok_or("No recovery command configured")?;
        self.projects.known(project)?;
        let capture = profile_capture(&profile)?;
        let entries = self.scan()?;
        if entries
            .iter()
            .any(|p| p.project.as_ref().is_some_and(|p| p.root_path == project))
            || !port_free(port, &entries)
        {
            return Err("This project or its expected port is already running. Use Restart to replace the active launch; Test never stops an existing process.".into());
        }
        if self
            .store
            .lock()
            .map_err(|_| "Recovery unavailable")?
            .records
            .values()
            .any(|r| {
                r.context.project_id.as_deref() == Some(project)
                    && resolver::alive(r.context.launch_root)
            })
        {
            return Err("A launch for this project is still running. Inspect it before testing another command.".into());
        }
        let mut context = launch_context::from_capture(
            &capture,
            ProcessIdentity {
                pid: 0,
                started_at: 0,
            },
            Source::UserDefined,
        );
        context.project_id = Some(project.into());
        let id = context.id.clone();
        self.persist(&context)?;
        let (root, output) = executor::launch(capture.clone(), &self.output_directory)?;
        context.launch_root = root;
        context.relaunched_at = Some(now());
        let _ = self.persist(&context);
        {
            let mut store = self.store.lock().map_err(|_| "Recovery unavailable")?;
            store.records.insert(
                id.clone(),
                Record {
                    context,
                    capture: Some(capture),
                },
            );
            store.outputs.insert(id.clone(), output);
            store.statuses.insert(
                id.clone(),
                RecoveryStatus {
                    state: RecoveryState::VerifyingPort,
                    message: format!("Testing recovery command on port {port}…"),
                    old_pid: 0,
                    new_pid: Some(root.pid),
                    ports: vec![port],
                    output: String::new(),
                },
            );
        }
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut stable = 0;
        loop {
            let entries = match self.scan() {
                Ok(entries) => entries,
                Err(error) => {
                    self.update(&id, RecoveryState::ExpectedPortNotOpened, error);
                    break;
                }
            };
            let owners: Vec<_> = entries.iter().filter(|p| p.port == port).collect();
            let verified = !owners.is_empty()
                && owners.iter().all(|p| {
                    p.protocol == "TCP"
                        && correlation::contains(p, &ancestry::ancestors(p), root)
                        && p.project.as_ref().is_some_and(|p| p.root_path == project)
                });
            if verified && resolver::alive(root) {
                stable += 1;
            } else {
                stable = 0;
            }
            if stable >= 3 {
                self.update(
                    &id,
                    RecoveryState::Running,
                    format!(
                        "Recovery command verified on port {port}. The service is now running."
                    ),
                );
                break;
            }
            if !resolver::alive(root) || Instant::now() >= deadline {
                self.update(&id,RecoveryState::ExpectedPortNotOpened,format!("Recovery command launched, but port {port} did not reopen. View output or inspect the process."));
                break;
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        Ok((
            id.clone(),
            self.status(&id).ok_or("Recovery status unavailable")?,
        ))
    }
}
fn profile_capture(p: &LaunchProfile) -> Result<Capture, String> {
    let cwd = Path::new(&p.working_directory);
    if !cwd.is_absolute() || !cwd.is_dir() {
        return Err("The original working directory no longer exists. Choose an existing absolute directory.".into());
    }
    let executable =
        crate::autopilot::capture::resolve_executable(&p.executable, cwd).ok_or_else(|| {
            format!(
                "{} could not be found in the current environment.",
                p.executable
            )
        })?;
    let mut argv = vec![p.executable.clone()];
    argv.extend(p.args.clone());
    let capture = Capture {
        argv,
        executable: executable.to_string_lossy().into(),
        cwd: p.working_directory.clone(),
        env: std::env::vars().collect(),
        error: String::new(),
        exit_code: 1,
    };
    capture.validate()?;
    Ok(capture)
}
#[cfg(test)]
mod tests;
