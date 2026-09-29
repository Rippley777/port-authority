use super::models::*;
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

pub struct Repository {
    pub db: Connection,
    path: PathBuf,
}
impl Repository {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let db = Connection::open(path).map_err(|e| e.to_string())?;
        db.busy_timeout(std::time::Duration::from_secs(2))
            .map_err(|e| e.to_string())?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA secure_delete=ON;
            CREATE TABLE IF NOT EXISTS port_events (
                sequence INTEGER PRIMARY KEY AUTOINCREMENT, id TEXT UNIQUE NOT NULL,
                timestamp INTEGER NOT NULL, port INTEGER NOT NULL, protocol TEXT NOT NULL,
                address TEXT NOT NULL, event_type TEXT NOT NULL, pid INTEGER, process_started_at INTEGER,
                project TEXT, search TEXT NOT NULL, payload TEXT NOT NULL);
            CREATE INDEX IF NOT EXISTS events_port_sequence ON port_events(port,sequence);
            CREATE INDEX IF NOT EXISTS events_type_sequence ON port_events(event_type,sequence);
            CREATE INDEX IF NOT EXISTS events_port_time ON port_events(port,timestamp);
            CREATE INDEX IF NOT EXISTS events_identity ON port_events(pid,process_started_at);
            CREATE INDEX IF NOT EXISTS events_project_time ON port_events(project,timestamp);
            CREATE INDEX IF NOT EXISTS events_type_time ON port_events(event_type,timestamp);
            CREATE INDEX IF NOT EXISTS events_time ON port_events(timestamp);
            CREATE TABLE IF NOT EXISTS monitoring_sessions (id TEXT PRIMARY KEY, started_at INTEGER NOT NULL, last_observed INTEGER NOT NULL, ended_at INTEGER, reason TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS timeline_state (key TEXT PRIMARY KEY, value TEXT NOT NULL);") .map_err(|e| e.to_string())?;
        Ok(Self {
            db,
            path: path.into(),
        })
    }
    pub fn state<T: serde::de::DeserializeOwned + Default>(&self, key: &str) -> Result<T, String> {
        use rusqlite::OptionalExtension;
        let value: Option<String> = self
            .db
            .query_row("SELECT value FROM timeline_state WHERE key=?", [key], |r| {
                r.get(0)
            })
            .optional()
            .map_err(|e| e.to_string())?;
        value
            .map(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
            .unwrap_or_else(|| Ok(T::default()))
    }
    pub fn save_config(&self, config: &Config) -> Result<(), String> {
        self.db
            .execute(
                "INSERT OR REPLACE INTO timeline_state VALUES ('config',?)",
                [serde_json::to_string(config).map_err(|e| e.to_string())?],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn record(
        &mut self,
        events: &[Event],
        current: &[Observation],
        session: &MonitoringSession,
        previous_session: Option<&MonitoringSession>,
    ) -> Result<(), String> {
        let tx = self.db.transaction().map_err(|e| e.to_string())?;
        for s in previous_session.into_iter().chain(std::iter::once(session)) {
            tx.execute(
                "INSERT OR REPLACE INTO monitoring_sessions VALUES (?,?,?,?,?)",
                params![s.id, s.started_at, s.last_observed, s.ended_at, s.reason],
            )
            .map_err(|e| e.to_string())?;
        }
        for e in events {
            let p = e
                .process
                .as_ref()
                .or(e.previous_process.as_ref())
                .map(|p| &p.process);
            let payload = serde_json::to_string(e).map_err(|e| e.to_string())?;
            let search = format!(
                "{} {} {} {}",
                e.event_type.key().replace('_', " "),
                payload,
                p.and_then(|p| p.pid)
                    .map(|pid| format!("PID {pid}"))
                    .unwrap_or_default(),
                e.previous_process
                    .as_ref()
                    .and_then(|p| p.process.pid)
                    .map(|pid| format!("PID {pid}"))
                    .unwrap_or_default()
            )
            .to_lowercase();
            tx.execute("INSERT INTO port_events (id,timestamp,port,protocol,address,event_type,pid,process_started_at,project,search,payload) VALUES (?,?,?,?,?,?,?,?,?,?,?)", params![e.id,e.timestamp,e.port,e.protocol,e.address,e.event_type.key(),p.and_then(|p|p.pid),p.and_then(|p|p.started_at),p.and_then(|p|p.project.as_ref()).map(|p|&p.root_path),search,payload]).map_err(|e|e.to_string())?;
        }
        tx.execute(
            "INSERT OR REPLACE INTO timeline_state VALUES ('owners',?)",
            [serde_json::to_string(current).map_err(|e| e.to_string())?],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }
    pub fn close_session(&self, s: &MonitoringSession) -> Result<(), String> {
        self.db
            .execute(
                "UPDATE monitoring_sessions SET last_observed=?, ended_at=?, reason=? WHERE id=?",
                params![s.last_observed, s.ended_at, s.reason, s.id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn sessions(&self) -> Result<Vec<MonitoringSession>, String> {
        let mut stmt = self.db.prepare("SELECT id,started_at,last_observed,ended_at,reason FROM monitoring_sessions ORDER BY rowid DESC LIMIT 1000").map_err(|e|e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok(MonitoringSession {
                    id: r.get(0)?,
                    started_at: r.get(1)?,
                    last_observed: r.get(2)?,
                    ended_at: r.get(3)?,
                    reason: r.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
    pub fn query(&self, q: &Query) -> Result<Vec<Event>, String> {
        let search = q.search.as_deref().unwrap_or("").to_lowercase();
        // Build only active predicates so SQLite can use endpoint/type indexes.
        let mut sql = String::from(
            "SELECT sequence,payload FROM port_events WHERE timestamp>=? AND sequence<?",
        );
        let mut values = vec![
            rusqlite::types::Value::Integer(q.since.unwrap_or(0).min(i64::MAX as u64) as i64),
            rusqlite::types::Value::Integer(q.before.unwrap_or(i64::MAX)),
        ];
        if let Some(port) = q.port {
            sql.push_str(" AND port=?");
            values.push((port as i64).into());
        }
        if let Some(kind) = &q.event_type {
            sql.push_str(" AND event_type=?");
            values.push(kind.clone().into());
        }
        if !search.is_empty() {
            sql.push_str(" AND instr(search,?)>0");
            values.push(search.into());
        }
        sql.push_str(" ORDER BY sequence DESC LIMIT ?");
        values.push((q.limit.unwrap_or(100).clamp(1, 10000) as i64).into());
        let mut stmt = self.db.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(values), |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        rows.map(|r| {
            let (seq, json) = r.map_err(|e| e.to_string())?;
            let mut event: Event = serde_json::from_str(&json).map_err(|e| e.to_string())?;
            event.sequence = seq;
            Ok(event)
        })
        .collect()
    }
    pub fn prune(&self, days: u64, now: u64) -> Result<(), String> {
        if days == 0 {
            return Ok(());
        }
        let cutoff = now.saturating_sub(days * 86400);
        // A read first keeps maintenance write-free until retained data actually expires.
        let exists: bool=self.db.query_row("SELECT EXISTS(SELECT 1 FROM port_events WHERE timestamp<? UNION ALL SELECT 1 FROM monitoring_sessions WHERE last_observed<?)",params![cutoff,cutoff],|r|r.get(0)).map_err(|e|e.to_string())?;
        if exists {
            self.db
                .execute("DELETE FROM port_events WHERE timestamp<?", [cutoff])
                .map_err(|e| e.to_string())?;
            self.db
                .execute(
                    "DELETE FROM monitoring_sessions WHERE last_observed<?",
                    [cutoff],
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub fn clear(&self) -> Result<(), String> {
        self.db.execute_batch("BEGIN; DELETE FROM port_events; DELETE FROM monitoring_sessions; DELETE FROM timeline_state WHERE key='owners'; COMMIT; PRAGMA wal_checkpoint(TRUNCATE); VACUUM;").map_err(|e|e.to_string())
    }
    pub fn bytes(&self) -> u64 {
        [
            &self.path,
            &PathBuf::from(format!("{}-wal", self.path.display())),
        ]
        .iter()
        .filter_map(|p| p.metadata().ok())
        .map(|m| m.len())
        .sum()
    }
}
