use crate::ports::models::PortEntry;
use serde::{Deserialize, Serialize};

pub use crate::process::ProcessIdentity;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ancestor {
    pub identity: ProcessIdentity,
    pub name: String,
    pub command: Vec<String>,
    pub cwd: Option<String>,
    pub parent_pid: Option<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub process: PortEntry,
    pub ancestors: Vec<Ancestor>,
}
impl Observation {
    pub fn endpoint(&self) -> String {
        format!(
            "{}:{}:{}",
            self.process.protocol, self.process.address, self.process.port
        )
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventType {
    PortClaimed,
    PortReleased,
    OwnerChanged,
    ProcessRestarted,
    PortReclaimed,
    Observed,
}
impl EventType {
    pub fn key(self) -> &'static str {
        match self {
            Self::PortClaimed => "PORT_CLAIMED",
            Self::PortReleased => "PORT_RELEASED",
            Self::OwnerChanged => "OWNER_CHANGED",
            Self::ProcessRestarted => "PROCESS_RESTARTED",
            Self::PortReclaimed => "PORT_RECLAIMED",
            Self::Observed => "OBSERVED",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    pub sequence: i64,
    pub timestamp: u64,
    pub session_id: String,
    pub port: u16,
    pub protocol: String,
    pub address: String,
    pub event_type: EventType,
    pub process: Option<Observation>,
    pub previous_process: Option<Observation>,
    pub uncertain: bool,
    #[serde(default)]
    pub endpoint_available: bool,
    pub correlation_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitoringSession {
    pub id: String,
    pub started_at: u64,
    pub last_observed: u64,
    pub ended_at: Option<u64>,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub enabled: bool,
    pub retention_days: u64,
    pub reclaim_window_seconds: u64,
    pub reclaim_threshold: usize,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            retention_days: 30,
            reclaim_window_seconds: 600,
            reclaim_threshold: 3,
        }
    }
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    pub port: Option<u16>,
    pub search: Option<String>,
    pub event_type: Option<String>,
    pub since: Option<u64>,
    pub before: Option<i64>,
    pub limit: Option<usize>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recurrence {
    pub session_id: String,
    pub port: u16,
    pub protocol: String,
    pub address: String,
    pub count: usize,
    pub first_at: u64,
    pub latest_at: u64,
    pub persistent_ancestor: Option<Ancestor>,
    pub process: Observation,
    pub previous_pids: Vec<u32>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub events: Vec<Event>,
    pub sessions: Vec<MonitoringSession>,
    pub recurring: Vec<Recurrence>,
    pub next_cursor: Option<i64>,
    pub config: Config,
    pub database_bytes: u64,
    pub error: Option<String>,
    pub monitoring: bool,
}
