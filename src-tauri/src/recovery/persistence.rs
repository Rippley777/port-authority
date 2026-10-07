use super::models::*;
use crate::{ports::models::PortEntry, timeline::repository::Repository};
use rusqlite::{params, OptionalExtension};
use std::path::Path;

/// Command Recovery and Run History share the existing timeline database.
pub struct Persistence(pub Repository);

impl Persistence {
    pub fn open(path: &Path) -> Result<Self, String> {
        let repo = Repository::open(path)?;
        repo.db
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS launch_contexts (
                    id TEXT PRIMARY KEY, payload TEXT NOT NULL, updated INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS execution_contexts (id TEXT PRIMARY KEY, payload TEXT NOT NULL);
                CREATE TABLE IF NOT EXISTS launch_profiles (
                    project TEXT PRIMARY KEY, payload TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS command_runs (
                    id TEXT PRIMARY KEY,
                    launch_context_id TEXT NOT NULL,
                    fingerprint TEXT NOT NULL,
                    started_at INTEGER NOT NULL,
                    ended_at INTEGER,
                    state TEXT NOT NULL,
                    search TEXT NOT NULL,
                    payload TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS command_runs_context_time
                    ON command_runs(launch_context_id, started_at DESC);
                CREATE INDEX IF NOT EXISTS command_runs_fingerprint_time
                    ON command_runs(fingerprint, started_at DESC);
                CREATE INDEX IF NOT EXISTS command_runs_state_time
                    ON command_runs(state, started_at DESC);
                CREATE TABLE IF NOT EXISTS run_ports (
                    run_id TEXT NOT NULL,
                    port INTEGER NOT NULL,
                    protocol TEXT NOT NULL,
                    address TEXT NOT NULL,
                    PRIMARY KEY(run_id, port, protocol, address)
                );
                CREATE INDEX IF NOT EXISTS run_ports_port ON run_ports(port, run_id);
                CREATE TABLE IF NOT EXISTS favorite_commands (
                    port INTEGER NOT NULL,
                    launch_context_id TEXT NOT NULL,
                    is_pinned INTEGER NOT NULL DEFAULT 0,
                    linked_at INTEGER NOT NULL,
                    PRIMARY KEY(port, launch_context_id)
                );
                CREATE INDEX IF NOT EXISTS favorite_commands_context
                    ON favorite_commands(launch_context_id);",
            )
            .map_err(|e| e.to_string())?;
        Ok(Self(repo))
    }

    pub fn save(&self, context: &DisplayLaunchContext) -> Result<(), String> {
        let mut durable = context.clone();
        if durable.fingerprint.is_empty() {
            durable.fingerprint = CommandFingerprint::from_context(&durable).key();
        }
        // Display metadata alone is never enough to launch a command.
        durable.recoverable = false;
        durable.recovery_confidence = RecoveryConfidence::Unavailable;
        durable.reason = if context.schema_version < 2 {
            "Legacy history entry: original launch command was not retained. Capture it again or configure a recovery command."
        } else if context.args.iter().any(|arg| arg.contains("••••")) {
            "Secret-bearing launch arguments were not saved. Run from the original terminal or configure a recovery command."
        } else {
            "Original execution data is unavailable. Capture the launch again or configure a recovery command."
        }.into();
        self.0
            .db
            .execute(
                "INSERT OR REPLACE INTO launch_contexts VALUES (?,?,?)",
                params![
                    durable.id,
                    serde_json::to_string(&durable).map_err(|e| e.to_string())?,
                    crate::autopilot::models::now()
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn save_capture(
        &self,
        display: &DisplayLaunchContext,
        capture: &crate::autopilot::capture::Capture,
    ) -> Result<(), String> {
        let transaction = self
            .0
            .db
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;
        self.save_execution(display, capture)?;
        self.save(display)?;
        transaction.commit().map_err(|e| e.to_string())
    }

    pub fn save_execution(
        &self,
        display: &DisplayLaunchContext,
        capture: &crate::autopilot::capture::Capture,
    ) -> Result<(), String> {
        if let Some(context) = super::execution::LaunchContext::from_capture(display, capture) {
            self.0
                .db
                .execute(
                    "INSERT OR REPLACE INTO execution_contexts VALUES (?,?)",
                    params![
                        context.id,
                        serde_json::to_string(&context).map_err(|e| e.to_string())?
                    ],
                )
                .map_err(|e| e.to_string())?;
        } else {
            // Never leave an older executable recipe behind under a reused id.
            self.0
                .db
                .execute("DELETE FROM execution_contexts WHERE id=?", [&display.id])
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub fn execution(&self, id: &str) -> Result<Option<super::execution::LaunchContext>, String> {
        let payload: Option<String> = self
            .0
            .db
            .query_row(
                "SELECT payload FROM execution_contexts WHERE id=?",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        payload
            .map(|p| serde_json::from_str(&p).map_err(|e| e.to_string()))
            .transpose()
    }

    pub fn contexts(&self) -> Result<Vec<DisplayLaunchContext>, String> {
        let mut contexts: Vec<DisplayLaunchContext> =
            self.read("SELECT payload FROM launch_contexts")?;
        for context in &mut contexts {
            if self
                .execution(&context.id)?
                .is_some_and(|execution| execution.capture().is_ok())
            {
                context.recoverable = true;
                context.recovery_confidence =
                    if context.relaunched_at.is_some() || context.source == Source::UserDefined {
                        RecoveryConfidence::Exact
                    } else if context.source == Source::ShellObserved {
                        RecoveryConfidence::Observed
                    } else {
                        RecoveryConfidence::Recovered
                    };
                context.reason = "Uses the current environment with captured toolchain paths. Secrets are not restored from history.".into();
            } else if context.schema_version < 2 {
                context.recoverable = false;
                context.reason = "Legacy history entry: original launch command was not retained. Capture it again or configure a recovery command.".into();
            }
            if context.fingerprint.is_empty() {
                context.fingerprint = CommandFingerprint::from_context(context).key();
            }
        }
        Ok(contexts)
    }

    pub fn profiles(&self) -> Result<Vec<LaunchProfile>, String> {
        self.read("SELECT payload FROM launch_profiles")
    }

    fn read<T: serde::de::DeserializeOwned>(&self, query: &str) -> Result<Vec<T>, String> {
        let mut stmt = self.0.db.prepare(query).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.map(|row| {
            serde_json::from_str(&row.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        })
        .collect()
    }

    pub fn profile(&self, profile: &LaunchProfile) -> Result<(), String> {
        self.0
            .db
            .execute(
                "INSERT OR REPLACE INTO launch_profiles VALUES (?,?)",
                params![
                    profile.project_id,
                    serde_json::to_string(profile).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn start_run(&self, context: &DisplayLaunchContext) -> Result<CommandRun, String> {
        if let Some(run) = self.latest_active(&context.id)? {
            if run.process_identity == Some(context.launch_root) {
                return Ok(run);
            }
            self.finish_run(
                &context.id,
                CommandRunState::Stopped,
                "A later execution of this command was observed.",
                None,
            )?;
        }
        let run = CommandRun {
            id: uuid::Uuid::new_v4().to_string(),
            launch_context_id: context.id.clone(),
            fingerprint: context.fingerprint.clone(),
            started_at: context.launch_root.started_at,
            ended_at: None,
            exit_code: None,
            termination_reason: None,
            state: CommandRunState::Running,
            project_id: context.project_id.clone(),
            project_name: context
                .project_id
                .as_ref()
                .and_then(|root| Path::new(root).file_name())
                .and_then(|name| name.to_str())
                .map(crate::projects::manifests::display_name),
            process_name: None,
            observed_ports: vec![],
            process_identity: Some(context.launch_root),
        };
        self.save_run(&run)?;
        Ok(run)
    }

    pub fn observe_port(
        &self,
        context: &DisplayLaunchContext,
        entry: &PortEntry,
    ) -> Result<(), String> {
        let Some(mut run) = self.latest_active(&context.id)? else {
            return Ok(());
        };
        run.fingerprint = context.fingerprint.clone();
        run.project_id = context.project_id.clone();
        if let Some(project) = &entry.project {
            run.project_name = Some(project.name.clone());
        }
        if entry.project.is_some() || run.process_name.is_none() {
            run.process_name = Some(
                entry
                    .service_name
                    .clone()
                    .unwrap_or_else(|| entry.process.clone()),
            );
        }
        // A run is rooted at its launcher; listener children must not replace it.
        run.process_identity = Some(context.launch_root);
        let binding = RunPort {
            port: entry.port,
            protocol: entry.protocol.clone(),
            address: entry.address.clone(),
        };
        if !run.observed_ports.contains(&binding) {
            run.observed_ports.push(binding.clone());
            run.observed_ports
                .sort_by_key(|port| (port.port, port.protocol.clone(), port.address.clone()));
        }
        self.save_run(&run)?;
        self.0
            .db
            .execute(
                "INSERT OR IGNORE INTO run_ports VALUES (?,?,?,?)",
                params![run.id, binding.port, binding.protocol, binding.address],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn refresh_active_context(&self, context: &DisplayLaunchContext) -> Result<(), String> {
        if let Some(mut run) = self.latest_active(&context.id)? {
            run.fingerprint = context.fingerprint.clone();
            run.project_id = context.project_id.clone();
            self.save_run(&run)?;
        }
        Ok(())
    }

    pub fn finish_run(
        &self,
        context_id: &str,
        state: CommandRunState,
        reason: impl Into<String>,
        exit_code: Option<i32>,
    ) -> Result<(), String> {
        if let Some(mut run) = self.latest_active(context_id)? {
            run.ended_at = Some(crate::autopilot::models::now());
            run.state = state;
            run.exit_code = exit_code;
            run.termination_reason = Some(reason.into());
            self.save_run(&run)?;
        }
        Ok(())
    }

    pub fn mark_unverified(
        &self,
        context_id: &str,
        reason: impl Into<String>,
    ) -> Result<(), String> {
        if let Some(mut run) = self.latest_active(context_id)? {
            run.state = CommandRunState::Unverified;
            run.termination_reason = Some(reason.into());
            self.save_run(&run)?;
        }
        Ok(())
    }

    pub fn finish_observed_root(
        &self,
        context_id: &str,
        root: crate::process::ProcessIdentity,
        reason: &str,
    ) -> Result<(), String> {
        if let Some(mut run) = self
            .latest_active(context_id)?
            .filter(|run| run.process_identity == Some(root))
        {
            run.state = CommandRunState::Stopped;
            run.ended_at = Some(crate::autopilot::models::now());
            run.termination_reason = Some(reason.into());
            self.save_run(&run)?;
        }
        Ok(())
    }

    fn latest_active(&self, context_id: &str) -> Result<Option<CommandRun>, String> {
        let payload: Option<String> = self
            .0
            .db
            .query_row(
                "SELECT payload FROM command_runs
                 WHERE launch_context_id=? AND state IN ('RUNNING','UNVERIFIED')
                 ORDER BY started_at DESC LIMIT 1",
                [context_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        payload
            .map(|value| serde_json::from_str(&value).map_err(|e| e.to_string()))
            .transpose()
    }

    pub(super) fn save_run(&self, run: &CommandRun) -> Result<(), String> {
        let search = format!(
            "{} {} {} {} {}",
            run.project_name.as_deref().unwrap_or_default(),
            run.project_id.as_deref().unwrap_or_default(),
            run.process_name.as_deref().unwrap_or_default(),
            run.observed_ports
                .iter()
                .map(|port| port.port.to_string())
                .collect::<Vec<_>>()
                .join(" "),
            run.fingerprint
        )
        .to_lowercase();
        self.0
            .db
            .execute(
                "INSERT INTO command_runs
                 (id,launch_context_id,fingerprint,started_at,ended_at,state,search,payload)
                 VALUES (?,?,?,?,?,?,?,?)
                 ON CONFLICT(id) DO UPDATE SET launch_context_id=excluded.launch_context_id,
                 fingerprint=excluded.fingerprint,started_at=excluded.started_at,ended_at=excluded.ended_at,
                 state=excluded.state,search=excluded.search,payload=excluded.payload",
                params![
                    run.id,
                    run.launch_context_id,
                    run.fingerprint,
                    run.started_at,
                    run.ended_at,
                    run_state_key(&run.state),
                    search,
                    serde_json::to_string(run).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn runs(&self, limit: usize) -> Result<Vec<CommandRun>, String> {
        let mut statement = self
            .0
            .db
            .prepare(
                "SELECT payload FROM command_runs ORDER BY started_at DESC, rowid DESC LIMIT ?",
            )
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([limit.clamp(1, 5000) as i64], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.map(|row| {
            serde_json::from_str(&row.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        })
        .collect()
    }

    pub fn remove_run(&self, id: &str) -> Result<(), String> {
        self.0
            .db
            .execute("DELETE FROM run_ports WHERE run_id=?", [id])
            .map_err(|e| e.to_string())?;
        self.0
            .db
            .execute("DELETE FROM command_runs WHERE id=?", [id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn pin(&self, port: u16, context_id: &str, pinned: bool) -> Result<(), String> {
        if pinned {
            let transaction = self
                .0
                .db
                .unchecked_transaction()
                .map_err(|e| e.to_string())?;
            transaction
                .execute(
                    "UPDATE favorite_commands SET is_pinned=0 WHERE port=?",
                    [port],
                )
                .map_err(|e| e.to_string())?;
            transaction
                .execute(
                    "INSERT INTO favorite_commands(port,launch_context_id,is_pinned,linked_at)
                     VALUES (?,?,1,?)
                     ON CONFLICT(port,launch_context_id) DO UPDATE SET is_pinned=1,linked_at=excluded.linked_at",
                    params![port, context_id, crate::autopilot::models::now()],
                )
                .map_err(|e| e.to_string())?;
            transaction.commit().map_err(|e| e.to_string())?;
        } else {
            self.0
                .db
                .execute(
                    "UPDATE favorite_commands SET is_pinned=0 WHERE port=? AND launch_context_id=?",
                    params![port, context_id],
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn pinned(&self) -> Result<Vec<(u16, String)>, String> {
        let mut statement = self
            .0
            .db
            .prepare("SELECT port,launch_context_id FROM favorite_commands WHERE is_pinned=1")
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
}

fn run_state_key(state: &CommandRunState) -> &'static str {
    match state {
        CommandRunState::Running => "RUNNING",
        CommandRunState::Completed => "COMPLETED",
        CommandRunState::Failed => "FAILED",
        CommandRunState::Stopped => "STOPPED",
        CommandRunState::Unverified => "UNVERIFIED",
    }
}
