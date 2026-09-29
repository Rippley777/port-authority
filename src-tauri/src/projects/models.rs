use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryInfo {
    pub provider: Option<String>,
    pub owner: Option<String>,
    pub repository: String,
    // Sanitized: credentials, query strings, and fragments are never retained.
    pub remote_url: String,
    pub web_url: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Confidence {
    High,
    Medium,
    Low,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIdentity {
    pub name: String,
    pub root_path: String,
    pub display_path: String,
    pub project_type: Option<String>,
    pub frameworks: Vec<String>,
    pub repository: Option<RepositoryInfo>,
    pub git_branch: Option<String>,
    pub git_dirty: Option<bool>,
    pub confidence: Confidence,
    pub evidence: String,
    pub manifests: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentProject {
    pub identity: ProjectIdentity,
    pub last_observed: u64,
    pub pinned: bool,
    pub known_ports: Vec<u16>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSnapshot {
    pub projects: Vec<RecentProject>,
    pub resolving: bool,
    pub storage_error: Option<String>,
}
