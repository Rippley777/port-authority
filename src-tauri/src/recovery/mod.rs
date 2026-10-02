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
        let mut context = launch_context::from_capture(&capture, root, Source::ShellObserved);
        let mut store = self.store.lock().map_err(|_| "Recovery unavailable")?;
        if let Some((id, previous)) = store.records.iter().find(|(_, record)| {
            record.context.executable == context.executable
                && record.context.args == context.args
                && record.context.working_directory == context.working_directory
                && record.context.kind == context.kind
                && record.context.shell == context.shell
        }) {
            context.id = id.clone();
            context.project_id = previous.context.project_id.clone();
            context.fingerprint = CommandFingerprint::from_context(&context).key();
        }
        let id = context.id.clone();
        self.persist(&context)?;
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
                context: context.clone(),
                capture: Some(capture),
            },
        );
        drop(store);
        self.persistence
            .lock()
            .map_err(|_| "Recovery storage unavailable")?
            .as_ref()
            .ok_or("Recovery storage unavailable")?
            .start_run(&context)?;
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
        for entry in entries.iter_mut() {
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
                record.context.fingerprint =
                    CommandFingerprint::from_context(&record.context).key();
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
                    context.fingerprint = CommandFingerprint::from_context(&context).key();
                    record.context = context;
                    record.capture = Some(capture);
                }
            }
            entry.restartable = record.context.recoverable && entry.protocol == "TCP";
            entry.restart_reason = record.context.reason.clone();
            entry.launch = Some(record.context.clone());
            let context = record.context.clone();
            drop(store);
            if let Ok(persistence) = self.persistence.lock() {
                if let Some(persistence) = persistence.as_ref() {
                    let _ = persistence.start_run(&context);
                    let _ = persistence.refresh_active_context(&context);
                    let _ = persistence.observe_port(&context, entry);
                }
            }
        }
        if let (Ok(store), Ok(persistence)) = (self.store.lock(), self.persistence.lock()) {
            if let Some(persistence) = persistence.as_ref() {
                for (id, record) in &store.records {
                    if !resolver::alive(record.context.launch_root) {
                        let _ = persistence.finish_run(
                            id,
                            CommandRunState::Stopped,
                            "The process is no longer running.",
                            None,
                        );
                    }
                }
            }
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
        let launched_context = self
            .store
            .lock()
            .ok()
            .and_then(|store| store.records.get(id).map(|record| record.context.clone()));
        if let (Some(context), Ok(persistence)) = (launched_context, self.persistence.lock()) {
            if let Some(persistence) = persistence.as_ref() {
                let _ = persistence.start_run(&context);
            }
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
    pub fn history(&self, query: HistoryQuery) -> Result<RunHistoryPage, String> {
        let (mut runs, persisted_contexts, pinned) = {
            let persistence = self
                .persistence
                .lock()
                .map_err(|_| "Run history unavailable")?;
            let persistence = persistence.as_ref().ok_or("Run history unavailable")?;
            (
                persistence.runs(5000)?,
                persistence.contexts()?,
                persistence.pinned()?,
            )
        };
        let runtime_contexts: HashMap<_, _> = self
            .store
            .lock()
            .map_err(|_| "Recovery unavailable")?
            .records
            .iter()
            .map(|(id, record)| (id.clone(), record.context.clone()))
            .collect();
        let mut contexts: HashMap<_, _> = persisted_contexts
            .into_iter()
            .map(|context| (context.id.clone(), context))
            .collect();
        contexts.extend(runtime_contexts);
        let search = query.search.unwrap_or_default().to_lowercase();
        let state = query.state.unwrap_or_default().to_uppercase();
        runs.retain(|run| {
            let Some(context) = contexts.get(&run.launch_context_id) else {
                return false;
            };
            let haystack = format!(
                "{} {} {} {} {} {}",
                run.project_name.as_deref().unwrap_or_default(),
                run.project_id.as_deref().unwrap_or_default(),
                run.process_name.as_deref().unwrap_or_default(),
                context.command,
                context.working_directory,
                run.observed_ports
                    .iter()
                    .map(|port| port.port.to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            )
            .to_lowercase();
            let matches_search = search
                .split_whitespace()
                .all(|term| haystack.contains(term));
            let matches_state = state.is_empty()
                || match state.as_str() {
                    "COMPLETED" => run.state == CommandRunState::Completed,
                    "FAILED" => run.state == CommandRunState::Failed,
                    "STOPPED" => run.state == CommandRunState::Stopped,
                    "RUNNING" => run.state == CommandRunState::Running,
                    _ => true,
                };
            let matches_time = query.since.is_none_or(|since| run.started_at >= since);
            let matches_port = query.port.is_none_or(|port| {
                run.observed_ports
                    .iter()
                    .any(|binding| binding.port == port)
            });
            matches_search && matches_state && matches_time && matches_port
        });
        let maximum = query.limit.unwrap_or(250).clamp(1, 1000);
        runs.truncate(maximum);

        let mut groups: HashMap<String, Vec<CommandRun>> = HashMap::new();
        for run in &runs {
            groups
                .entry(run.fingerprint.clone())
                .or_default()
                .push(run.clone());
        }
        let mut commands = vec![];
        for grouped_runs in groups.values() {
            let latest_run = grouped_runs[0].clone();
            let Some(context) = contexts.get(&latest_run.launch_context_id).cloned() else {
                continue;
            };
            let mut typical_ports: Vec<_> = grouped_runs
                .iter()
                .flat_map(|run| run.observed_ports.iter().map(|port| port.port))
                .collect();
            typical_ports.sort_unstable();
            typical_ports.dedup();
            let mut pinned_ports: Vec<_> = pinned
                .iter()
                .filter(|(_, context_id)| context_id == &context.id)
                .map(|(port, _)| *port)
                .collect();
            pinned_ports.sort_unstable();
            let active = resolver::alive(context.launch_root)
                || grouped_runs
                    .iter()
                    .any(|run| run.state == CommandRunState::Running);
            commands.push(HistoricalCommand {
                launch_context: context,
                latest_run,
                run_count: grouped_runs.len(),
                typical_ports,
                pinned_ports,
                active,
            });
        }
        let represented: std::collections::HashSet<_> = commands
            .iter()
            .map(|command| command.launch_context.id.clone())
            .collect();
        let mut pinned_contexts: HashMap<String, Vec<u16>> = HashMap::new();
        for (port, context_id) in &pinned {
            pinned_contexts
                .entry(context_id.clone())
                .or_default()
                .push(*port);
        }
        for (context_id, mut pinned_ports) in pinned_contexts {
            if represented.contains(&context_id)
                || !state.is_empty()
                || query.port.is_some_and(|port| !pinned_ports.contains(&port))
            {
                continue;
            }
            let Some(context) = contexts.get(&context_id).cloned() else {
                continue;
            };
            let haystack = format!(
                "{} {} {} {}",
                context.command,
                context.working_directory,
                context.project_id.as_deref().unwrap_or_default(),
                pinned_ports
                    .iter()
                    .map(u16::to_string)
                    .collect::<Vec<_>>()
                    .join(" ")
            )
            .to_lowercase();
            if !search
                .split_whitespace()
                .all(|term| haystack.contains(term))
            {
                continue;
            }
            pinned_ports.sort_unstable();
            pinned_ports.dedup();
            let last_used = context.relaunched_at.unwrap_or(context.captured_at);
            let latest_run = CommandRun {
                id: format!("pinned:{context_id}"),
                launch_context_id: context_id,
                fingerprint: context.fingerprint.clone(),
                started_at: last_used,
                ended_at: Some(last_used),
                exit_code: None,
                termination_reason: Some(
                    "Pinned command; individual execution history has expired.".into(),
                ),
                state: CommandRunState::Stopped,
                project_id: context.project_id.clone(),
                project_name: None,
                process_name: None,
                observed_ports: pinned_ports
                    .iter()
                    .map(|port| RunPort {
                        port: *port,
                        protocol: "TCP".into(),
                        address: String::new(),
                    })
                    .collect(),
                process_identity: None,
            };
            let active = resolver::alive(context.launch_root);
            commands.push(HistoricalCommand {
                launch_context: context,
                latest_run,
                run_count: 0,
                typical_ports: pinned_ports.clone(),
                pinned_ports,
                active,
            });
        }
        commands.sort_by_key(|command| std::cmp::Reverse(command.latest_run.started_at));
        Ok(RunHistoryPage {
            commands,
            runs,
            storage_error: self.storage_error.clone(),
        })
    }

    pub fn pin_command(&self, port: u16, context_id: &str, pinned: bool) -> Result<(), String> {
        if port == 0 {
            return Err("Choose a port from 1–65535.".into());
        }
        if !self
            .store
            .lock()
            .map_err(|_| "Recovery unavailable")?
            .records
            .contains_key(context_id)
        {
            return Err("Launch context unavailable.".into());
        }
        self.persistence
            .lock()
            .map_err(|_| "Run history unavailable")?
            .as_ref()
            .ok_or("Run history unavailable")?
            .pin(port, context_id, pinned)
    }

    pub fn remove_run(&self, id: &str) -> Result<(), String> {
        self.persistence
            .lock()
            .map_err(|_| "Run history unavailable")?
            .as_ref()
            .ok_or("Run history unavailable")?
            .remove_run(id)
    }

    pub fn run_again(&self, id: &str, confirmed: bool) -> Result<RecoveryStatus, String> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| "Another recovery is in progress")?;
        let (mut context, capture) = {
            let store = self.store.lock().map_err(|_| "Recovery unavailable")?;
            let record = store.records.get(id).ok_or("Launch context unavailable")?;
            (
                record.context.clone(),
                record
                    .capture
                    .clone()
                    .ok_or_else(|| record.context.reason.clone())?,
            )
        };
        safety::validate_launch(&context, &capture)?;
        if safety::requires_confirmation(&context, &capture) && !confirmed {
            return Err("Confirmation required: this recovered command is outside the standard development-command adapters. Review its exact arguments and working directory before running it.".into());
        }
        if self
            .store
            .lock()
            .map_err(|_| "Recovery unavailable")?
            .records
            .values()
            .any(|record| {
                record.context.fingerprint == context.fingerprint
                    && resolver::alive(record.context.launch_root)
            })
        {
            return Err(format!(
                "This command appears to already be running. Open or Restart it instead: {}",
                context.command
            ));
        }
        let expected_ports: Vec<u16> = {
            let persistence = self
                .persistence
                .lock()
                .map_err(|_| "Run history unavailable")?;
            let mut ports: Vec<_> = persistence
                .as_ref()
                .ok_or("Run history unavailable")?
                .runs(5000)?
                .into_iter()
                .filter(|run| run.launch_context_id == id)
                .find(|run| !run.observed_ports.is_empty())
                .into_iter()
                .flat_map(|run| run.observed_ports.into_iter().map(|port| port.port))
                .collect();
            ports.sort_unstable();
            ports.dedup();
            ports
        };
        let entries = self.scan()?;
        if let Some(conflict) = entries
            .iter()
            .find(|entry| expected_ports.contains(&entry.port))
        {
            return Err(format!(
                "Cannot run {} on :{}. {} currently owns the port. Inspect the conflict in Conflict Autopilot.",
                context.command,
                conflict.port,
                conflict
                    .project
                    .as_ref()
                    .map(|project| project.name.as_str())
                    .unwrap_or(&conflict.process)
            ));
        }
        let old_pid = context.launch_root.pid;
        self.store
            .lock()
            .map_err(|_| "Recovery unavailable")?
            .statuses
            .insert(
                id.into(),
                RecoveryStatus {
                    state: RecoveryState::Relaunching,
                    message: format!("Starting {}…", context.command),
                    old_pid,
                    new_pid: None,
                    ports: expected_ports.clone(),
                    output: String::new(),
                },
            );
        let (root, output) = match executor::launch(capture.clone(), &self.output_directory) {
            Ok(launched) => launched,
            Err(error) => {
                self.update(id, RecoveryState::LaunchFailed, &error);
                return Err(error);
            }
        };
        context.launch_root = root;
        context.relaunched_at = Some(now());
        {
            let mut store = self.store.lock().map_err(|_| "Recovery unavailable")?;
            let record = store
                .records
                .get_mut(id)
                .ok_or("Launch context unavailable")?;
            record.context = context.clone();
            record.capture = Some(capture);
            store.outputs.insert(id.into(), output);
            store.statuses.get_mut(id).unwrap().new_pid = Some(root.pid);
        }
        self.persist(&context)?;
        self.persistence
            .lock()
            .map_err(|_| "Run history unavailable")?
            .as_ref()
            .ok_or("Run history unavailable")?
            .start_run(&context)?;
        self.update(
            id,
            RecoveryState::VerifyingProcess,
            "Checking the new process identity…",
        );
        if root.started_at == 0 || !resolver::alive(root) {
            if let Ok(persistence) = self.persistence.lock() {
                if let Some(persistence) = persistence.as_ref() {
                    let _ = persistence.finish_run(
                        id,
                        CommandRunState::Failed,
                        "Command exited before it could be verified.",
                        None,
                    );
                }
            }
            self.update(
                id,
                RecoveryState::LaunchFailed,
                "The command exited before its process could be verified. View output.",
            );
            return Err(
                "The command exited before its process could be verified. View output.".into(),
            );
        }
        if expected_ports.is_empty() {
            self.update(
                id,
                RecoveryState::Running,
                format!(
                    "{} started. No historical port was available to verify.",
                    context.command
                ),
            );
            return self.status(id).ok_or("Recovery status unavailable".into());
        }
        self.update(
            id,
            RecoveryState::VerifyingPort,
            "Waiting for the expected service ports…",
        );
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut stable = 0;
        loop {
            let mut entries = self.scan()?;
            self.enrich(&mut entries);
            let verified = expected_ports.iter().all(|port| {
                let owners: Vec<_> = entries.iter().filter(|entry| entry.port == *port).collect();
                !owners.is_empty()
                    && owners.iter().all(|entry| {
                        entry.protocol == "TCP"
                            && correlation::contains(entry, &ancestry::ancestors(entry), root)
                    })
            });
            if verified && resolver::alive(root) {
                stable += 1;
            } else {
                stable = 0;
            }
            if stable >= 3 {
                self.update(
                    id,
                    RecoveryState::Running,
                    format!(
                        "{} started. Verified ports {}.",
                        context.command,
                        expected_ports
                            .iter()
                            .map(|port| format!(":{port}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                );
                return self.status(id).ok_or("Recovery status unavailable".into());
            }
            if !resolver::alive(root) || Instant::now() >= deadline {
                let message = format!(
                    "Command launched, but expected ports {} did not appear reliably. View output or inspect the process.",
                    expected_ports
                        .iter()
                        .map(|port| format!(":{port}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                if let Ok(persistence) = self.persistence.lock() {
                    if let Some(persistence) = persistence.as_ref() {
                        if resolver::alive(root) {
                            let _ = persistence.mark_unverified(id, &message);
                        } else {
                            let _ =
                                persistence.finish_run(id, CommandRunState::Failed, &message, None);
                        }
                    }
                }
                self.update(id, RecoveryState::ExpectedPortNotOpened, &message);
                return Err(message);
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
        let launched_context = self
            .store
            .lock()
            .ok()
            .and_then(|store| store.records.get(&id).map(|record| record.context.clone()));
        if let (Some(context), Ok(persistence)) = (launched_context, self.persistence.lock()) {
            if let Some(persistence) = persistence.as_ref() {
                let _ = persistence.start_run(&context);
            }
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
