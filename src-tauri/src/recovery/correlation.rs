use crate::{ports::models::PortEntry, process::ProcessIdentity, timeline::models::Ancestor};
pub fn contains(entry: &PortEntry, ancestors: &[Ancestor], root: ProcessIdentity) -> bool {
    entry.identity() == Some(root) || ancestors.iter().any(|a| a.identity == root)
}
