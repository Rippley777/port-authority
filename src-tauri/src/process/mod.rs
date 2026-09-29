pub mod controller;
pub mod inspector;

use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ProcessIdentity {
    pub pid: u32,
    pub started_at: u64,
}
