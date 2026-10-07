use crate::process::ProcessIdentity;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    ShellObserved,
    ParentProcess,
    ProcessInspection,
    UserDefined,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Confidence {
    High,
    Medium,
    Low,
    Unknown,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LaunchKind {
    DirectProcess,
    ShellCommand,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryConfidence {
    Exact,
    Observed,
    Recovered,
    Inferred,
    #[default]
    Unavailable,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentVariable {
    pub name: String,
    pub value: Option<String>,
}
/// Sanitized metadata for UI/history; never an execution input.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayLaunchContext {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub environment_strategy: Option<String>,
    #[serde(default)]
    pub recovery_confidence: RecoveryConfidence,
    pub id: String,
    pub source: Source,
    pub confidence: Confidence,
    pub kind: LaunchKind,
    pub command: String,
    pub executable: String,
    pub args: Vec<String>,
    pub working_directory: String,
    pub environment: Vec<EnvironmentVariable>,
    pub shell: Option<String>,
    pub project_id: Option<String>,
    pub launch_root: ProcessIdentity,
    pub captured_at: u64,
    pub recoverable: bool,
    pub reason: String,
    #[serde(default)]
    pub relaunched_at: Option<u64>,
    /// Stable grouping key. It deliberately excludes PID and timestamps.
    #[serde(default)]
    pub fingerprint: String,
}

impl DisplayLaunchContext {
    pub fn grouping_key(&self) -> String {
        if self.args.iter().any(|a| a.contains("••••")) {
            format!("private-context:{}", self.id)
        } else {
            CommandFingerprint::from_context(self).key()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommandFingerprint {
    pub project_id: Option<String>,
    pub executable: String,
    pub args: Vec<String>,
    pub working_directory: String,
    pub execution_type: LaunchKind,
    pub shell: Option<String>,
}

impl CommandFingerprint {
    pub fn from_context(context: &DisplayLaunchContext) -> Self {
        Self {
            project_id: context.project_id.clone(),
            executable: context.executable.clone(),
            args: context.args.clone(),
            working_directory: context.working_directory.clone(),
            execution_type: context.kind.clone(),
            shell: context.shell.clone(),
        }
    }

    pub fn key(&self) -> String {
        // JSON provides a deterministic, length-delimited representation for this struct.
        serde_json::to_string(self).expect("CommandFingerprint is serializable")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandRunState {
    Running,
    Completed,
    Failed,
    Stopped,
    Unverified,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RunPort {
    pub port: u16,
    pub protocol: String,
    pub address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandRun {
    pub id: String,
    pub launch_context_id: String,
    pub fingerprint: String,
    pub started_at: u64,
    pub ended_at: Option<u64>,
    pub exit_code: Option<i32>,
    pub termination_reason: Option<String>,
    pub state: CommandRunState,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub process_name: Option<String>,
    pub observed_ports: Vec<RunPort>,
    pub process_identity: Option<ProcessIdentity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalCommand {
    pub launch_context: DisplayLaunchContext,
    pub latest_run: CommandRun,
    pub run_count: usize,
    pub typical_ports: Vec<u16>,
    pub pinned_ports: Vec<u16>,
    pub active: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HistoryQuery {
    pub search: Option<String>,
    pub state: Option<String>,
    pub since: Option<u64>,
    pub port: Option<u16>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunHistoryPage {
    pub commands: Vec<HistoricalCommand>,
    pub runs: Vec<CommandRun>,
    pub storage_error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchProfile {
    pub project_id: String,
    pub name: String,
    pub executable: String,
    pub args: Vec<String>,
    pub working_directory: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecoveryState {
    Ready,
    Validating,
    Terminating,
    WaitingForExit,
    WaitingForPortRelease,
    Relaunching,
    VerifyingProcess,
    VerifyingPort,
    Running,
    Unverified,
    ProcessChanged,
    TerminationFailed,
    PortNotReleased,
    CommandNotRecoverable,
    LaunchFailed,
    ExpectedPortNotOpened,
    PermissionDenied,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryStatus {
    pub state: RecoveryState,
    pub message: String,
    pub old_pid: u32,
    pub new_pid: Option<u32>,
    pub ports: Vec<u16>,
    pub output: String,
}
