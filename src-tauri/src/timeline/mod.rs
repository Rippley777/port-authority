pub mod ancestry;
pub mod correlation;
pub mod models;
pub mod reconciler;
pub mod repository;
#[cfg(test)]
mod tests;
use crate::{ports::models::PortEntry, projects::cache::ProjectEngine};
use models::*;
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Instant,
};
pub type TimelineState = Arc<Mutex<Timeline>>;
pub fn now() -> u64 {
    crate::autopilot::models::now()
}
pub struct Timeline {
    repo: Option<repository::Repository>,
    reconciler: reconciler::Reconciler,
    session: Option<MonitoringSession>,
    last_scan: Option<(u64, Instant)>,
    pub config: Config,
    pub monitoring_paused: bool,
    pub error: Option<String>,
    maintenance: Instant,
}
impl Timeline {
    pub fn new(path: &Path) -> Self {
        let result = repository::Repository::open(path);
        let (repo, mut error) = match result {
            Ok(r) => (Some(r), None),
            Err(e) => (None, Some(e)),
        };
        let mut previous = vec![];
        let mut config = Config::default();
        if let Some(repo) = &repo {
            match repo.state("owners") {
                Ok(p) => previous = p,
                Err(e) => error = Some(e),
            }
            match repo.state("config") {
                Ok(c) => config = c,
                Err(e) => error = Some(e),
            }
        }
        let mut reconciler = reconciler::Reconciler::default();
        reconciler.previous = previous;
        Self {
            repo,
            reconciler,
            session: None,
            last_scan: None,
            config,
            monitoring_paused: false,
            error,
            maintenance: Instant::now(),
        }
    }
    pub fn observe(&mut self, entries: &[PortEntry], projects: &ProjectEngine) {
        if self.monitoring_paused || !self.config.enabled || self.repo.is_none() {
            return;
        }
        if let Err(error) = self.record(entries, projects) {
            self.error = Some(format!("History recording unavailable: {error}"));
            self.pause("Storage unavailable");
        }
    }
    fn record(&mut self, entries: &[PortEntry], projects: &ProjectEngine) -> Result<(), String> {
        let at = now();
        let gap = self.session.is_none()
            || self.last_scan.is_some_and(|(last, mono)| {
                at < last || at.saturating_sub(last) > 30 || mono.elapsed().as_secs() > 30
            });
        let previous_session = if gap {
            self.session.take().map(|mut s| {
                s.ended_at = Some(s.last_observed);
                s.reason = "Monitoring interrupted (sleep, pause, or clock change)".into();
                s
            })
        } else {
            None
        };
        let mut session = self.session.clone().unwrap_or_else(|| MonitoringSession {
            id: uuid::Uuid::new_v4().to_string(),
            started_at: at,
            last_observed: at,
            ended_at: None,
            reason: "Monitoring resumed".into(),
        });
        session.last_observed = at;
        let mut current = vec![];
        let mut new_entries = vec![];
        for p in entries {
            let found = self.reconciler.previous.iter().find(|a| {
                a.endpoint() == format!("{}:{}:{}", p.protocol, p.address, p.port)
                    && a.process.identity() == p.identity()
                    && a.process.pid == p.pid
                    && a.process.parent_pid == p.parent_pid
            });
            if let Some(old) = found {
                let mut observed = old.clone();
                observed.process.launch = p.launch.clone();
                if p.project.is_some() {
                    observed.process.project = p.project.clone();
                    observed.process.service_name = p.service_name.clone();
                }
                current.push(observed);
            } else {
                new_entries.push(p.clone());
            }
        }
        // Resolve only changed identities, through the existing cached Project Awareness engine.
        if !new_entries.is_empty() {
            projects.enrich(&mut new_entries, false);
            current.extend(new_entries.into_iter().map(|process| {
                let ancestors = ancestry::ancestors(&process);
                Observation { process, ancestors }
            }));
        }
        let mut next = self.reconciler.clone();
        next.reclaim_window_seconds = self.config.reclaim_window_seconds;
        let events = next.reconcile(current, at, &session.id, gap);
        let repo = self.repo.as_mut().ok_or("History database unavailable")?;
        if gap || !events.is_empty() {
            repo.record(&events, &next.previous, &session, previous_session.as_ref())?;
        }
        self.reconciler = next;
        self.session = Some(session);
        self.last_scan = Some((at, Instant::now()));
        self.error = None;
        if self.maintenance.elapsed().as_secs() >= 3600 {
            repo.prune(self.config.retention_days, at)?;
            self.maintenance = Instant::now();
        }
        Ok(())
    }
    pub fn pause(&mut self, reason: &str) {
        if let Some(mut s) = self.session.take() {
            s.ended_at = Some(s.last_observed);
            s.reason = reason.into();
            if let Some(repo) = &self.repo {
                if let Err(e) = repo.close_session(&s) {
                    self.error = Some(e);
                }
            }
        }
        self.last_scan = None;
    }
    pub fn query(&self, mut q: Query) -> Result<Page, String> {
        let repo = self
            .repo
            .as_ref()
            .ok_or_else(|| self.error.clone().unwrap_or("History unavailable".into()))?;
        let limit = q.limit.unwrap_or(100).clamp(1, 200);
        q.limit = Some(limit + 1);
        if self.config.retention_days > 0 {
            q.since = Some(
                q.since
                    .unwrap_or(0)
                    .max(now().saturating_sub(self.config.retention_days * 86400)),
            );
        }
        let mut events = repo.query(&q)?;
        let next_cursor = if events.len() > limit {
            events.truncate(limit);
            events.last().map(|e| e.sequence)
        } else {
            None
        };
        let mut sessions = repo.sessions()?;
        for s in &mut sessions {
            if let Some(live) = &self.session {
                if s.id == live.id {
                    *s = live.clone();
                    continue;
                }
            }
            // After a crash, the last durable observation is a conservative bound, not an invented shutdown time.
            if s.ended_at.is_none() {
                s.ended_at = Some(s.last_observed);
                s.reason = "Monitoring ended; exact stop time unavailable".into();
            }
        }
        let recent = repo.query(&Query {
            port: q.port,
            since: Some(now().saturating_sub(self.config.reclaim_window_seconds)),
            limit: Some(10000),
            ..Default::default()
        })?;
        let recurring = correlation::recurring(&recent, &self.config, now())
            .into_iter()
            .filter_map(|mut r| {
                let session = self.session.as_ref()?;
                if r.session_id != session.id {
                    return None;
                }
                let current = self.reconciler.previous.iter().find(|p| {
                    p.endpoint() == r.process.endpoint()
                        && p.process.identity() == r.process.process.identity()
                })?;
                r.persistent_ancestor = r
                    .persistent_ancestor
                    .filter(|a| current.ancestors.iter().any(|p| p.identity == a.identity));
                r.process = current.clone();
                Some(r)
            })
            .collect();
        Ok(Page {
            events,
            sessions,
            recurring,
            next_cursor,
            config: self.config.clone(),
            database_bytes: repo.bytes(),
            error: self.error.clone(),
            monitoring: self.session.is_some(),
        })
    }
    pub fn configure(&mut self, config: Config) -> Result<(), String> {
        if ![0, 1, 7, 30, 90].contains(&config.retention_days)
            || !(60..=3600).contains(&config.reclaim_window_seconds)
            || !(2..=20).contains(&config.reclaim_threshold)
        {
            return Err("Invalid history settings".into());
        }
        self.repo
            .as_ref()
            .ok_or("History unavailable")?
            .save_config(&config)?;
        if !config.enabled {
            self.pause("History recording disabled");
        }
        self.config = config;
        self.repo
            .as_ref()
            .unwrap()
            .prune(self.config.retention_days, now())
    }
    pub fn clear(&mut self) -> Result<(), String> {
        self.repo.as_ref().ok_or("History unavailable")?.clear()?;
        self.reconciler = Default::default();
        self.session = None;
        self.last_scan = None;
        Ok(())
    }
}
