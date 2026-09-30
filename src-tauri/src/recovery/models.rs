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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentVariable {
    pub name: String,
    pub value: Option<String>,
}
/// Public and durable metadata. Secret-bearing execution data lives separately in memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchContext {
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
