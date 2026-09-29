use crate::ports::models::PortEntry;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Category {
    DevServer,
    UserProcess,
    Infrastructure,
    SystemProcess,
    Unknown,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Risk {
    Low,
    Medium,
    High,
    Blocked,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Safety {
    pub category: Category,
    pub risk: Risk,
    pub heading: String,
    pub reasons: Vec<String>,
    pub project: Option<String>,
    pub project_path: Option<String>,
    pub different_project: bool,
    pub previously_observed: bool,
    pub possibly_stale: bool,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Detected,
    Resolving,
    ForceRequired,
    Starting,
    Resolved,
    Failed,
    Unverified,
    Ignored,
    Expired,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub id: String,
    pub port: u16,
    pub command: Vec<String>,
    pub cwd: String,
    pub project: String,
    pub owner: Option<PortEntry>,
    pub safety: Safety,
    pub status: Status,
    pub message: String,
    pub evidence: String,
    pub steps: Vec<String>,
    pub output: String,
    pub alternative: Option<u16>,
    pub alternate_reason: String,
    pub restart_owner_available: bool,
    pub restart_owner_reason: String,
    pub launched_pid: Option<u32>,
    pub recovery_port: Option<u16>,
    pub recovery_action: Option<Action>,
    pub retry_command: Option<Vec<String>>,
    pub listener: Option<PortEntry>,
    pub created_at: u64,
    pub expires_at: u64,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    KillRetry,
    ForceRetry,
    Alternate,
    RestartOwner,
    Retry,
    Ignore,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub enabled: bool,
    pub supported: bool,
    pub connection_error: Option<String>,
    pub setup_command: String,
    pub conflicts: Vec<Conflict>,
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
