use super::models::*;
use crate::timeline::repository::Repository;
use rusqlite::params;
use std::path::Path;
/// Uses the existing timeline database and connection policy.
pub struct Persistence(pub Repository);
impl Persistence {
    pub fn open(path: &Path) -> Result<Self, String> {
        let repo = Repository::open(path)?;
        repo.db.execute_batch("CREATE TABLE IF NOT EXISTS launch_contexts (id TEXT PRIMARY KEY, payload TEXT NOT NULL, updated INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS launch_profiles (project TEXT PRIMARY KEY, payload TEXT NOT NULL);").map_err(|e|e.to_string())?;
        Ok(Self(repo))
    }
    pub fn save(&self, context: &LaunchContext) -> Result<(), String> {
        let mut durable = context.clone();
        // A persisted context is evidence, not authorization to substitute the app's environment.
        durable.recoverable = false;
        durable.reason = "This process depends on environment values Port Authority did not store. Restart it from the original terminal or configure a recovery command.".into();
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
        self.0.db.execute("DELETE FROM launch_contexts WHERE id NOT IN (SELECT id FROM launch_contexts ORDER BY updated DESC LIMIT 200)", []).map_err(|e|e.to_string())?;
        Ok(())
    }
    pub fn contexts(&self) -> Result<Vec<LaunchContext>, String> {
        self.read("SELECT payload FROM launch_contexts")
    }
    pub fn profiles(&self) -> Result<Vec<LaunchProfile>, String> {
        self.read("SELECT payload FROM launch_profiles")
    }
    fn read<T: serde::de::DeserializeOwned>(&self, query: &str) -> Result<Vec<T>, String> {
        let mut stmt = self.0.db.prepare(query).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.map(|r| {
            serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
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
}
