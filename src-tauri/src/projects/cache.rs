use super::{models::*, resolver};
use crate::{ports::models::PortEntry, process::inspector};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    thread,
    time::{Duration, Instant},
};
use sysinfo::{Pid, ProcessesToUpdate, System};

#[derive(Clone, Hash, PartialEq, Eq)]
struct Key {
    pid: u32,
    start: u64,
    cwd: Option<String>,
    executable: Option<String>,
    command: Vec<String>,
    parent: Option<u32>,
}
impl Key {
    fn of(p: &PortEntry) -> Option<Self> {
        Some(Self {
            pid: p.pid?,
            start: p.started_at?,
            cwd: p.cwd.clone(),
            executable: p.executable.clone(),
            command: p.command.clone(),
            parent: p.parent_pid,
        })
    }
}
struct Cached {
    at: Instant,
    identity: Option<ProjectIdentity>,
}
#[derive(Default)]
struct Data {
    processes: HashMap<Key, Cached>,
    roots: HashMap<String, (Instant, ProjectIdentity)>,
    recent: Vec<RecentProject>,
    storage_error: Option<String>,
}
#[derive(Default)]
struct Queue {
    entries: Option<Vec<PortEntry>>,
    working: bool,
}
type EnrichmentListener = Arc<dyn Fn(Vec<PortEntry>) + Send + Sync>;
pub struct ProjectEngine {
    listener: Mutex<Option<EnrichmentListener>>,
    data: Mutex<Data>,
    queue: Mutex<Queue>,
    wake: Condvar,
    file: Option<PathBuf>,
    persistence: Mutex<()>,
    resolution: Mutex<()>,
}
impl ProjectEngine {
    pub fn new(file: Option<PathBuf>) -> Arc<Self> {
        let mut data = Data::default();
        if let Some(path) = &file {
            if path.exists() {
                match super::manifests::read_bounded(path, 4 * 1024 * 1024)
                    .and_then(|s| serde_json::from_str::<Vec<RecentProject>>(&s).ok())
                {
                    Some(mut recent) => {
                        recent.retain(|r| Path::new(&r.identity.root_path).is_absolute());
                        recent.truncate(200);
                        data.recent = recent;
                    }
                    None => {
                        data.storage_error = Some(
                            "Recent projects could not be read. New discoveries remain available."
                                .into(),
                        )
                    }
                }
            }
        }
        let engine = Arc::new(Self {
            listener: Mutex::new(None),
            data: Mutex::new(data),
            queue: Mutex::new(Queue::default()),
            wake: Condvar::new(),
            file,
            persistence: Mutex::new(()),
            resolution: Mutex::new(()),
        });
        let worker = engine.clone();
        thread::spawn(move || loop {
            let entries = {
                let mut queue = worker.queue.lock().unwrap();
                while queue.entries.is_none() {
                    if Arc::strong_count(&worker) == 1 {
                        return;
                    }
                    queue = worker
                        .wake
                        .wait_timeout(queue, Duration::from_secs(1))
                        .unwrap()
                        .0;
                }
                queue.working = true;
                queue.entries.take().unwrap()
            };
            worker.resolve_entries(&entries);
            if let Some(listener) = worker.listener.lock().unwrap().clone() {
                let mut enriched = entries;
                worker.enrich(&mut enriched, false);
                listener(enriched);
            }
            worker.queue.lock().unwrap().working = false;
        });
        engine
    }
    pub fn on_enriched(&self, listener: impl Fn(Vec<PortEntry>) + Send + Sync + 'static) {
        *self.listener.lock().unwrap() = Some(Arc::new(listener));
    }
    // Only memory work on the socket scan path. A single coalescing queue cannot build a backlog.
    pub fn enrich(&self, entries: &mut [PortEntry], submit: bool) {
        if let Ok(data) = self.data.lock() {
            for p in entries.iter_mut() {
                p.project = Key::of(p)
                    .and_then(|k| data.processes.get(&k))
                    .and_then(|c| c.identity.clone());
                p.service_name = Some(resolver::service(p, p.project.as_ref()));
            }
        }
        if submit {
            if let Ok(mut queue) = self.queue.lock() {
                queue.entries = Some(entries.to_vec());
                self.wake.notify_one();
            }
        }
    }
    fn resolve_entries(&self, entries: &[PortEntry]) {
        self.resolve(entries, true);
    }
    pub(crate) fn resolve_changed(&self, entries: &[PortEntry]) {
        self.resolve(entries, false);
    }
    fn resolve(&self, entries: &[PortEntry], prune: bool) {
        let _resolution = self.resolution.lock().unwrap();
        let live: HashSet<_> = entries.iter().filter_map(Key::of).collect();
        let now = crate::autopilot::models::now();
        let mut system = System::new();
        let parent_ids: Vec<_> = entries
            .iter()
            .filter_map(|e| e.parent_pid)
            .map(Pid::from_u32)
            .collect();
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&parent_ids),
            true,
            inspector::refresh_kind(),
        );
        for p in entries.iter().take(2048) {
            let Some(key) = Key::of(p) else {
                continue;
            };
            let cached = self
                .data
                .lock()
                .unwrap()
                .processes
                .get(&key)
                .filter(|c| c.at.elapsed() < Duration::from_secs(30))
                .map(|c| c.identity.clone());
            let identity = if let Some(value) = cached {
                value
            } else {
                let parent = p
                    .parent_pid
                    .and_then(|id| system.process(Pid::from_u32(id)))
                    .filter(|parent| {
                        crate::platform::same_user(parent) && parent.start_time() <= key.start
                    })
                    .and_then(|parent| parent.cwd());
                resolver::candidate(p, parent).map(|(root, markers, confidence, evidence)| {
                    let root_key = root.to_string_lossy().to_string();
                    let cached = self
                        .data
                        .lock()
                        .unwrap()
                        .roots
                        .get(&root_key)
                        .filter(|(at, _)| at.elapsed() < Duration::from_secs(60))
                        .map(|(_, v)| v.clone());
                    let mut identity = cached.unwrap_or_else(|| {
                        let identity = resolver::identity(&root, &markers);
                        self.data
                            .lock()
                            .unwrap()
                            .roots
                            .insert(root_key, (Instant::now(), identity.clone()));
                        identity
                    });
                    identity.confidence = confidence;
                    identity.evidence = evidence;
                    identity
                })
            };
            let mut data = self.data.lock().unwrap();
            // Cache time is not extended on hits: negatives and positives eventually refresh.
            if cached_is_expired(&data.processes, &key) {
                data.processes.insert(
                    key,
                    Cached {
                        at: Instant::now(),
                        identity: identity.clone(),
                    },
                );
            }
            if let Some(identity) = identity {
                // Weak argument inference is visible as tentative but never retained/grouped as fact.
                if identity.confidence != Confidence::Low {
                    if let Some(recent) = data
                        .recent
                        .iter_mut()
                        .find(|r| r.identity.root_path == identity.root_path)
                    {
                        recent.identity = identity;
                        recent.last_observed = now;
                        if !recent.known_ports.contains(&p.port) {
                            recent.known_ports.push(p.port);
                            recent.known_ports.sort();
                            recent.known_ports.truncate(128);
                        }
                    } else {
                        data.recent.push(RecentProject {
                            identity,
                            last_observed: now,
                            pinned: false,
                            known_ports: vec![p.port],
                        });
                    }
                }
            }
        }
        {
            let mut data = self.data.lock().unwrap();
            if prune {
                data.processes.retain(|k, _| live.contains(k));
            }
            data.roots
                .retain(|_, (at, _)| at.elapsed() < Duration::from_secs(300));
            data.recent.sort_by_key(|r| {
                (
                    std::cmp::Reverse(r.pinned),
                    std::cmp::Reverse(r.last_observed),
                )
            });
            data.recent.truncate(200);
        }
        self.persist();
    }
    pub fn snapshot(&self) -> ProjectSnapshot {
        let data = self.data.lock().unwrap();
        let queue = self.queue.lock().unwrap();
        ProjectSnapshot {
            projects: data.recent.clone(),
            resolving: queue.working || queue.entries.is_some(),
            storage_error: data.storage_error.clone(),
        }
    }
    pub fn known(&self, root: &str) -> Result<ProjectIdentity, String> {
        self.data
            .lock()
            .map_err(|_| "Projects unavailable")?
            .recent
            .iter()
            .find(|r| r.identity.root_path == root)
            .map(|r| r.identity.clone())
            .ok_or("This project is no longer in recent projects.".into())
    }
    pub fn pin(&self, root: &str, pinned: bool) -> Result<(), String> {
        {
            let mut data = self.data.lock().map_err(|_| "Projects unavailable")?;
            let recent = data
                .recent
                .iter_mut()
                .find(|r| r.identity.root_path == root)
                .ok_or("Unknown project")?;
            recent.pinned = pinned;
        }
        self.persist();
        Ok(())
    }
    pub fn forget(&self, root: &str) {
        self.data
            .lock()
            .unwrap()
            .recent
            .retain(|r| r.identity.root_path != root);
        self.persist();
    }
    pub fn refresh_project(&self, root: &str) -> Result<ProjectIdentity, String> {
        self.known(root)?;
        let (path, markers) = resolver::find_root(Path::new(root))
            .filter(|(path, _)| path == Path::new(root))
            .ok_or("Project markers are no longer available at this location.")?;
        let identity = resolver::identity(&path, &markers);
        {
            let mut data = self.data.lock().map_err(|_| "Projects unavailable")?;
            data.roots
                .insert(root.to_string(), (Instant::now(), identity.clone()));
            for cached in data.processes.values_mut() {
                if let Some(p) = &mut cached.identity {
                    if p.root_path == root {
                        let evidence = p.evidence.clone();
                        let confidence = p.confidence.clone();
                        *p = identity.clone();
                        p.evidence = evidence;
                        p.confidence = confidence;
                    }
                }
            }
            if let Some(recent) = data
                .recent
                .iter_mut()
                .find(|r| r.identity.root_path == root)
            {
                recent.identity = identity.clone();
            }
        }
        self.persist();
        Ok(identity)
    }
    pub fn invalidate(&self) {
        let mut d = self.data.lock().unwrap();
        d.processes.clear();
        d.roots.clear();
    }
    fn persist(&self) {
        let Some(file) = &self.file else {
            return;
        };
        let _guard = self.persistence.lock().unwrap();
        let result = (|| -> Result<(), String> {
            let bytes =
                serde_json::to_vec(&self.data.lock().unwrap().recent).map_err(|e| e.to_string())?;
            // Write only changed snapshots, at most once a minute during polling.
            if file
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .is_some_and(|t| t.elapsed().unwrap_or_default() < Duration::from_secs(60))
            {
                let previous: Vec<RecentProject> = std::fs::read(file)
                    .ok()
                    .and_then(|b| serde_json::from_slice(&b).ok())
                    .unwrap_or_default();
                let current = self.data.lock().unwrap().recent.clone();
                if previous.len() == current.len()
                    && previous.iter().zip(&current).all(|(a, b)| {
                        a.pinned == b.pinned
                            && a.identity == b.identity
                            && a.known_ports == b.known_ports
                    })
                {
                    return Ok(());
                }
            }
            std::fs::create_dir_all(file.parent().ok_or("Missing storage directory")?)
                .map_err(|e| e.to_string())?;
            let temp = file.with_extension("tmp");
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            use std::io::Write;
            let mut output = options.open(&temp).map_err(|e| e.to_string())?;
            output.write_all(&bytes).map_err(|e| e.to_string())?;
            output.sync_all().map_err(|e| e.to_string())?;
            std::fs::rename(temp, file).map_err(|e| e.to_string())?;
            Ok(())
        })();
        self.data.lock().unwrap().storage_error = result
            .err()
            .map(|e| format!("Recent projects could not be saved: {e}"));
    }
}
fn cached_is_expired(processes: &HashMap<Key, Cached>, key: &Key) -> bool {
    processes
        .get(key)
        .is_none_or(|c| c.at.elapsed() >= Duration::from_secs(30))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
