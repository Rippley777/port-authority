use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortEntry {
    #[serde(default)]
    pub metadata_access: Option<crate::process::inspector::MetadataAccess>,
    #[serde(default)]
    pub launch: Option<crate::recovery::models::DisplayLaunchContext>,
    #[serde(default)]
    pub project: Option<crate::projects::models::ProjectIdentity>,
    #[serde(default)]
    pub service_name: Option<String>,
    pub id: String,
    pub port: u16,
    pub protocol: String,
    pub address: String,
    pub pid: Option<u32>,
    pub process: String,
    #[serde(serialize_with = "crate::recovery::launch_context::serialize_argv")]
    pub command: Vec<String>,
    pub executable: Option<String>,
    pub cwd: Option<String>,
    pub parent_pid: Option<u32>,
    pub user: Option<String>,
    pub started_at: Option<u64>,
    pub memory: Option<u64>,
    pub cpu: Option<f32>,
    pub system: bool,
    pub protected: bool,
    pub restartable: bool,
    pub restart_reason: String,
    pub permission_limited: bool,
}

use crate::process::ProcessIdentity;
impl PortEntry {
    pub fn identity(&self) -> Option<ProcessIdentity> {
        Some(ProcessIdentity {
            pid: self.pid?,
            started_at: self.started_at.filter(|t| *t > 0)?,
        })
    }
}
